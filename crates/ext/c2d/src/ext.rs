//! Extension `c2d` : lecture des projets Carbide Create, accessible par commande
//! (menus de l'application, CLI, scripts).

use std::path::Path;

use makercraft_ext_api::{CommandSpec, ExtError, Extension, Registry, Value, json};

use crate::Project;

pub struct C2dExtension;

impl Extension for C2dExtension {
    fn id(&self) -> &'static str {
        "c2d"
    }

    fn label(&self) -> &'static str {
        "Projets Carbide Create (.c2d)"
    }

    fn register(&self, r: &mut Registry) -> Result<(), ExtError> {
        r.add_command(CommandSpec { id: "c2d.info", label: "Contenu d'un projet Carbide Create", menu: "Carbide Create", run: info })
    }
}

fn info(p: &Value) -> Result<Value, ExtError> {
    let path = p.get("path").and_then(Value::as_str).ok_or_else(|| ExtError::BadParams("attendu {\"path\": \"projet.c2d\"}".into()))?;
    let pr = Project::open(Path::new(path)).map_err(|e| ExtError::Failed(e.to_string()))?;
    Ok(json!({
        "stock": [pr.stock.width, pr.stock.height, pr.stock.thickness],
        "machine": pr.machine,
        "layers": pr.layers.iter().map(|l| json!({ "name": l.name, "visible": l.visible })).collect::<Vec<_>>(),
        "elements": pr.elements.len(),
        "toolpaths": pr.toolpaths.iter().map(|t| json!({ "name": t.name, "kind": t.kind.label(), "enabled": t.enabled, "tool": t.tool.number })).collect::<Vec<_>>(),
        "encrypted_gcode_ignored": pr.has_encrypted_gcode(),
        "warnings": pr.warnings,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use makercraft_ext_api::Host;

    #[test]
    fn registers_and_checks_params() {
        let mut host = Host::new();
        assert!(host.load(vec![Box::new(C2dExtension)]).is_ok());
        assert!(matches!(host.run("c2d.info", &json!({})), Err(ExtError::BadParams(_))));
        assert!(matches!(host.run("c2d.info", &json!({ "path": "absent.c2d" })), Err(ExtError::Failed(_))));
    }
}
