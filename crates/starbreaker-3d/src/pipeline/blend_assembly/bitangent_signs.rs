//! The game's tangent handedness (`Mesh.tangents[..][3]`) as the add-on's
//! `starbreaker_bitangent_sign` attribute, for the flat and NMC-hierarchy mesh
//! writers.
//!
//! The POM `tangent_space` group multiplies its bitangent, `cross(N, T)` of
//! the MikkTSpace tangent, by this per-corner float so UV-mirrored faces march
//! the right way. The mesh's own tangent frame (IVO tangent / QTangent stream)
//! carries that handedness, the frame the engine shades with. The writers flip
//! V (`uv_to_blender`), which mirrors UV space, so Blender's sign is the
//! negated game handedness. A mesh without tangents gets no attribute.
//!
//! - [`BitangentSignLayer::allocate`]: the per-corner signs (no tangents →
//!   `None`) and the name / `AttributeArray` / raw-data pointers.
//! - [`BitangentSignLayer::attribute`]: the `Attribute` entry.
//! - [`BitangentSignLayer::write`]: the name, array and data blocks.

use starbreaker_blend::{
    build_attribute, build_attribute_array, write_block, PtrAlloc, ATTR_DOMAIN_CORNER, ATTR_TYPE_FLOAT,
    SDNA_IDX_ATTRIBUTE_ARRAY,
};

use crate::error::Error;
use crate::types::Mesh;

/// The attribute the add-on's POM `tangent_space` group reads.
pub(super) const BITANGENT_SIGN_ATTRIBUTE: &str = "starbreaker_bitangent_sign";

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
    /// The signs of `mesh.tangents` per corner (`None` when the mesh has no
    /// tangents). Errors when an index has no tangent.
    pub fn allocate(ptrs: &mut PtrAlloc, mesh: &Mesh) -> Result<Option<Self>, Error> {
        let Some(tangents) = &mesh.tangents else {
            return Ok(None);
        };
        let mut data = Vec::with_capacity(mesh.indices.len() * 4);
        for &index in &mesh.indices {
            let tangent = tangents.get(index as usize).ok_or_else(|| {
                Error::Other(format!("index {index} has no tangent ({} tangents)", tangents.len()))
            })?;
            let sign: f32 = if tangent[3] < 0.0 { 1.0 } else { -1.0 };
            data.extend_from_slice(&sign.to_le_bytes());
        }
        Ok(Some(Self {
            name_ptr: ptrs.alloc(),
            array_ptr: ptrs.alloc(),
            raw_ptr: ptrs.alloc(),
            corners: mesh.indices.len(),
            data,
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
