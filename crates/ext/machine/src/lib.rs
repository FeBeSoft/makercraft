//! Extension `machine` : pilotage de machines à commande numérique Grbl 1.1 / grblHAL
//! (graveurs laser, fraiseuses, traceurs).
//!
//! - [`transport`] : série USB, TCP, émulateur intégré ;
//! - [`protocol`] : décodage des réponses Grbl, octets temps réel ;
//! - [`controller`] : thread d'envoi (comptage de caractères), états, sûreté ;
//! - [`codes`] : descriptions des erreurs, alarmes et réglages.
//!
//! Correspondance avec les sources portées : `upstream.toml` à la racine du dépôt.

pub mod codes;
pub mod controller;
pub mod protocol;
pub mod transport;

pub use controller::{Command, Controller, ControllerOptions, Event, Issue, MachineStatus, OnError, Origin, Progress, StreamMode};
pub use protocol::{MachineState, Response};
pub use transport::{TransportConfig, TransportError};

use makercraft_ext_api::{CommandSpec, ExtError, Extension, Registry, Value, json};

pub struct MachineExtension;

impl Extension for MachineExtension {
    fn id(&self) -> &'static str {
        "machine"
    }

    fn label(&self) -> &'static str {
        "Machine (laser, fraiseuse)"
    }

    fn register(&self, r: &mut Registry) -> Result<(), ExtError> {
        let specs: [CommandSpec; 7] = [
            CommandSpec { id: "machine.info", label: "Informations", menu: "Machine", run: info },
            CommandSpec { id: "machine.ports", label: "Ports série", menu: "Machine/Connexion", run: ports },
            CommandSpec { id: "machine.parse", label: "Décoder une réponse Grbl", menu: "Machine/Outils", run: parse },
            CommandSpec { id: "machine.error", label: "Expliquer une erreur", menu: "Machine/Outils", run: error },
            CommandSpec { id: "machine.alarm", label: "Expliquer une alarme", menu: "Machine/Outils", run: alarm },
            CommandSpec { id: "machine.setting", label: "Expliquer un réglage $", menu: "Machine/Outils", run: setting },
            CommandSpec { id: "machine.jog_line", label: "Construire une ligne de jog", menu: "Machine/Outils", run: jog },
        ];
        specs.into_iter().try_for_each(|s| r.add_command(s))
    }
}

fn info(_: &Value) -> Result<Value, ExtError> {
    Ok(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "firmwares": ["grbl-1.1", "grblhal"],
        "transports": [if cfg!(feature = "serial") { "serial" } else { "serial (indisponible)" }, "tcp", "emulator"],
        "stream_modes": ["buffered", "synchronous", "repeat_on_error"],
    }))
}

fn ports(_: &Value) -> Result<Value, ExtError> {
    let list = transport::list_ports().map_err(|e| ExtError::Failed(e.to_string()))?;
    Ok(Value::Array(list.into_iter().map(|p| json!({ "name": p.name, "description": p.description })).collect()))
}

fn param_u16(p: &Value, key: &str) -> Result<u16, ExtError> {
    p.get(key)
        .and_then(Value::as_u64)
        .and_then(|v| u16::try_from(v).ok())
        .ok_or_else(|| ExtError::BadParams(format!("attendu {{\"{key}\": entier}}")))
}

fn code_json(c: Option<codes::CodeInfo>, code: u16) -> Value {
    match c {
        Some(c) => json!({ "code": c.code, "brief": c.brief, "description": c.description }),
        None => json!({ "code": code, "brief": "inconnu" }),
    }
}

fn error(p: &Value) -> Result<Value, ExtError> {
    let code = param_u16(p, "code")?;
    Ok(code_json(codes::error(code), code))
}

fn alarm(p: &Value) -> Result<Value, ExtError> {
    let code = param_u16(p, "code")?;
    Ok(code_json(codes::alarm(code), code))
}

fn setting(p: &Value) -> Result<Value, ExtError> {
    let id = param_u16(p, "id")?;
    Ok(match codes::setting(id) {
        Some(s) => json!({ "id": s.id, "name": s.name, "unit": s.unit, "description": s.description }),
        None => json!({ "id": id, "name": "inconnu" }),
    })
}

fn xyz(v: makercraft_geom::Vec3) -> Value {
    json!([v.x, v.y, v.z])
}

fn parse(p: &Value) -> Result<Value, ExtError> {
    let line = p.get("line").and_then(Value::as_str).ok_or_else(|| ExtError::BadParams("attendu {\"line\": \"<Idle|MPos:0,0,0>\"}".into()))?;
    Ok(match Response::parse(line) {
        Response::Ok => json!({ "type": "ok" }),
        Response::Error(c) => json!({ "type": "error", "detail": code_json(codes::error(c), c) }),
        Response::Alarm(c) => json!({ "type": "alarm", "detail": code_json(codes::alarm(c), c) }),
        Response::Status(r) => json!({
            "type": "status",
            "state": format!("{:?}", r.state),
            "label": r.state.label(),
            "mpos": r.mpos.map(xyz),
            "wpos": r.wpos.map(xyz),
            "wco": r.wco.map(xyz),
            "feed": r.feed,
            "spindle": r.spindle,
            "buffer": r.buffer.map(|(a, b)| json!([a, b])),
            "overrides": r.overrides.map(|(a, b, c)| json!([a, b, c])),
        }),
        Response::Welcome(w) => json!({ "type": "welcome", "firmware": format!("{:?}", w.firmware), "version": w.version() }),
        Response::Setting { id, value } => json!({ "type": "setting", "id": id, "value": value }),
        Response::Feedback(f) => json!({ "type": "feedback", "text": f }),
        Response::Other(o) => json!({ "type": "other", "text": o }),
    })
}

fn jog(p: &Value) -> Result<Value, ExtError> {
    let g = |k: &str| p.get(k).and_then(Value::as_f64).unwrap_or(0.0);
    controller::jog_line(g("x"), g("y"), g("z"), g("feed"))
        .map(Value::String)
        .ok_or_else(|| ExtError::BadParams("attendu {\"x\"|\"y\"|\"z\": mm, \"feed\": mm/min > 0}".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use makercraft_ext_api::Host;

    #[test]
    fn registers_commands() {
        let mut host = Host::new();
        assert_eq!(host.load(vec![Box::new(MachineExtension)]), Ok(()));
        assert_eq!(host.commands().len(), 7);
        let v = host.run("machine.parse", &json!({ "line": "<Hold:0|MPos:1,2,3>" }));
        assert!(v.as_ref().is_ok_and(|v| v["state"] == "Hold" && v["mpos"] == json!([1.0, 2.0, 3.0])), "{v:?}");
        let v = host.run("machine.error", &json!({ "code": 22 }));
        assert!(v.is_ok_and(|v| v["brief"].as_str().is_some_and(|b| !b.is_empty())));
        assert_eq!(host.run("machine.jog_line", &json!({ "x": 5, "feed": 600 })), Ok(json!("$J=G91G21X5F600")));
        assert!(matches!(host.run("machine.error", &json!({})), Err(ExtError::BadParams(_))));
    }
}
