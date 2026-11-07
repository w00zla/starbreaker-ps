//! Tests for the `starbreaker_bitangent_sign` corner attribute in the `.blend`
//! writers (flat and NMC-hierarchy meshes): each triangle's handedness in
//! Blender's UV space (MikkTSpace's convention), one float per corner, taken
//! from the UVs and not from the game's tangent frame.

use super::*;
use starbreaker_blend::reader::{cstr, i16_at, i32_at, ptr_at, BlendFile, Block};
use starbreaker_blend::ATTR_TYPE_FLOAT;

use crate::types::SubMesh;

/// A quad (two triangles). In Blender's UV space (V flipped) the first
/// triangle winds clockwise (sign -1) and the second counter-clockwise (+1).
/// The tangents claim +1 everywhere: the attribute must not follow them.
fn quad(uvs: Option<Vec<[f32; 2]>>, tangents: Option<Vec<[f32; 4]>>) -> Mesh {
    let mut mesh = Mesh::empty();
    mesh.positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0], [2.0, 1.0, 0.0]];
    mesh.indices = vec![0, 1, 2, 1, 4, 2];
    mesh.uvs = uvs;
    mesh.tangents = tangents;
    mesh.submeshes = vec![SubMesh {
        material_name: Some("mat".to_string()),
        material_id: 0,
        source_material_id: None,
        first_index: 0,
        num_indices: 6,
        first_vertex: 0,
        num_vertices: 5,
        node_parent_index: 0,
    }];
    mesh
}

fn game_uvs() -> Vec<[f32; 2]> {
    vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.5, 0.5], [0.0, 1.0]]
}

fn game_tangents() -> Vec<[f32; 4]> {
    vec![[1.0, 0.0, 0.0, 1.0]; 5]
}

const EXPECTED: [f32; 6] = [-1.0, -1.0, -1.0, 1.0, 1.0, 1.0];

fn mesh_named<'a>(file: &'a BlendFile, name: &str) -> Block<'a> {
    file.ids(b"ME").find(|me| me.id_name() == name).unwrap_or_else(|| panic!("no mesh {name}"))
}

/// The mesh's `starbreaker_bitangent_sign` attribute: `(type, domain, values)`.
fn sign_attribute(file: &BlendFile, me: Block<'_>) -> Option<(i16, u8, Vec<f32>)> {
    let attrs = file.block(ptr_at(me.data, 456))?;
    attrs.data.chunks_exact(24).take(i32_at(me.data, 464) as usize).find_map(|a| {
        let name = file.block(ptr_at(a, 0)).map(|b| cstr(b.data, 0, b.data.len()).to_string())?;
        (name == "starbreaker_bitangent_sign").then(|| {
            let array = file.block(ptr_at(a, 16)).expect("attribute array");
            let raw = file.block(ptr_at(array.data, 0)).expect("attribute data");
            let values = raw.data.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
            (i16_at(a, 8), a[10], values)
        })
    })
}

fn flat_signs(mesh: &Mesh) -> Option<(i16, u8, Vec<f32>)> {
    let bytes = mesh_to_blend("quad", mesh, &None, None, None, &[], &[]).unwrap();
    let file = BlendFile::parse(&bytes).unwrap();
    sign_attribute(&file, mesh_named(&file, "quad"))
}

#[test]
fn flat_mesh_writes_the_uv_handedness_as_a_float_corner_attribute() {
    let (kind, domain, values) = flat_signs(&quad(Some(game_uvs()), Some(game_tangents()))).expect("bitangent sign");
    assert_eq!((kind, domain), (ATTR_TYPE_FLOAT, ATTR_DOMAIN_CORNER));
    assert_eq!(values, EXPECTED);
}

#[test]
fn a_mesh_without_tangents_gets_the_sign_from_its_uvs() {
    let (_, _, values) = flat_signs(&quad(Some(game_uvs()), None)).expect("bitangent sign");
    assert_eq!(values, EXPECTED);
}

#[test]
fn a_mesh_without_uvs_gets_no_sign() {
    assert!(flat_signs(&quad(None, Some(game_tangents()))).is_none());
}

#[test]
fn hierarchy_meshes_write_the_sign_per_node() {
    let identity = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]];
    let node = |name: &str, parent_index: Option<u16>, geometry_type: u16| crate::nmc::NmcNode {
        name: name.to_string(),
        parent_index,
        world_to_bone: identity,
        bone_to_world: identity,
        scale: [1.0; 3],
        geometry_type,
        properties: HashMap::new(),
    };
    let nmc = NodeMeshCombo { nodes: vec![node("root", None, 3), node("geo", Some(0), 0)], material_indices: vec![] };
    let mut mesh = quad(Some(game_uvs()), None);
    mesh.submeshes[0].node_parent_index = 1;
    let bytes = mesh_to_blend("asset_LOD0", &mesh, &None, Some(&nmc), None, &[], &[]).unwrap();
    let file = BlendFile::parse(&bytes).unwrap();
    let (kind, domain, values) = sign_attribute(&file, mesh_named(&file, "geo")).expect("bitangent sign");
    assert_eq!((kind, domain), (ATTR_TYPE_FLOAT, ATTR_DOMAIN_CORNER));
    assert_eq!(values, EXPECTED);
}

/// A triangle without UV area (MikkTSpace's "group with any") takes at each
/// corner the sign of the first proper triangle at that vertex; a corner with
/// none keeps the flat triangle's own -1.
#[test]
fn a_triangle_without_uv_area_takes_its_neighbours_sign() {
    let mut mesh = quad(Some(game_uvs()), None);
    let uvs = mesh.uvs.as_mut().unwrap();
    uvs[4] = [1.0, 0.5]; // (1, 4, 2) flat in UV: vertex 4 on the line 1-2
    uvs[3] = [0.0, 0.5]; // (1, 3, 2) counter-clockwise in Blender's UV space: +1
    mesh.indices = vec![1, 3, 2, 0, 1, 2, 1, 4, 2];
    mesh.submeshes[0].num_indices = 9;
    let (_, _, values) = flat_signs(&mesh).expect("bitangent sign");
    assert_eq!(values, [1.0, 1.0, 1.0, -1.0, -1.0, -1.0, 1.0, -1.0, 1.0]);
}
