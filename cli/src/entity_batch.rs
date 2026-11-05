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
//!
//! `--stdin` instead of a job list keeps the process alive: after loading it
//! prints `{"ready":true}` (or `{"ready":false,"error"}` and exits non-zero),
//! then reads one JSON object per line: a job (one entry of the job list,
//! answered with its result line), `{"end_chunk": "<output>"}` (forgets the
//! assets written into that output root, answered with
//! `{"chunk_done": "<output>"}`) or `{"quit": true}`. EOF ends it as well.
//! A line that is not a valid request is answered with
//! `{"ok": false, "error"}` (plus `name` when it has one). A panic in a job
//! ends the process, as in batch mode.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, Write};
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

/// Parse one job object `{name, output, worn?}`; the error says what is missing.
fn parse_job(item: &serde_json::Value) -> std::result::Result<BatchJob, &'static str> {
    let field = |key: &str| item.get(key).and_then(|v| v.as_str()).filter(|s| !s.is_empty());
    let name = field("name").ok_or("has no name")?;
    let output = field("output").ok_or("has no output")?;
    Ok(BatchJob {
        name: name.to_owned(),
        output: PathBuf::from(output),
        worn: item.get("worn").and_then(|v| v.as_bool()).unwrap_or(false),
    })
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
        .map(|(index, item)| parse_job(item).map_err(|why| CliError::InvalidInput(format!("batch job {index} {why}"))))
        .collect()
}

/// One request line of `--stdin` mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Request {
    Job(BatchJob),
    /// The output root as sent (echoed back in `chunk_done`).
    EndChunk(String),
    Quit,
}

/// Parse a request line; the error is the result line to answer it with.
pub(crate) fn parse_request(line: &str) -> std::result::Result<Request, serde_json::Value> {
    let fail = |name: Option<&str>, error: String| {
        let mut v = serde_json::json!({ "ok": false, "error": error, "seconds": 0.0 });
        if let Some(name) = name {
            v["name"] = name.into();
        }
        v
    };
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|e| fail(None, format!("request is not JSON: {e}")))?;
    if !value.is_object() {
        return Err(fail(None, "request must be a JSON object".into()));
    }
    if let Some(dir) = value.get("end_chunk") {
        return match dir.as_str().filter(|s| !s.is_empty()) {
            Some(dir) => Ok(Request::EndChunk(dir.to_owned())),
            None => Err(fail(None, "end_chunk needs an output root".into())),
        };
    }
    if value.get("quit").and_then(|v| v.as_bool()) == Some(true) {
        return Ok(Request::Quit);
    }
    parse_job(&value).map(Request::Job).map_err(|why| {
        let name = value.get("name").and_then(|v| v.as_str()).filter(|s| !s.is_empty());
        fail(name, format!("batch job {why}"))
    })
}

/// Write one protocol line to stdout (or a test buffer) and flush it.
fn emit(out: &mut impl Write, line: &serde_json::Value) -> Result<()> {
    writeln!(out, "{line}")
        .and_then(|_| out.flush())
        .map_err(|e| CliError::IoPath { source: e, path: "stdout".into() })
}

/// `--stdin` mode after loading: answer request lines until EOF or quit.
/// `job` exports one job, given the assets written so far into its output root.
fn serve(
    input: impl BufRead,
    out: &mut impl Write,
    mut job: impl FnMut(&BatchJob, &mut HashSet<String>) -> serde_json::Value,
) -> Result<()> {
    // Reusable assets written so far, per output root (lower-case relative paths).
    let mut written: HashMap<PathBuf, HashSet<String>> = HashMap::new();
    for line in input.lines() {
        let line = line.map_err(|e| CliError::IoPath { source: e, path: "stdin".into() })?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match parse_request(line) {
            Ok(Request::Job(j)) => {
                let result = job(&j, written.entry(j.output.clone()).or_default());
                emit(out, &result)?;
            }
            Ok(Request::EndChunk(dir)) => {
                // Batch mode has no step after its last job: the chunk only
                // ends asset reuse for its root (and frees that memory).
                written.remove(Path::new(&dir));
                emit(out, &serde_json::json!({ "chunk_done": dir }))?;
            }
            Ok(Request::Quit) => break,
            Err(answer) => emit(out, &answer)?,
        }
    }
    Ok(())
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

/// `--stdin` mode: a failed start is reported as `{"ready": false, "error"}`.
fn started<T>(stdin: bool, result: Result<T>) -> Result<T> {
    if stdin && let Err(e) = &result {
        // Best effort: the process exits non-zero either way.
        let _ = emit(&mut std::io::stdout().lock(), &serde_json::json!({ "ready": false, "error": e.to_string() }));
    }
    result
}

/// `jobs_path`: the JSON job list; `None` reads requests from stdin (`--stdin`).
pub(crate) fn export_batch(
    jobs_path: Option<PathBuf>,
    p4k_path: Option<PathBuf>,
    icon_files: bool,
    opts: ExportOpts,
) -> Result<()> {
    let jobs = match &jobs_path {
        Some(path) => {
            let bytes = std::fs::read(path)
                .map_err(|e| CliError::IoPath { source: e, path: path.display().to_string() })?;
            Some(parse_jobs(&bytes)?)
        }
        None => None,
    };
    let stdin = jobs.is_none();

    let (p4k, dcb_bytes) = started(stdin, load_dcb_bytes(p4k_path.as_deref(), None))?;
    let p4k = started(
        stdin,
        p4k.ok_or_else(|| CliError::MissingRequirement("P4k required for entity export".into())),
    )?;
    let db = started(stdin, Database::from_bytes(&dcb_bytes).map_err(CliError::from))?;
    let idx = EntityIndex::new(&db);
    let records = started(stdin, entity_records_by_name(&db))?;

    let run_job = |job: &BatchJob, assets: &mut HashSet<String>| {
        let start = Instant::now();
        let result = match records.get(&job.name.to_ascii_lowercase()) {
            None => Err(CliError::NotFound(format!("no EntityClassDefinition record named '{}'", job.name))),
            Some(record) => {
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
        match &result {
            Ok((package, worn)) => serde_json::json!({
                "name": job.name, "ok": true, "package": package, "worn": worn,
                "seconds": start.elapsed().as_secs_f64(),
            }),
            Err(e) => serde_json::json!({
                "name": job.name, "ok": false, "error": e.to_string(),
                "seconds": start.elapsed().as_secs_f64(),
            }),
        }
    };

    let stdout = std::io::stdout();
    match jobs {
        Some(jobs) => {
            // Reusable assets written so far, per output root (lower-case relative paths).
            let mut written: HashMap<PathBuf, HashSet<String>> = HashMap::new();
            for job in &jobs {
                let line = run_job(job, written.entry(job.output.clone()).or_default());
                emit(&mut stdout.lock(), &line)?;
            }
            Ok(())
        }
        None => {
            emit(&mut stdout.lock(), &serde_json::json!({ "ready": true }))?;
            serve(std::io::stdin().lock(), &mut stdout.lock(), run_job)
        }
    }
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
    fn requests_parse() {
        assert_eq!(
            parse_request(r#"{"name":"a","output":"x/1","worn":true}"#).unwrap(),
            Request::Job(BatchJob { name: "a".into(), output: PathBuf::from("x/1"), worn: true })
        );
        assert_eq!(parse_request(r#"{"end_chunk":"x/1"}"#).unwrap(), Request::EndChunk("x/1".into()));
        assert_eq!(parse_request(r#"{"quit":true}"#).unwrap(), Request::Quit);
        assert_eq!(parse_request(r#"{"name":"a"}"#).unwrap_err()["name"], "a");
        assert!(parse_request(r#"{"end_chunk":""}"#).is_err());
        assert!(parse_request("[1]").is_err());
        assert!(parse_request("nope").is_err());
    }

    /// The fake job reports the assets its root already had, then adds its own name.
    fn serve_lines(input: &str) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        serve(input.as_bytes(), &mut out, |job, assets| {
            let seen = assets.len();
            assets.insert(job.name.clone());
            serde_json::json!({ "name": job.name, "ok": true, "seen": seen })
        })
        .unwrap();
        String::from_utf8(out).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
    }

    #[test]
    fn serve_answers_every_request_and_end_chunk_forgets_the_root() {
        let lines = serve_lines(concat!(
            "{\"name\":\"a\",\"output\":\"x/1\"}\n",
            "{\"name\":\"b\",\"output\":\"x/2\"}\r\n",
            "\n",
            "{\"name\":\"c\",\"output\":\"x/1\"}\n",
            "garbage\n",
            "{\"end_chunk\":\"x/1\"}\n",
            "{\"name\":\"d\",\"output\":\"x/1\"}\n",
            "{\"name\":\"e\",\"output\":\"x/2\"}\n",
            "{\"quit\":true}\n",
            "{\"name\":\"never\",\"output\":\"x/1\"}\n",
        ));
        let seen = |i: usize| lines[i]["seen"].as_u64().unwrap();
        assert_eq!(lines.len(), 7);
        assert_eq!((seen(0), seen(1), seen(2)), (0, 0, 1));
        assert_eq!(lines[3]["ok"], false);
        assert_eq!(lines[4], serde_json::json!({ "chunk_done": "x/1" }));
        // x/1 starts over, x/2 still reuses b's assets
        assert_eq!((lines[5]["name"].as_str(), seen(5)), (Some("d"), 0));
        assert_eq!((lines[6]["name"].as_str(), seen(6)), (Some("e"), 1));
    }

    #[test]
    fn serve_ends_at_eof() {
        assert_eq!(serve_lines("{\"name\":\"a\",\"output\":\"x\"}").len(), 1);
        assert!(serve_lines("").is_empty());
    }

    #[test]
    fn icon_files_leave_out_animations_and_diagnostics() {
        assert!(is_icon_irrelevant_file("Packages/behr rifle_LOD1_TEX2/animations/fire_01.caf.json"));
        assert!(is_icon_irrelevant_file("Packages/behr rifle_LOD1_TEX2/weapon_assembly_diagnostics.json"));
        assert!(!is_icon_irrelevant_file("Packages/behr rifle_LOD1_TEX2/scene.json"));
        assert!(!is_icon_irrelevant_file("Data/Objects/animations/thing.blend"));
    }
}
