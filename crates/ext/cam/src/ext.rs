//! Extension `cam` : commandes accessibles depuis la CLI, les scripts et plus tard l'interface.

use std::path::{Path, PathBuf};

use makercraft_c2d::Project;
use makercraft_ext_api::{CommandSpec, ExtError, Extension, Registry, Value, json};

use crate::job::{Job, job_from_c2d, tool_label};
use crate::{gcode, preview};

pub struct CamExtension;

impl Extension for CamExtension {
    fn id(&self) -> &'static str {
        "cam"
    }

    fn label(&self) -> &'static str {
        "FAO (parcours d'outil, G-code)"
    }

    fn register(&self, r: &mut Registry) -> Result<(), ExtError> {
        r.add_command(CommandSpec { id: "cam.c2d_info", label: "Infos d'un projet Carbide Create", menu: "FAO/Carbide Create", run: c2d_info })?;
        r.add_command(CommandSpec {
            id: "cam.c2d_gcode",
            label: "G-code depuis un projet Carbide Create",
            menu: "FAO/Carbide Create",
            run: c2d_gcode,
        })
    }
}

fn path_param(p: &Value) -> Result<PathBuf, ExtError> {
    p.get("path").and_then(Value::as_str).map(PathBuf::from).ok_or_else(|| ExtError::BadParams("attendu {\"path\": \"projet.c2d\"}".into()))
}

/// Ouvre un projet et calcule le travail.
pub fn load(path: &Path, only: Option<&str>) -> Result<(Project, Job), String> {
    let pr = Project::open(path).map_err(|e| e.to_string())?;
    let title = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let job = job_from_c2d(&pr, &title, only);
    Ok((pr, job))
}

/// Écrit les fichiers G-code (un par outil, ou un seul) et l'aperçu SVG dans `dir`.
pub fn write_outputs(job: &Job, dir: &Path, single: bool) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{} : {e}", dir.display()))?;
    let safe = |s: &str| s.chars().map(|c| if c.is_alphanumeric() || " -_.".contains(c) { c } else { '_' }).collect::<String>();
    let mut files = Vec::new();
    let mut write = |name: String, content: &str| -> Result<(), String> {
        let p = dir.join(name);
        std::fs::write(&p, content).map_err(|e| format!("{} : {e}", p.display()))?;
        files.push(p);
        Ok(())
    };
    if single || job.groups.len() <= 1 {
        write(format!("{}.nc", safe(&job.title)), &gcode::write_all(job))?;
    } else {
        for (i, g) in job.groups.iter().enumerate() {
            let ops: Vec<&str> = g.ops.iter().map(|o| o.name.as_str()).collect();
            if let Some(text) = gcode::write_group(job, i) {
                write(format!("{} - {} - T{} {}.nc", safe(&job.title), i + 1, g.tool.number, safe(&ops.join(", "))), &text)?;
            }
        }
    }
    write(format!("{} - apercu.svg", safe(&job.title)), &preview::svg(job))?;
    Ok(files)
}

fn job_json(job: &Job) -> Value {
    let groups: Vec<Value> = job
        .groups
        .iter()
        .map(|g| {
            let st = g.stats(job.heights);
            json!({
                "tool": tool_label(&g.tool),
                "operations": g.ops.iter().map(|o| json!({ "name": o.name, "kind": o.kind, "warnings": o.warnings })).collect::<Vec<_>>(),
                "cut_length_mm": st.cut_length.round(),
                "minutes": (st.seconds / 60.0 * 10.0).round() / 10.0,
            })
        })
        .collect();
    json!({ "title": job.title, "stock": [job.stock_size.0, job.stock_size.1, job.stock_size.2], "machine": job.machine, "groups": groups, "warnings": job.warnings })
}

fn c2d_info(p: &Value) -> Result<Value, ExtError> {
    let path = path_param(p)?;
    let (pr, job) = load(&path, None).map_err(ExtError::Failed)?;
    let mut v = job_json(&job);
    v["elements"] = json!(pr.elements.len());
    v["layers"] = json!(pr.layers.iter().map(|l| l.name.clone()).collect::<Vec<_>>());
    v["encrypted_gcode_ignored"] = json!(pr.has_encrypted_gcode());
    Ok(v)
}

fn c2d_gcode(p: &Value) -> Result<Value, ExtError> {
    let path = path_param(p)?;
    let only = p.get("only").and_then(Value::as_str);
    let single = p.get("single").and_then(Value::as_bool).unwrap_or(false);
    let (_, job) = load(&path, only).map_err(ExtError::Failed)?;
    let dir = p.get("out").and_then(Value::as_str).map(PathBuf::from).unwrap_or_else(|| path.parent().map(Path::to_path_buf).unwrap_or_default());
    let files = write_outputs(&job, &dir, single).map_err(ExtError::Failed)?;
    let mut v = job_json(&job);
    v["files"] = json!(files.iter().map(|f| f.display().to_string()).collect::<Vec<_>>());
    Ok(v)
}
