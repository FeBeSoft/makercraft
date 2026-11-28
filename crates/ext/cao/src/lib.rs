//! Extension `cao` : modélisation 3D, greffée sur SolidCraft.
//!
//! SolidCraft est utilisé tel quel, en dépendance git épinglée (voir `UPSTREAM.md`) : seul son
//! noyau B-rep `solidcraft-kernel` (sans interface graphique) est lié. Ses crates d'interface
//! remplacent `egui-wgpu` et `winit` par leurs propres copies, incompatibles avec celles de
//! CadCraft dans un même exécutable.
//!
//! Pour la v0.1, l'adaptateur lit un fichier STEP et en donne les corps avec leurs mesures
//! (volume, surface, encombrement) : c'est la porte d'entrée de l'usinage 3D à venir.

pub mod view3d;

use std::path::Path;

use makercraft_ext_api::{CommandSpec, ExtError, Extension, Registry, Value, json};

/// Version de SolidCraft greffée (doit suivre le tag de `Cargo.toml`).
pub const SOLIDCRAFT_VERSION: &str = "0.51.2";

/// Un corps lu dans un fichier STEP, avec ses mesures (mm, mm², mm³).
#[derive(Debug, Clone, PartialEq)]
pub struct BodySummary {
    pub name: String,
    pub closed: bool,
    pub volume: f64,
    pub area: f64,
    pub bbox_min: [f64; 3],
    pub bbox_max: [f64; 3],
    pub faces: usize,
}

/// Contenu d'un fichier STEP.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StepSummary {
    pub bodies: Vec<BodySummary>,
    pub warnings: Vec<String>,
}

/// Lit le texte d'un fichier STEP et mesure chacun de ses corps.
pub fn step_summary_text(text: &str) -> Result<StepSummary, String> {
    let import = solidcraft_kernel::step_import(text).map_err(|e| e.to_string())?;
    let mut out = StepSummary { warnings: import.warnings.clone(), ..StepSummary::default() };
    for b in &import.bodies {
        match solidcraft_kernel::measure(&b.body) {
            Ok(m) => out.bodies.push(BodySummary {
                name: b.name.clone(),
                closed: b.closed,
                volume: m.volume,
                area: m.area,
                bbox_min: [m.bbox.min.x, m.bbox.min.y, m.bbox.min.z],
                bbox_max: [m.bbox.max.x, m.bbox.max.y, m.bbox.max.z],
                faces: m.faces,
            }),
            Err(e) => out.warnings.push(format!("« {} » : mesure impossible ({e})", b.name)),
        }
    }
    Ok(out)
}

/// Lit un fichier STEP.
pub fn step_summary(path: &Path) -> Result<StepSummary, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{} : {e}", path.display()))?;
    step_summary_text(&text)
}

pub struct CaoExtension;

impl Extension for CaoExtension {
    fn id(&self) -> &'static str {
        "cao"
    }

    fn label(&self) -> &'static str {
        "CAO 3D (SolidCraft)"
    }

    fn register(&self, r: &mut Registry) -> Result<(), ExtError> {
        r.add_command(CommandSpec { id: "cao.info", label: "Version de SolidCraft greffée", menu: "CAO", run: info })?;
        r.add_command(CommandSpec { id: "cao.step_info", label: "Contenu d'un fichier STEP", menu: "CAO/Fichiers", run: step_info })
    }
}

fn info(_: &Value) -> Result<Value, ExtError> {
    Ok(json!({ "solidcraft": SOLIDCRAFT_VERSION, "formats": ["step"] }))
}

fn step_info(p: &Value) -> Result<Value, ExtError> {
    let path = p.get("path").and_then(Value::as_str).ok_or_else(|| ExtError::BadParams("attendu {\"path\": \"piece.step\"}".into()))?;
    let s = step_summary(Path::new(path)).map_err(ExtError::Failed)?;
    let bodies: Vec<Value> = s
        .bodies
        .iter()
        .map(|b| json!({ "name": b.name, "closed": b.closed, "volume_mm3": b.volume, "area_mm2": b.area, "bbox_min": b.bbox_min, "bbox_max": b.bbox_max, "faces": b.faces }))
        .collect();
    Ok(json!({ "bodies": bodies, "warnings": s.warnings }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_are_registered() {
        let mut host = makercraft_ext_api::Host::new();
        assert!(host.load(vec![Box::new(CaoExtension)]).is_ok());
        let v = host.run("cao.info", &Value::Null).ok();
        assert_eq!(v.and_then(|v| v["solidcraft"].as_str().map(str::to_owned)).as_deref(), Some(SOLIDCRAFT_VERSION));
        assert!(matches!(host.run("cao.step_info", &json!({})), Err(ExtError::BadParams(_))));
    }

    #[test]
    fn rejects_text_that_is_not_step() {
        assert!(step_summary_text("ceci n'est pas un fichier STEP").is_err());
    }
}
