//! Each corner's UV handedness as the add-on's `starbreaker_bitangent_sign`
//! attribute, for the flat and NMC-hierarchy mesh writers.
//!
//! The POM `tangent_space` group multiplies its bitangent, `cross(N, T)` of
//! Blender's MikkTSpace tangent, by this per-corner float so UV-mirrored faces
//! march the right way. The value is MikkTSpace's own sign
//! (`loop.bitangent_sign`): +1 where a triangle's UVs wind counter-clockwise
//! in Blender's UV space (V flipped, `uv_to_blender`), -1 otherwise. A mesh
//! without UVs gets no attribute (Blender has no tangent for it either).
//!
//! **Why the UVs and not the game's tangent frame** (`Mesh.tangents[..][3]`,
//! the IVO tangent / QTangent handedness the engine shades with): the add-on
//! pairs the sign with Blender's MikkTSpace tangent, which Blender derives
//! from the UVs, so the sign must come from the same frame. The game's
//! handedness (negated for the V flip) matched Blender's sign on 98.7-99.9 %
//! of the corners of eight FPS weapons and three helmets, but on worn armour
//! cores it did not: 90.2 % (cds heavy core), 94.6 % (cds superheavy suit),
//! 94.7 % (ccc shrike). There 5-10 % of the corners carry a game sign against
//! their UV orientation, mostly on whole faces (cds heavy core: 6.9 % of the
//! faces), not along seams; mirrored transforms and flipped winding were
//! ruled out, the cause is unknown. The game data is
//! the alternative if that ever matters, e.g. for normal maps baked against
//! the game's frame.
//!
//! - [`corner_signs`]: the sign per corner.
//! - [`BitangentSignLayer::allocate`]: the signs (no UVs → `None`) and the
//!   name / `AttributeArray` / raw-data pointers.
//! - [`BitangentSignLayer::attribute`]: the `Attribute` entry.
//! - [`BitangentSignLayer::write`]: the name, array and data blocks.

use starbreaker_blend::{
    build_attribute, build_attribute_array, write_block, PtrAlloc, ATTR_DOMAIN_CORNER, ATTR_TYPE_FLOAT,
    SDNA_IDX_ATTRIBUTE_ARRAY,
};

use super::uv_to_blender;
use crate::error::Error;
use crate::types::Mesh;

/// The attribute the add-on's POM `tangent_space` group reads.
pub(super) const BITANGENT_SIGN_ATTRIBUTE: &str = "starbreaker_bitangent_sign";

/// MikkTSpace's sign per corner of `mesh` (triangles): +1 when the
/// triangle's Blender-space UVs have a positive signed area, else -1. A
/// triangle with no UV or position area (MikkTSpace's "group with any") takes
/// at each corner the sign of the first proper triangle at that vertex
/// (MikkTSpace's shared vertex: same position, normal and UV); with none
/// there it keeps its own (-1 for no UV area, as MikkTSpace's flag).
/// Errors when an index has no position or UV.
pub(super) fn corner_signs(mesh: &Mesh, uvs: &[[f32; 2]]) -> Result<Vec<f32>, Error> {
    let missing = |index: u32| Error::Other(format!("index {index} has no position or UV"));
    let uv = |index: u32| uvs.get(index as usize).copied().map(uv_to_blender).ok_or_else(|| missing(index));
    let position = |index: u32| mesh.positions.get(index as usize).copied().ok_or_else(|| missing(index));
    let shared_vertex = |index: u32| {
        let bits = |v: &[f32]| v.iter().map(|c| c.to_bits()).collect::<Vec<_>>();
        let normal = mesh.normals.as_ref().and_then(|n| n.get(index as usize)).map(|n| bits(n));
        (bits(&mesh.positions[index as usize]), bits(&uvs[index as usize]), normal)
    };

    let mut own = Vec::with_capacity(mesh.indices.len() / 3);
    for triangle in mesh.indices.chunks_exact(3) {
        let [a, b, c] = [uv(triangle[0])?, uv(triangle[1])?, uv(triangle[2])?];
        let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        let [p, q, r] = [position(triangle[0])?, position(triangle[1])?, position(triangle[2])?];
        let positions_span = (glam::Vec3::from(q) - glam::Vec3::from(p))
            .cross(glam::Vec3::from(r) - glam::Vec3::from(p))
            .length() > f32::MIN_POSITIVE;
        let sign = if area > 0.0 { 1.0 } else { -1.0 };
        own.push((sign, area.abs() > f32::MIN_POSITIVE && positions_span));
    }
    let mut group_sign = std::collections::HashMap::new();
    for (triangle, &(sign, proper)) in mesh.indices.chunks_exact(3).zip(&own) {
        if proper {
            for &index in triangle {
                group_sign.entry(shared_vertex(index)).or_insert(sign);
            }
        }
    }
    let mut signs = Vec::with_capacity(mesh.indices.len());
    for (triangle, &(sign, proper)) in mesh.indices.chunks_exact(3).zip(&own) {
        for &index in triangle {
            signs.push(if proper { sign } else { group_sign.get(&shared_vertex(index)).copied().unwrap_or(sign) });
        }
    }
    Ok(signs)
}

/// One mesh's `starbreaker_bitangent_sign` attribute and its block pointers.
#[derive(Debug, Clone)]
pub(super) struct BitangentSignLayer {
    name_ptr: u64,
    array_ptr: u64,
    raw_ptr: u64,
    corners: usize,
    data: Vec<u8>,
}

impl BitangentSignLayer {
    /// The signs of `mesh`'s UVs per corner (`None` when it has no UVs).
    pub fn allocate(ptrs: &mut PtrAlloc, mesh: &Mesh) -> Result<Option<Self>, Error> {
        let Some(uvs) = &mesh.uvs else {
            return Ok(None);
        };
        let signs = corner_signs(mesh, uvs)?;
        Ok(Some(Self {
            name_ptr: ptrs.alloc(),
            array_ptr: ptrs.alloc(),
            raw_ptr: ptrs.alloc(),
            corners: signs.len(),
            data: signs.iter().flat_map(|s| s.to_le_bytes()).collect(),
        }))
    }

    /// The `Attribute` struct for the mesh's attribute array.
    pub fn attribute(&self) -> Vec<u8> {
        build_attribute(self.name_ptr, ATTR_TYPE_FLOAT, ATTR_DOMAIN_CORNER, self.array_ptr)
    }

    /// Write the attribute's name, `AttributeArray` and raw data blocks.
    pub fn write(&self, out: &mut Vec<u8>) {
        let name = format!("{BITANGENT_SIGN_ATTRIBUTE}\0");
        write_block(out, b"DATA", 0, self.name_ptr, 1, name.as_bytes());
        write_block(out, b"DATA", SDNA_IDX_ATTRIBUTE_ARRAY, self.array_ptr, 1,
            &build_attribute_array(self.raw_ptr, self.corners as i64));
        write_block(out, b"DATA", 0, self.raw_ptr, 1, &self.data);
    }
}
