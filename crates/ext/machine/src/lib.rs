//! Extension `machine` : pilotage de machines à commande numérique.
//!
//! Squelette. Les modules à venir (voir PORTING.md) :
//! - `transport` : série, TCP, émulateur (LaserGRBL `ComWrapper`, Candle `connections`, UGS `connection`)
//! - `controller` : protocole Grbl 1.1 / grblHAL, streaming par comptage de caractères,
//!   commandes temps réel (LaserGRBL `GrblCore`, UGS `IController` / `ICommunicator`)
//! - `gcode` : parseur, état modal, arcs (Candle `parser`)

use makercraft_ext_api::{CommandSpec, ExtError, Extension, Registry, Value, json};

/// États machine : union de LaserGRBL `MacStatus` et UGS `ControllerState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineState {
    Disconnected,
    Connecting,
    Idle,
    Run,
    Hold,
    Jog,
    Alarm,
    Door,
    Check,
    Home,
    Sleep,
    Tool,
    Unknown,
}

impl MachineState {
    /// Lit le premier champ d'un rapport d'état Grbl (`<Idle|MPos:…>` → `Idle`, `Hold:0` → `Hold`).
    pub fn from_grbl(field: &str) -> Self {
        match field.split(':').next().unwrap_or("") {
            "Idle" => Self::Idle,
            "Run" => Self::Run,
            "Hold" => Self::Hold,
            "Jog" => Self::Jog,
            "Alarm" => Self::Alarm,
            "Door" => Self::Door,
            "Check" => Self::Check,
            "Home" => Self::Home,
            "Sleep" => Self::Sleep,
            "Tool" => Self::Tool,
            _ => Self::Unknown,
        }
    }
}

pub struct MachineExtension;

impl Extension for MachineExtension {
    fn id(&self) -> &'static str {
        "machine"
    }

    fn label(&self) -> &'static str {
        "Machine (laser, fraiseuse)"
    }

    fn register(&self, r: &mut Registry) -> Result<(), ExtError> {
        r.add_command(CommandSpec { id: "machine.info", label: "Informations", menu: "Machine", run: info })?;
        r.add_command(CommandSpec { id: "machine.parse_state", label: "Décoder un état Grbl", menu: "Machine/Outils", run: parse_state })
    }
}

fn info(_: &Value) -> Result<Value, ExtError> {
    Ok(json!({
        "status": "squelette",
        "firmwares": ["grbl-1.1", "grblhal"],
        "transports": ["serial", "tcp", "emulator"],
    }))
}

fn parse_state(p: &Value) -> Result<Value, ExtError> {
    let field = p.get("state").and_then(Value::as_str).ok_or_else(|| ExtError::BadParams("attendu {\"state\": \"Idle\"}".into()))?;
    Ok(json!({ "state": format!("{:?}", MachineState::from_grbl(field)) }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use makercraft_ext_api::Host;

    #[test]
    fn grbl_states() {
        assert_eq!(MachineState::from_grbl("Hold:0"), MachineState::Hold);
        assert_eq!(MachineState::from_grbl("Idle"), MachineState::Idle);
        assert_eq!(MachineState::from_grbl(""), MachineState::Unknown);
    }

    #[test]
    fn registers_commands() {
        let mut host = Host::new();
        assert_eq!(host.load(vec![Box::new(MachineExtension)]), Ok(()));
        assert_eq!(host.run("machine.parse_state", &json!({ "state": "Alarm" })), Ok(json!({ "state": "Alarm" })));
        assert!(matches!(host.run("machine.parse_state", &json!({})), Err(ExtError::BadParams(_))));
    }
}
