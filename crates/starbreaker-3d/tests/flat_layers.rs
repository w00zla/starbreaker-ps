//! MatLayers without a layer file (`Path=""`) against real data: a flat
//! TintColor the blend map lays on like any other layer (the glowing eye
//! strip of the Artimex helmet's face plate).
//!
//! Gated on `Data.p4k` being accessible; skips gracefully when absent.

use std::path::PathBuf;

use starbreaker_p4k::MappedP4k;

const DEFAULT_P4K_PATH: &str =
    r"C:\Program Files\Roberts Space Industries\StarCitizen\LIVE\Data.p4k";

fn p4k() -> Option<MappedP4k> {
    let path = std::env::var("SC_DATA_P4K")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_P4K_PATH));
    if !path.exists() {
        eprintln!("Data.p4k not found at {}, skipping", path.display());
        return None;
    }
    Some(MappedP4k::open(&path).expect("open p4k"))
}

#[test]
fn a_layer_without_a_file_is_kept_as_a_flat_layer() {
    let Some(p4k) = p4k() else { return };
    let bytes = p4k
        .read_file(r"Data\Objects\Characters\Human\male_v7\armor\qrt\quirinus\m_qrt_specialist_medium_helmet_01_04.mtl")
        .unwrap();
    let mtl = starbreaker_3d::mtl::parse_mtl(&bytes).unwrap();
    let face = mtl.materials.iter().find(|m| m.name == "face01_m").unwrap();

    let names: Vec<&str> = face.layers.iter().map(|l| l.name.as_str()).collect();
    assert!(!names.contains(&"BaseLayer1"), "layers keep only those with a file: {names:?}");
    assert_eq!(face.flat_layers.len(), 1);
    let flat = &face.flat_layers[0];
    assert_eq!(flat.name, "BaseLayer1");
    assert!(flat.path.is_empty());
    assert_eq!(flat.tint_color, [0.27049783, 0.013702084, 0.0]);
    assert!((flat.gloss_mult - 0.36).abs() < 1e-6);
    assert_eq!(flat.palette_tint, 0);
}
