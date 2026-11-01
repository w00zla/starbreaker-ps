//! `entity export-batch`: decomposed exports of many entities with one P4k and
//! DataCore load (loading both costs more than exporting a small item).
//!
//! Reads a JSON job list `[{"name": "<class>", "output": "<root>", "worn": bool}]`,
//! looks each entity up by its exact class name and writes its decomposed
//! package under `output`. Prints one JSON line per job to stdout:
//! `{"name", "ok", "package", "worn", "seconds", "error"}`.
//!
//! - A worn export that fails is retried with the default geometry.
//! - Assets an earlier job of the run wrote into the same output root are
//!   reused instead of exported again (mesh files carry their material in the
//!   name, so variants sharing a geometry do not overwrite each other).
//! - `--icon-files` leaves out what an icon render never reads: animation
//!   clips and the weapon assembly diagnostics.

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use starbreaker_datacore::database::Database;
use starbreaker_datacore::loadout::{EntityIndex, resolve_loadout_indexed};
use starbreaker_datacore::types::Record;
use starbreaker_p4k::MappedP4k;

use crate::common::{ExportOpts, load_dcb_bytes};
use crate::entity::{
    decomposed_package_rel_path, export_entity_name, prepare_decomposed_output_root,
    read_reusable_decomposed_asset, sanitize_export_name, write_decomposed_file,
};
use crate::error::{CliError, Result};

/// One entity to export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchJob {
    pub name: String,
    pub output: PathBuf,
    pub worn: bool,
}

/// Parse the job list: an array of `{name, output, worn?}` objects.
pub(crate) fn parse_jobs(bytes: &[u8]) -> Result<Vec<BatchJob>> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| CliError::InvalidInput(format!("batch job list is not JSON: {e}")))?;
    let items = value
        .as_array()
        .ok_or_else(|| CliError::InvalidInput("batch job list must be a JSON array".into()))?;
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let field = |key: &str| item.get(key).and_then(|v| v.as_str()).filter(|s| !s.is_empty());
            let name = field("name")
                .ok_or_else(|| CliError::InvalidInput(format!("batch job {index} has no name")))?;
            let output = field("output")
                .ok_or_else(|| CliError::InvalidInput(format!("batch job {index} has no output")))?;
            Ok(BatchJob {
                name: name.to_owned(),
                output: PathBuf::from(output),
                worn: item.get("worn").and_then(|v| v.as_bool()).unwrap_or(false),
            })
        })
        .collect()
}

/// Files an icon render never reads: the package's animation clips and the
/// weapon assembly diagnostics.
pub(crate) fn is_icon_irrelevant_file(relative_path: &str) -> bool {
    let normalized = relative_path.replace('\\', "/").to_ascii_lowercase();
    normalized.starts_with("packages/")
        && (normalized.contains("/animations/") || normalized.ends_with("/weapon_assembly_diagnostics.json"))
}

/// Exact, case-insensitive lookup of entity records by class name.
fn entity_records_by_name<'a>(db: &'a Database) -> Result<HashMap<String, &'a Record>> {
    let entity_si = db
        .struct_id("EntityClassDefinition")
        .ok_or_else(|| CliError::NotFound("EntityClassDefinition struct not found in DCB".into()))?;
    Ok(db
        .records_of_type(entity_si)
        .map(|r| (export_entity_name(db.resolve_string2(r.name_offset)).to_ascii_lowercase(), r))
        .collect())
}

pub(crate) fn export_batch(jobs_path: PathBuf, p4k_path: Option<PathBuf>, icon_files: bool, opts: ExportOpts) -> Result<()> {
    let jobs_bytes = std::fs::read(&jobs_path)
        .map_err(|e| CliError::IoPath { source: e, path: jobs_path.display().to_string() })?;
    let jobs = parse_jobs(&jobs_bytes)?;

    let (p4k, dcb_bytes) = load_dcb_bytes(p4k_path.as_deref(), None)?;
    let p4k = p4k.ok_or_else(|| CliError::MissingRequirement("P4k required for entity export".into()))?;
    let db = Database::from_bytes(&dcb_bytes)?;
    let idx = EntityIndex::new(&db);
    let records = entity_records_by_name(&db)?;

    // Reusable assets written so far, per output root (lower-case relative paths).
    let mut written: HashMap<PathBuf, HashSet<String>> = HashMap::new();
    let stdout = std::io::stdout();
    for job in &jobs {
        let start = Instant::now();
        let result = match records.get(&job.name.to_ascii_lowercase()) {
            None => Err(CliError::NotFound(format!("no EntityClassDefinition record named '{}'", job.name))),
            Some(record) => {
                let assets = written.entry(job.output.clone()).or_default();
                let worn_result = job
                    .worn
                    .then(|| export_one(&db, &p4k, &idx, record, &job.output, &opts, true, icon_files, assets));
                match worn_result {
                    Some(Ok(package)) => Ok((package, true)),
                    // No usable worn model: the default geometry, as a plain export would.
                    _ => export_one(&db, &p4k, &idx, record, &job.output, &opts, false, icon_files, assets)
                        .map(|package| (package, false)),
                }
            }
        };
        let line = match &result {
            Ok((package, worn)) => serde_json::json!({
                "name": job.name, "ok": true, "package": package, "worn": worn,
                "seconds": start.elapsed().as_secs_f64(),
            }),
            Err(e) => serde_json::json!({
                "name": job.name, "ok": false, "error": e.to_string(),
                "seconds": start.elapsed().as_secs_f64(),
            }),
        };
        let mut out = stdout.lock();
        writeln!(out, "{line}").and_then(|_| out.flush()).map_err(|e| CliError::IoPath {
            source: e,
            path: "stdout".into(),
        })?;
    }
    Ok(())
}

/// Export one entity's decomposed package; returns the package directory.
#[allow(clippy::too_many_arguments)]
fn export_one(
    db: &Database,
    p4k: &MappedP4k,
    idx: &EntityIndex,
    record: &Record,
    output: &Path,
    opts: &ExportOpts,
    worn: bool,
    icon_files: bool,
    written: &mut HashSet<String>,
) -> Result<PathBuf> {
    let mut export_opts = starbreaker_3d::ExportOptions::from(opts);
    export_opts.kind = starbreaker_3d::ExportKind::Decomposed;
    export_opts.format = starbreaker_3d::ExportFormat::Blend;
    export_opts.worn_subgeometry_tag = worn.then(|| "Male".to_string());

    let export_name = sanitize_export_name(&export_entity_name(db.resolve_string2(record.name_offset)));
    let package_name = decomposed_package_rel_path(&export_name, &export_opts);
    let output = output.to_path_buf();
    prepare_decomposed_output_root(&output, &package_name)?;

    let tree = resolve_loadout_indexed(idx, record);
    let existing = written.clone();
    let loader = |relative_path: &str| read_reusable_decomposed_asset(&output, &existing, relative_path);
    let result = starbreaker_3d::assemble_glb_with_loadout_with_progress(
        db,
        p4k,
        record,
        &tree,
        &export_opts,
        None,
        Some(&existing),
        None,
        Some(&loader),
    )?;
    let decomposed = result
        .decomposed
        .as_ref()
        .ok_or_else(|| CliError::InvalidInput("entity export returned no decomposed files".into()))?;

    for file in &decomposed.files {
        if icon_files && is_icon_irrelevant_file(&file.relative_path) {
            continue;
        }
        let key = file.relative_path.replace('\\', "/").to_ascii_lowercase();
        let output_path = output.join(&file.relative_path);
        if file.kind.is_reusable_asset() && written.contains(&key) && output_path.is_file() {
            continue;
        }
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CliError::IoPath { source: e, path: parent.display().to_string() })?;
        }
        write_decomposed_file(file, &output_path, false, p4k)?;
        if file.kind.is_reusable_asset() {
            written.insert(key);
        }
    }

    let package_dir = output.join("Packages").join(&package_name);
    if !package_dir.join("scene.json").is_file() {
        return Err(CliError::NotFound(format!(
            "decomposed scene manifest missing after export: {}",
            package_dir.display()
        )));
    }
    Ok(package_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jobs_parse_with_worn_defaulting_to_false() {
        let jobs = parse_jobs(br#"[{"name":"a_helmet","output":"x/1","worn":true},{"name":"b","output":"x/1"}]"#).unwrap();
        assert_eq!(
            jobs,
            vec![
                BatchJob { name: "a_helmet".into(), output: PathBuf::from("x/1"), worn: true },
                BatchJob { name: "b".into(), output: PathBuf::from("x/1"), worn: false },
            ]
        );
    }

    #[test]
    fn jobs_without_name_or_output_are_rejected() {
        assert!(parse_jobs(br#"[{"output":"x"}]"#).is_err());
        assert!(parse_jobs(br#"[{"name":"a"}]"#).is_err());
        assert!(parse_jobs(br#"{"name":"a"}"#).is_err());
    }

    #[test]
    fn icon_files_leave_out_animations_and_diagnostics() {
        assert!(is_icon_irrelevant_file("Packages/behr rifle_LOD1_TEX2/animations/fire_01.caf.json"));
        assert!(is_icon_irrelevant_file("Packages/behr rifle_LOD1_TEX2/weapon_assembly_diagnostics.json"));
        assert!(!is_icon_irrelevant_file("Packages/behr rifle_LOD1_TEX2/scene.json"));
        assert!(!is_icon_irrelevant_file("Data/Objects/animations/thing.blend"));
    }
}
