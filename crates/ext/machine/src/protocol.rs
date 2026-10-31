//! Protocole Grbl 1.1 / grblHAL : octets temps réel, décodage des lignes reçues, préparation
//! des lignes envoyées.
//!
//! Porté de LaserGRBL 7.14.1 :
//! - `LaserGRBL/Core/GrblCore.cs` : `ManageReceivedLine`, `ManageRealTimeStatus`, `Parse*`,
//!   messages de bienvenue, « ok » abîmé ;
//! - `LaserGRBL/GrblCommand.cs` : `BuildHelper` (commentaires, espaces) et `SerialData`.
//!
//! États : union de LaserGRBL `MacStatus` et UGS 2.1.27 `ControllerState`.

use makercraft_geom::Vec3;

/// Octets temps réel : traités par Grbl dès réception, hors du tampon de lignes.
pub mod rt {
    pub const STATUS: u8 = b'?';
    pub const CYCLE_START: u8 = b'~';
    pub const FEED_HOLD: u8 = b'!';
    pub const SOFT_RESET: u8 = 0x18;
    pub const SAFETY_DOOR: u8 = 0x84;
    pub const JOG_CANCEL: u8 = 0x85;
    pub const FEED_OV_RESET: u8 = 0x90;
    pub const FEED_OV_PLUS_10: u8 = 0x91;
    pub const FEED_OV_MINUS_10: u8 = 0x92;
    pub const FEED_OV_PLUS_1: u8 = 0x93;
    pub const FEED_OV_MINUS_1: u8 = 0x94;
    pub const RAPID_OV_100: u8 = 0x95;
    pub const RAPID_OV_50: u8 = 0x96;
    pub const RAPID_OV_25: u8 = 0x97;
    pub const SPINDLE_OV_RESET: u8 = 0x99;
    pub const SPINDLE_OV_PLUS_10: u8 = 0x9A;
    pub const SPINDLE_OV_MINUS_10: u8 = 0x9B;
    pub const SPINDLE_OV_PLUS_1: u8 = 0x9C;
    pub const SPINDLE_OV_MINUS_1: u8 = 0x9D;
    pub const SPINDLE_STOP: u8 = 0x9E;
    pub const FLOOD_TOGGLE: u8 = 0xA0;
    pub const MIST_TOGGLE: u8 = 0xA1;

    /// Vrai pour un octet que Grbl intercepte hors des lignes.
    pub fn is_realtime(b: u8) -> bool {
        matches!(b, STATUS | CYCLE_START | FEED_HOLD | SOFT_RESET) || b >= 0x80
    }
}

/// État de la machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
    #[default]
    Unknown,
}

impl MachineState {
    /// Lit le premier champ d'un rapport d'état (`Idle`, `Hold:0`, `Door:1`…).
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

    /// Libellé français.
    pub fn label(self) -> &'static str {
        match self {
            Self::Disconnected => "Déconnectée",
            Self::Connecting => "Connexion…",
            Self::Idle => "Prête",
            Self::Run => "En cours",
            Self::Hold => "En pause",
            Self::Jog => "Déplacement manuel",
            Self::Alarm => "Alarme",
            Self::Door => "Porte ouverte",
            Self::Check => "Mode vérification",
            Self::Home => "Prise d'origine",
            Self::Sleep => "En veille",
            Self::Tool => "Changement d'outil",
            Self::Unknown => "Inconnu",
        }
    }
}

/// Rapport d'état `<…>` décodé. Les champs absents du rapport restent `None`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StatusReport {
    pub state: MachineState,
    /// Sous-état (`Hold:0`, `Door:1`).
    pub sub_state: Option<u8>,
    pub mpos: Option<Vec3>,
    pub wpos: Option<Vec3>,
    pub wco: Option<Vec3>,
    pub feed: Option<f64>,
    pub spindle: Option<f64>,
    /// `Bf:` : (blocs libres du planificateur, octets libres du tampon de réception).
    pub buffer: Option<(u32, u32)>,
    /// `Ov:` : avance, rapides, broche/laser, en %.
    pub overrides: Option<(u32, u32, u32)>,
    pub pins: Option<String>,
}

/// Famille de firmware détectée au message de bienvenue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Firmware {
    Grbl,
    GrblHal,
}

/// Message de bienvenue (`Grbl 1.1h ['$' for help]`, `GrblHAL 1.1f ['$' or '$HELP' for help]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareInfo {
    pub firmware: Firmware,
    pub major: u8,
    pub minor: u8,
    pub build: Option<char>,
    pub raw: String,
}

impl FirmwareInfo {
    pub fn version(&self) -> String {
        match self.build {
            Some(b) => format!("{}.{}{b}", self.major, self.minor),
            None => format!("{}.{}", self.major, self.minor),
        }
    }

    /// Grbl ≥ 1.1 : jog `$J=`, overrides, rapports `|`, codes d'erreur numériques.
    pub fn supports_v11(&self) -> bool {
        (self.major, self.minor) >= (1, 1)
    }
}

/// Une ligne reçue, décodée.
#[derive(Debug, Clone, PartialEq)]
pub enum Response {
    Ok,
    /// `error:N` (0 si Grbl n'a pas donné de code).
    Error(u16),
    /// `ALARM:N`.
    Alarm(u16),
    Status(StatusReport),
    Welcome(FirmwareInfo),
    /// `$n=valeur` (réponse à `$$`).
    Setting {
        id: String,
        value: String,
    },
    /// `[MSG:…]`, `[GC:…]`, `[VER:…]`, `[OPT:…]`, `[PRB:…]`… : contenu entre crochets.
    Feedback(String),
    Other(String),
}

impl Response {
    /// Décode une ligne reçue (sans fin de ligne). Ne panique jamais.
    pub fn parse(line: &str) -> Self {
        let line = line.trim();
        let lower = line.to_ascii_lowercase();
        if lower == "ok" {
            return Self::Ok;
        }
        if lower.starts_with("error") {
            let code = line.split_once(':').and_then(|(_, c)| c.trim().parse().ok()).unwrap_or(0);
            return Self::Error(code);
        }
        if lower.starts_with("alarm:") {
            return Self::Alarm(line[6..].trim().parse().unwrap_or(0));
        }
        if let Some(report) = line.strip_prefix('<').and_then(|l| l.strip_suffix('>')).and_then(parse_status) {
            return Self::Status(report);
        }
        if let Some(info) = parse_welcome(line) {
            return Self::Welcome(info);
        }
        if line.starts_with('[') && line.ends_with(']') && line.len() >= 2 {
            return Self::Feedback(line[1..line.len() - 1].to_owned());
        }
        if let Some((id, value)) = line.strip_prefix('$').and_then(|s| s.split_once('=')) {
            return Self::Setting { id: id.to_owned(), value: value.to_owned() };
        }
        // « ok » abîmé par du bruit électrique (LaserGRBL `IsBrokenOkMessage`). Limité aux lignes
        // très courtes : LaserGRBL l'appliquait à toute ligne contenant « ok », y compris des messages.
        if line.len() <= 6 && lower.contains("ok") {
            return Self::Ok;
        }
        Self::Other(line.to_owned())
    }
}

fn parse_f(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// `x,y[,z[,…]]` → Vec3 (axe absent = 0, axes au-delà de Z ignorés).
fn parse_xyz(s: &str) -> Option<Vec3> {
    let mut it = s.split(',');
    let x = parse_f(it.next()?)?;
    let y = it.next().map(parse_f).unwrap_or(Some(0.0))?;
    let z = it.next().map(parse_f).unwrap_or(Some(0.0))?;
    Some(Vec3::new(x, y, z))
}

fn parse_u32_list<const N: usize>(s: &str) -> Option<[u32; N]> {
    let mut out = [0u32; N];
    let mut it = s.split(',');
    for v in out.iter_mut() {
        *v = it.next()?.trim().parse().ok()?;
    }
    Some(out)
}

/// Décode le contenu d'un rapport d'état, sans les chevrons.
pub fn parse_status(body: &str) -> Option<StatusReport> {
    let mut r = StatusReport::default();
    if body.contains('|') {
        // Grbl 1.1 : <Idle|MPos:0.000,0.000,0.000|FS:0,0|WCO:0.000,0.000,0.000>
        let mut fields = body.split('|');
        let state = fields.next()?;
        r.state = MachineState::from_grbl(state);
        r.sub_state = state.split_once(':').and_then(|(_, s)| s.parse().ok());
        for f in fields {
            let Some((key, value)) = f.split_once(':') else { continue };
            match key {
                "MPos" => r.mpos = parse_xyz(value),
                "WPos" => r.wpos = parse_xyz(value),
                "WCO" => r.wco = parse_xyz(value),
                "Bf" => r.buffer = parse_u32_list::<2>(value).map(|[a, b]| (a, b)),
                "Ov" => r.overrides = parse_u32_list::<3>(value).map(|[a, b, c]| (a, b, c)),
                "FS" => {
                    let mut it = value.split(',');
                    r.feed = it.next().and_then(parse_f);
                    r.spindle = it.next().and_then(parse_f);
                }
                "F" => r.feed = parse_f(value),
                "Pn" => r.pins = Some(value.to_owned()),
                _ => {}
            }
        }
    } else {
        // Grbl 0.9 : <Idle,MPos:0.000,0.000,0.000,WPos:0.000,0.000,0.000>
        let parts: Vec<&str> = body.split(',').collect();
        r.state = MachineState::from_grbl(parts.first()?);
        if parts.len() >= 4 {
            let x = parts[1].strip_prefix("MPos:")?;
            r.mpos = parse_xyz(&format!("{x},{},{}", parts[2], parts[3]));
        }
        if parts.len() >= 7 {
            let x = parts[4].strip_prefix("WPos:")?;
            r.wpos = parse_xyz(&format!("{x},{},{}", parts[5], parts[6]));
        }
    }
    (r.state != MachineState::Unknown).then_some(r)
}

/// `1.1h` → (1, 1, Some('h')).
fn parse_version(s: &str) -> Option<(u8, u8, Option<char>)> {
    let (major, rest) = s.split_once('.')?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let build = rest.chars().nth(digits.len()).filter(|c| !c.is_whitespace());
    Some((major.trim().parse().ok()?, digits.parse().ok()?, build))
}

/// Message de bienvenue Grbl ou grblHAL.
pub fn parse_welcome(line: &str) -> Option<FirmwareInfo> {
    let (firmware, rest) = match line.strip_prefix("GrblHAL ") {
        Some(r) => (Firmware::GrblHal, r),
        None => (Firmware::Grbl, line.strip_prefix("Grbl ")?),
    };
    let (major, minor, build) = parse_version(rest.split_whitespace().next()?)?;
    Some(FirmwareInfo { firmware, major, minor, build, raw: line.to_owned() })
}

/// `[OPT:V,15,128]` (contenu `OPT:V,15,128`) → (blocs du planificateur, octets du tampon de réception).
pub fn parse_opt(feedback: &str) -> Option<(u32, u32)> {
    let mut f = feedback.strip_prefix("OPT:")?.split(',');
    let _options = f.next()?;
    Some((f.next()?.trim().parse().ok()?, f.next()?.trim().parse().ok()?))
}

/// Prépare une ligne à envoyer : retire commentaires `( … )` et `; …`, met en majuscules et
/// supprime les espaces. Les commandes système `$…` sont envoyées telles quelles (la casse peut
/// compter : mot de passe Wi-Fi…). Renvoie `None` pour une ligne vide ou un délimiteur `%`.
pub fn prepare_line(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.starts_with('$') {
        return Some(raw.to_owned());
    }
    let mut out = String::with_capacity(raw.len());
    let mut depth = 0u32;
    for c in raw.chars() {
        match c {
            ';' if depth == 0 => break,
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth > 0 || c.is_whitespace() => {}
            _ => out.push(c.to_ascii_uppercase()),
        }
    }
    (!out.is_empty() && out != "%").then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_responses() {
        assert_eq!(Response::parse("ok\r"), Response::Ok);
        assert_eq!(Response::parse("error:22"), Response::Error(22));
        assert_eq!(Response::parse("error"), Response::Error(0));
        assert_eq!(Response::parse("ALARM:1"), Response::Alarm(1));
        assert_eq!(Response::parse("$110=500.000"), Response::Setting { id: "110".into(), value: "500.000".into() });
        assert_eq!(Response::parse("[MSG:Caution: Unlocked]"), Response::Feedback("MSG:Caution: Unlocked".into()));
        assert_eq!(Response::parse("\u{1}ok"), Response::Ok);
        assert_eq!(Response::parse("[MSG:Book]"), Response::Feedback("MSG:Book".into()));
        assert_eq!(Response::parse("Bonjour"), Response::Other("Bonjour".into()));
    }

    #[test]
    fn status_v11() {
        let Response::Status(r) = Response::parse("<Hold:0|MPos:1.000,2.500,-3.000|Bf:15,128|FS:1200,800|Ov:100,50,110|WCO:1.000,0.000,0.000|Pn:XZ>")
        else {
            panic!("pas un statut");
        };
        assert_eq!(r.state, MachineState::Hold);
        assert_eq!(r.sub_state, Some(0));
        assert_eq!(r.mpos, Some(Vec3::new(1.0, 2.5, -3.0)));
        assert_eq!(r.buffer, Some((15, 128)));
        assert_eq!((r.feed, r.spindle), (Some(1200.0), Some(800.0)));
        assert_eq!(r.overrides, Some((100, 50, 110)));
        assert_eq!(r.wco, Some(Vec3::new(1.0, 0.0, 0.0)));
        assert_eq!(r.pins.as_deref(), Some("XZ"));
    }

    #[test]
    fn status_v09_and_two_axes() {
        let Response::Status(r) = Response::parse("<Idle,MPos:1.000,2.000,3.000,WPos:0.000,0.000,0.000>") else { panic!() };
        assert_eq!(r.mpos, Some(Vec3::new(1.0, 2.0, 3.0)));
        assert_eq!(r.wpos, Some(Vec3::ZERO));
        let Response::Status(r) = Response::parse("<Jog|WPos:5.0,6.0|F:300>") else { panic!() };
        assert_eq!(r.wpos, Some(Vec3::new(5.0, 6.0, 0.0)));
        assert_eq!(r.feed, Some(300.0));
        // statut illisible : renvoyé tel quel, sans paniquer
        assert!(matches!(Response::parse("<Bidule|MPos:x>"), Response::Other(_)));
        assert!(matches!(Response::parse("<>"), Response::Other(_)));
    }

    #[test]
    fn welcome_and_options() {
        let Response::Welcome(w) = Response::parse("Grbl 1.1h ['$' for help]") else { panic!() };
        assert_eq!((w.firmware.clone(), w.version()), (Firmware::Grbl, "1.1h".to_owned()));
        assert!(w.supports_v11());
        let Response::Welcome(w) = Response::parse("GrblHAL 1.1f ['$' or '$HELP' for help]") else { panic!() };
        assert_eq!(w.firmware, Firmware::GrblHal);
        let Response::Welcome(w) = Response::parse("Grbl 0.9j ['$' for help]") else { panic!() };
        assert!(!w.supports_v11());
        assert_eq!(parse_opt("OPT:VL,15,128"), Some((15, 128)));
        assert_eq!(parse_opt("VER:1.1h"), None);
    }

    #[test]
    fn prepare_lines() {
        assert_eq!(prepare_line("g1 x10.5  y-3 (commentaire) f1000 ; fin"), Some("G1X10.5Y-3F1000".into()));
        assert_eq!(prepare_line("(seulement un commentaire)"), None);
        assert_eq!(prepare_line("  %  "), None);
        assert_eq!(prepare_line("$WiFi/Password=AbC d"), Some("$WiFi/Password=AbC d".into()));
        assert_eq!(prepare_line("G0 X1 (a (b) c) Y2"), Some("G0X1Y2".into()));
    }
}
