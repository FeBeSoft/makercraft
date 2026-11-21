//! Atelier Machines : page en onglets (Fiche, Pilotage, Calibrage, Réglages Grbl, Entretien et
//! journal) autour d'une machine Grbl 1.1 / grblHAL pilotée par l'extension `machine` (port
//! série USB ou émulateur intégré).
//!
//! Sûreté : l'émulateur est le port choisi par défaut ; sur une vraie machine, l'envoi d'un
//! programme et la mise en marche de la broche sont confirmés, et sans zéro valable les
//! déplacements manuels longs demandent une ou deux validations (voir `equip`).
//! Le mode d'utilisation (maker, prof, élève) règle ce qui est permis (`equip::Rights`).

pub mod equip;
mod fiche;
mod pilotage;
mod vue;

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::Duration;

use eframe::egui::{self, Align2, CornerRadius, Rect, RichText, ScrollArea, Sense, Stroke, Ui, UiBuilder, pos2, vec2};
use makercraft_ext_machine::codes;
use makercraft_ext_machine::program::ProgramText;
use makercraft_ext_machine::transport::{PortInfo, list_ports};
use makercraft_ext_machine::{Controller, ControllerOptions, Event, MachineState, MachineStatus, Progress, TransportConfig};
use makercraft_tooldb::{Profile, ProfileTool, ToolDb};

use self::equip::{Equipment, Reference, Rights};
use crate::icons::Icon;
use crate::prefs::{Mode, Prefs, Units};
use crate::ribbon::{Group, Item, Tab, Tone};
use crate::theme::{self, Palette};
use crate::widgets::{self, Kind};

/// Vitesses série proposées.
pub const BAUDS: [u32; 6] = [9_600, 19_200, 38_400, 57_600, 115_200, 250_000];
/// Pas de déplacement manuel proposés par défaut (mm), modifiables par clic droit.
pub const STEPS: [f64; 4] = [0.1, 1.0, 5.0, 10.0];
const LOG_MAX: usize = 400;
/// Libellé du port de l'émulateur dans la liste des ports.
const EMULATOR: &str = "Émulateur (essai sans machine)";

/// Ce que l'atelier demande à l'application.
pub enum Request {
    Status(String),
    /// Mémoriser le port et la vitesse choisis.
    Port(String, u32),
    /// Mémoriser les équipements déclarés d'une machine (identifiant du profil).
    Equipment(String, Equipment),
    /// Ouvrir la fenêtre Options de l'application (rail de gauche).
    Options,
}

/// Onglets de la page Machines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Fiche,
    Pilotage,
    Calibrage,
    Reglages,
    Entretien,
}

impl Page {
    fn label(self) -> &'static str {
        match self {
            Self::Fiche => "Fiche",
            Self::Pilotage => "Pilotage",
            Self::Calibrage => "Calibrage",
            Self::Reglages => "Réglages Grbl",
            Self::Entretien => "Entretien et journal",
        }
    }

    /// Onglet réservé aux réglages (masqué en mode élève).
    fn expert(self) -> bool {
        matches!(self, Self::Calibrage | Self::Reglages | Self::Entretien)
    }
}

struct Program {
    path: PathBuf,
    /// Lignes à envoyer et commentaires des pauses `M0` (rappel d'outil et de pince).
    text: ProgramText,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Run {
    Idle,
    Running,
    Finished,
    Aborted,
}

/// Confirmation demandée avant une action sur une vraie machine.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Confirm {
    Program,
    Spindle,
    /// Déplacement manuel sans zéro valable : validation `n` sur `of`.
    Jog { d: [f64; 3], absolute: bool, n: u8, of: u8 },
}

pub struct Machines {
    ports: Vec<PortInfo>,
    pub port: String,
    pub baud: u32,
    /// Émulateur intégré (aucune machine réelle) : port par défaut.
    pub emulator: bool,
    ctrl: Option<Controller>,
    transport: String,
    firmware: Option<String>,
    status: MachineStatus,
    log: VecDeque<String>,
    input: String,
    program: Option<Program>,
    progress: Progress,
    run: Run,
    paused: bool,
    /// Dernière alarme reçue, tant que la machine reste en alarme (bandeau d'état).
    alarm: Option<u16>,
    /// Pause programmée (`M0`) en cours : commentaire qui la précède dans le programme.
    pause_note: Option<String>,
    steps: [f64; 4],
    step: f64,
    /// Pas en cours de modification (clic droit) : index et valeur.
    step_edit: Option<(usize, f64)>,
    feed: f64,
    rpm: f64,
    confirm: Option<Confirm>,
    /// Validations déjà données pour les déplacements sans zéro valable (séance en cours).
    jog_ok: u8,
    // machine et équipements
    profiles: Vec<Profile>,
    profile: usize,
    tools: Vec<ProfileTool>,
    db_error: Option<String>,
    equipment: Equipment,
    reference: Reference,
    /// Prise d'origine `$H` demandée, en attente de la fin du cycle.
    homing: bool,
    mode: Mode,
    page: Page,
    view: vue::View,
    console_open: bool,
    /// Photo de la machine : fichier et texture (aucune si illisible).
    photo_tex: Option<(PathBuf, Option<egui::TextureHandle>)>,
    /// Position machine au moment du zéro table (coin avant gauche).
    table_origin: [f64; 3],
    /// Fenêtre « Refaire le Z » ouverte : pas choisi et épaisseur de la cale (mm).
    z_redo: Option<(f64, f64)>,
}

impl Default for Machines {
    fn default() -> Self {
        Self {
            ports: Vec::new(),
            port: String::new(),
            baud: 115_200,
            emulator: true,
            ctrl: None,
            transport: String::new(),
            firmware: None,
            status: MachineStatus::default(),
            log: VecDeque::new(),
            input: String::new(),
            program: None,
            progress: Progress::default(),
            run: Run::Idle,
            paused: false,
            alarm: None,
            pause_note: None,
            steps: STEPS,
            step: 1.0,
            step_edit: None,
            feed: 600.0,
            rpm: 10_000.0,
            confirm: None,
            jog_ok: 0,
            profiles: Vec::new(),
            profile: 0,
            tools: Vec::new(),
            db_error: None,
            equipment: Equipment::default(),
            reference: Reference::None,
            homing: false,
            mode: Mode::Maker,
            page: Page::Fiche,
            view: vue::View::Iso,
            console_open: true,
            photo_tex: None,
            table_origin: [0.0; 3],
            z_redo: None,
        }
    }
}

/// Lit un programme G-code : seules les lignes que le contrôleur enverra sont gardées (sans les
/// commentaires ni les lignes vides), pour que le compte affiché soit celui de l'envoi ; les
/// commentaires qui précèdent les pauses `M0` sont gardés à part.
pub fn read_program(path: &Path) -> Result<ProgramText, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{} : {e}", path.display()))?;
    Ok(ProgramText::parse(&text))
}

impl Machines {
    /// Page Machines selon les préférences : port, mode, machines de la banque et équipements.
    pub fn from_prefs(prefs: &Prefs) -> Self {
        let mut m = Self { port: prefs.port.clone(), baud: prefs.baud, mode: prefs.mode, ..Self::default() };
        match ToolDb::open(&Prefs::dir().join("outils")).and_then(|db| Ok((db.profiles()?, db))) {
            Ok((profiles, db)) => {
                m.profiles = profiles;
                if let Some(p) = m.profiles.first() {
                    m.tools = db.profile_tools(&p.id).unwrap_or_default();
                    m.equipment = prefs.equipment.get(&p.id).cloned().unwrap_or_default();
                }
            }
            Err(e) => m.db_error = Some(e.to_string()),
        }
        m
    }

    pub fn connected(&self) -> bool {
        self.ctrl.is_some()
    }

    pub fn state_label(&self) -> &'static str {
        if self.connected() { self.status.state.label() } else { "déconnectée" }
    }

    fn rights(&self) -> Rights {
        Rights::of(self.mode)
    }

    fn profile(&self) -> Option<&Profile> {
        self.profiles.get(self.profile)
    }

    /// Courses de la machine choisie (300 x 180 x 45 sans profil).
    fn travel(&self) -> [f64; 3] {
        self.profile().map_or([300.0, 180.0, 45.0], |p| p.travel_mm)
    }

    /// Position de l'outil sur la table (mm), si la position machine est connue.
    fn table_pos(&self) -> Option<[f64; 3]> {
        let m = &self.status.mpos;
        match self.reference {
            Reference::None => None,
            Reference::TableZero => Some([m.x - self.table_origin[0], m.y - self.table_origin[1], m.z - self.table_origin[2]]),
            Reference::Homed => Some([m.x, m.y, m.z]),
        }
    }

    /// Résumé pour la barre d'état de l'application.
    pub fn status_summary(&self) -> String {
        if !self.connected() {
            return "Déconnectée".to_owned();
        }
        let link = if self.emulator { "Émulateur".to_owned() } else { self.port.clone() };
        let buffer = self.status.buffer.map(|(_, rx)| format!(" · Tampon {rx} octets")).unwrap_or_default();
        let zero = match self.reference {
            Reference::None => "Position non référencée",
            Reference::TableZero => "Zéro table valable",
            Reference::Homed => "Axes référencés",
        };
        format!("Connecté · {link}{buffer} · {zero}")
    }

    fn rpm_max(&self) -> f64 {
        self.profile().map_or(20_000.0, |p| p.spindle_rpm_max)
    }

    fn log(&mut self, s: impl Into<String>) {
        self.log.push_back(s.into());
        while self.log.len() > LOG_MAX {
            self.log.pop_front();
        }
    }

    /// La position machine n'est plus sûre : zéro table et prise d'origine sont à refaire.
    fn lose_reference(&mut self, why: &str) {
        if self.reference != Reference::None {
            self.log(format!("Position perdue ({why}) : zéro table ou prise d'origine à refaire"));
        }
        self.reference = Reference::None;
        self.homing = false;
        self.jog_ok = 0;
    }

    pub fn refresh_ports(&mut self) {
        match list_ports() {
            Ok(p) => self.ports = p,
            Err(e) => self.log(format!("Ports série : {e}")),
        }
    }

    pub fn connect(&mut self) -> Result<String, String> {
        let config = if self.emulator { TransportConfig::Emulator } else { TransportConfig::Serial { port: self.port.clone(), baud: self.baud } };
        if !self.emulator && self.port.is_empty() {
            return Err("aucun port série choisi".into());
        }
        let c = Controller::connect(&config, ControllerOptions::default()).map_err(|e| e.to_string())?;
        self.ctrl = Some(c);
        self.firmware = None;
        self.status = MachineStatus::default();
        self.run = Run::Idle;
        self.paused = false;
        self.lose_reference("connexion");
        let what = if self.emulator { "émulateur".to_owned() } else { format!("{} à {} bauds", self.port, self.baud) };
        self.log(format!("Connexion : {what}"));
        Ok(format!("Connexion à {what}"))
    }

    pub fn disconnect(&mut self) {
        if let Some(c) = self.ctrl.take() {
            c.disconnect();
            self.log("Déconnecté");
        }
        self.status = MachineStatus::default();
        self.run = Run::Idle;
        self.paused = false;
        self.lose_reference("déconnexion");
    }

    /// Lit les événements du contrôleur (à chaque image).
    pub fn poll(&mut self, ctx: &egui::Context) {
        let Some(c) = &self.ctrl else { return };
        let events: Vec<Event> = c.events().try_iter().take(500).collect();
        let mut lost = None;
        for ev in events {
            match ev {
                Event::Connected { transport } => {
                    self.transport = transport.clone();
                    self.log(format!("Connecté ({transport})"));
                }
                Event::Firmware(f) => {
                    self.firmware = Some(format!("{:?} {}", f.firmware, f.version()));
                }
                Event::Status(s) => {
                    self.status = s;
                    self.on_state();
                }
                Event::Sent { .. } => {}
                Event::Ack { line, result: Err(code), .. } => {
                    let d = codes::error(code).map_or_else(|| "erreur inconnue".to_owned(), |c| c.explain());
                    self.log(format!("error:{code} sur « {line} » : {d}"));
                }
                Event::Ack { .. } => {}
                Event::Alarm(code) => {
                    let d = codes::alarm(code).map_or_else(|| "alarme inconnue".to_owned(), |c| c.explain());
                    self.log(format!("ALARME {code} : {d}"));
                    self.alarm = Some(code);
                    self.lose_reference("alarme");
                }
                Event::Line(l) => self.log(l),
                Event::Progress(p) => self.progress = p,
                Event::ProgramFinished(p) => {
                    self.progress = p;
                    self.run = Run::Finished;
                    self.paused = false;
                    self.log(format!("Programme terminé : {} ligne(s), {} erreur(s)", p.acked, p.errors));
                }
                Event::ProgramAborted(p) => {
                    self.progress = p;
                    self.run = Run::Aborted;
                    self.paused = false;
                    self.log(format!("Programme arrêté à la ligne {}", p.acked));
                }
                Event::Issue(i) => self.log(format!("Attention : {}", i.label())),
                Event::Disconnected { reason } => {
                    lost = Some(reason.unwrap_or_else(|| "liaison fermée".into()));
                }
            }
        }
        if let Some(r) = lost {
            self.log(format!("Déconnecté : {r}"));
            self.ctrl = None;
            self.status = MachineStatus::default();
            self.run = Run::Idle;
            self.lose_reference("liaison perdue");
        }
        if self.ctrl.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    /// Suit l'état reçu : fin d'alarme, fin de prise d'origine, pause programmée (`M0`).
    fn on_state(&mut self) {
        let st = self.status.state;
        if st != MachineState::Alarm {
            self.alarm = None;
        }
        if self.homing && st == MachineState::Idle {
            self.homing = false;
            self.reference = Reference::Homed;
            self.log("Prise d'origine faite : axes référencés");
        }
        if st != MachineState::Hold {
            self.pause_note = None;
            return;
        }
        // en pause sans que l'opérateur l'ait demandé : c'est un M0 du programme
        if self.run == Run::Running && !self.paused && self.pause_note.is_none() {
            let note = self.program.as_ref().and_then(|p| p.text.note_before(self.progress.sent)).map(str::to_owned);
            let note = note.unwrap_or_else(|| "pause du programme (M0)".to_owned());
            self.log(format!("Pause programme : {note}  -  Reprise pour continuer"));
            self.pause_note = Some(note);
            self.paused = true;
        }
    }

    /// Message du bandeau d'état : alarme en cours ou rappel de la pause programmée.
    pub fn banner(&self) -> Option<(bool, String)> {
        if let Some(code) = self.alarm {
            let t = codes::alarm(code).map_or_else(|| format!("Alarme {code}"), |c| format!("Alarme {code} : {} - à faire : {}", c.label(), c.action));
            return Some((true, t));
        }
        self.pause_note.as_ref().map(|n| {
            let tool = n.to_ascii_lowercase().contains("outil");
            let extra = if tool { "  -  Outil changé : refaire le Z (XY conservés)" } else { "" };
            (false, format!("Pause : {n}{extra}"))
        })
    }

    pub fn load_program(&mut self, path: &Path) -> Result<String, String> {
        let text = read_program(path)?;
        let n = text.lines.len();
        self.program = Some(Program { path: path.to_path_buf(), text });
        self.progress = Progress { total: n, ..Progress::default() };
        self.run = Run::Idle;
        Ok(format!("{n} ligne(s) chargée(s)"))
    }

    fn start(&mut self) {
        let (Some(c), Some(p)) = (&self.ctrl, &self.program) else { return };
        c.run_program(p.text.lines.clone());
        self.progress = Progress { total: p.text.lines.len(), ..Progress::default() };
        self.run = Run::Running;
        self.paused = false;
        let name = p.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.log(format!("Envoi de {name}"));
    }

    /// Déplacement manuel relatif (`absolute` : retour au zéro pièce en X et Y), après les
    /// validations nécessaires sans zéro valable.
    fn jog(&mut self, d: [f64; 3], absolute: bool) {
        if !self.connected() || self.run == Run::Running {
            return;
        }
        let dist = if absolute { self.status.wpos.x.hypot(self.status.wpos.y) } else { d.iter().map(|v| v * v).sum::<f64>().sqrt() };
        let need = equip::jog_validations(self.reference, self.emulator, dist);
        if need > self.jog_ok {
            self.confirm = Some(Confirm::Jog { d, absolute, n: self.jog_ok + 1, of: need });
            return;
        }
        self.do_jog(d, absolute);
    }

    fn do_jog(&mut self, d: [f64; 3], absolute: bool) {
        let feed = self.feed.min(self.rights().max_jog_feed);
        let Some(c) = &self.ctrl else { return };
        if absolute {
            c.send(format!("$J=G90 G21 X0 Y0 F{feed:.0}"));
        } else {
            c.jog(d[0], d[1], d[2], feed);
        }
    }

    fn spindle_on(&mut self) {
        let rpm = self.rpm.clamp(0.0, self.rpm_max());
        if let Some(c) = &self.ctrl {
            c.send(format!("M3 S{rpm:.0}"));
            self.log(format!("Broche en marche à {rpm:.0} tr/min"));
        }
    }

    /// Choix d'une autre machine de la banque : ses outils et ses équipements déclarés.
    fn select_profile(&mut self, i: usize, prefs_equipment: Option<Equipment>) {
        self.profile = i;
        self.equipment = prefs_equipment.unwrap_or_default();
        self.photo_tex = None;
        self.tools = match ToolDb::open(&Prefs::dir().join("outils")) {
            Ok(db) => self.profiles.get(i).and_then(|p| db.profile_tools(&p.id).ok()).unwrap_or_default(),
            Err(_) => Vec::new(),
        };
    }

    /// Choix fait dans une liste déroulante du ruban.
    pub fn choose(&mut self, id: &str, i: usize, prefs: &Prefs, req: &mut Vec<Request>) {
        if self.connected() {
            return;
        }
        match id {
            "machine.choix.profil" if i < self.profiles.len() => {
                let eq = self.profiles.get(i).and_then(|p| prefs.equipment.get(&p.id).cloned());
                self.select_profile(i, eq);
            }
            "machine.choix.port" => {
                if i == 0 {
                    self.emulator = true;
                } else if let Some(p) = self.ports.get(i - 1) {
                    self.emulator = false;
                    self.port = p.name.clone();
                } else {
                    self.refresh_ports();
                    req.push(Request::Status(format!("{} port(s) série trouvé(s)", self.ports.len())));
                }
            }
            "machine.choix.vitesse" => {
                if let Some(b) = BAUDS.get(i) {
                    self.baud = *b;
                }
            }
            _ => {}
        }
    }

    /// Onglets du ruban de l'atelier.
    pub fn ribbon(&self) -> Vec<Tab> {
        let r = self.rights();
        let connected = self.connected();
        let running = self.run == Run::Running;
        let on = connected && !running;
        let link = if connected {
            Item::large("machine.deconnecter", "Déconnecter", Icon::Unplug).tip("Fermer la liaison avec la machine")
        } else {
            let what = if self.emulator { "l'émulateur".to_owned() } else if self.port.is_empty() { "la machine (aucun port choisi)".to_owned() } else { format!("{} à {} bauds", self.port, self.baud) };
            Item::large("machine.connecter", "Connecter", Icon::Plug).tone(Tone::Component).tip(format!("Se connecter à {what}"))
        };
        let home_tip = if self.equipment.can_home() { "Prise d'origine machine par les fins de course" } else { "Fins de course à déclarer dans la fiche" };
        let busy = connected;
        let mut machines: Vec<(String, bool)> = self.profiles.iter().map(|p| (p.name.clone(), true)).collect();
        machines.push(("Isel CPM 2018-TS (à venir)".to_owned(), false));
        let mut ports = vec![(EMULATOR.to_owned(), true)];
        ports.extend(self.ports.iter().map(|p| (format!("{}  {}", p.name, p.description), true)));
        ports.push(("Actualiser la liste des ports".to_owned(), true));
        let port_sel = if self.emulator { 0 } else { self.ports.iter().position(|p| p.name == self.port).map_or(0, |i| i + 1) };
        let bauds: Vec<(String, bool)> = BAUDS.iter().map(|b| (format!("{b} bauds"), true)).collect();
        let baud_sel = BAUDS.iter().position(|b| *b == self.baud).unwrap_or(4);
        vec![Tab::new(
            "Machine",
            vec![
                Group {
                    label: "Machine",
                    items: vec![
                        Item::choice("machine.choix.profil", "Machine", Icon::Gantry, machines, self.profile, 240.0).enabled(!busy).tip("machine de la banque"),
                        Item::choice("machine.choix.port", "Port", Icon::Plug, ports, port_sel, 240.0).enabled(!busy).tip("l'émulateur ne commande aucune machine réelle"),
                        Item::choice("machine.choix.vitesse", "Vitesse série", Icon::Rapids, bauds, baud_sel, 240.0).enabled(!busy && !self.emulator),
                    ],
                },
                Group { label: "Connexion", items: vec![link] },
                Group {
                    label: "Programme",
                    items: vec![
                        Item::large("machine.ouvrir", "Ouvrir .nc", Icon::Open).tone(Tone::Design).enabled(!running).tip("Charger un programme G-code"),
                        Item::large("machine.demarrer", "Démarrer", Icon::Play).tone(Tone::Component).enabled(on && self.program.is_some() && r.run_program).tip("Envoyer le programme chargé"),
                        Item::small("machine.pause", "Pause", Icon::Pause).enabled(running && !self.paused),
                        Item::small("machine.reprise", "Reprise", Icon::Play).enabled(running && self.paused),
                        Item::large("machine.arret", "Arrêt", Icon::Stop).tone(Tone::Danger).enabled(connected).tip("Arrêt logiciel : pause puis reset de la carte"),
                    ],
                },
                Group {
                    label: "Commandes",
                    items: vec![
                        Item::small("machine.origine", "Origine ($H)", Icon::Home).enabled(on && self.equipment.can_home()).tip(home_tip),
                        Item::small("machine.deverrouiller", "Déverrouiller ($X)", Icon::Unlock).enabled(on && r.unlock).tip("Lever l'alarme"),
                        Item::small("machine.zero", "Zéro pièce XYZ", Icon::Zero).enabled(on).tip("La position actuelle devient X0 Y0 Z0"),
                    ],
                },
            ],
        )]
    }

    /// Exécute une commande (ruban ou boutons de la page).
    pub fn command(&mut self, id: &str, req: &mut Vec<Request>) {
        let r = self.rights();
        let running = self.run == Run::Running;
        match id {
            "machine.connecter" if !self.connected() => {
                let res = self.connect();
                if res.is_ok() && !self.emulator {
                    req.push(Request::Port(self.port.clone(), self.baud));
                }
                req.push(Request::Status(res.unwrap_or_else(|e| format!("Connexion impossible : {e}"))));
            }
            "machine.deconnecter" => {
                self.disconnect();
                req.push(Request::Status("Machine déconnectée".into()));
            }
            "machine.ouvrir" if !running => {
                if let Some(p) = rfd::FileDialog::new().add_filter("G-code", &["nc", "gcode", "ngc", "tap", "txt"]).pick_file() {
                    req.push(Request::Status(self.load_program(&p).unwrap_or_else(|e| format!("Programme illisible : {e}"))));
                }
            }
            "machine.demarrer" if self.connected() && !running && self.program.is_some() && r.run_program => {
                if self.emulator {
                    self.start();
                } else {
                    self.confirm = Some(Confirm::Program);
                }
            }
            "machine.pause" if running && !self.paused => {
                if let Some(c) = &self.ctrl {
                    c.hold();
                    self.paused = true;
                }
            }
            "machine.reprise" => {
                if let Some(c) = &self.ctrl {
                    c.cycle_start();
                    self.paused = false;
                }
            }
            "machine.arret" => {
                if let Some(c) = &self.ctrl {
                    c.emergency_stop();
                    if running {
                        self.run = Run::Aborted;
                    }
                    self.paused = false;
                    self.log("Arrêt logiciel (pause + reset, broche coupée)");
                    self.lose_reference("arrêt");
                }
            }
            "machine.reset" if r.unlock => {
                if let Some(c) = &self.ctrl {
                    c.soft_reset();
                    self.log("Réinitialisation de Grbl (Ctrl-X)");
                    self.lose_reference("réinitialisation");
                }
            }
            "machine.origine" if !running && self.equipment.can_home() => {
                if let Some(c) = &self.ctrl {
                    c.home();
                    self.homing = true;
                }
            }
            "machine.deverrouiller" if !running && r.unlock => {
                if let Some(c) = &self.ctrl {
                    c.unlock();
                }
            }
            "machine.zero" if !running => {
                if let Some(c) = &self.ctrl {
                    c.set_work_zero(true, true, true);
                }
            }
            "machine.zero.x" | "machine.zero.y" | "machine.zero.z" if !running => {
                if let Some(c) = &self.ctrl {
                    c.set_work_zero(id.ends_with('x'), id.ends_with('y'), id.ends_with('z'));
                }
            }
            "machine.zero_table" if !running && r.edit_fiche => {
                if self.connected() {
                    let m = &self.status.mpos;
                    self.table_origin = [m.x, m.y, m.z];
                    self.reference = Reference::TableZero;
                    self.log("Zéro table pris à la position actuelle (coin avant gauche) pour la séance");
                }
            }
            "machine.lire" => {
                if let Some(c) = &self.ctrl {
                    c.realtime(b'?');
                }
            }
            "machine.broche.marche" if !running && r.spindle && self.connected() => {
                if self.emulator {
                    self.spindle_on();
                } else {
                    self.confirm = Some(Confirm::Spindle);
                }
            }
            "machine.broche.arret" => {
                if let Some(c) = &self.ctrl {
                    c.send("M5");
                    self.log("Broche arrêtée");
                }
            }
            _ => {}
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, pal: &Palette, units: Units, prefs: &Prefs, req: &mut Vec<Request>) {
        if self.mode != prefs.mode {
            self.mode = prefs.mode;
        }
        if !self.rights().expert_tabs && self.page.expert() {
            self.page = Page::Pilotage;
        }
        let full = ui.available_rect_before_wrap();
        ui.allocate_rect(full, Sense::hover());
        let rail = Rect::from_min_size(full.min, vec2(RAIL_W, full.height()));
        let body = Rect::from_min_max(pos2(rail.right() + 14.0, full.top()), full.max);
        let mut r = ui.new_child(UiBuilder::new().max_rect(rail));
        self.rail_ui(&mut r, pal, req);
        let mut b = ui.new_child(UiBuilder::new().max_rect(body));
        let ui = &mut b;
        let _ = prefs;
        ui.horizontal(|ui| {
            let (ir, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
            crate::icons::paint(ui.painter(), ir, Icon::Gantry, pal.orange);
            ui.label(RichText::new(self.page.label()).font(theme::bold(19.0)).color(pal.orange));
            if self.mode != Mode::Maker {
                widgets::pill(ui, &format!("mode {}", self.mode.label()), pal.blue_soft, pal.text);
            }
        });
        let r = ui.available_rect_before_wrap();
        ui.painter().hline(r.x_range(), r.top() + 4.0, Stroke::new(1.0, pal.card_line));
        ui.add_space(14.0);
        match self.page {
            Page::Fiche => self.fiche_ui(ui, pal, req),
            Page::Pilotage => self.pilotage_ui(ui, pal, units, req),
            Page::Calibrage => self.calibrage_ui(ui, pal, req),
            Page::Reglages => soon(ui, pal, "Réglages Grbl", &["Lecture des réglages $$ avec l'explication de chacun en français", "Sauvegarde et restauration", "Comparaison avec la fiche machine (courses, pas par mm, vitesses, broche $30 / $31)"]),
            Page::Entretien => soon(ui, pal, "Entretien et journal", &["Heures de broche cumulées", "Graissage des vis trapézoïdales et des tiges lisses, serrages", "Journal des alarmes, des calibrages et des changements de réglages Grbl"]),
        }
        if self.confirm.is_some() {
            self.confirm_window(ui.ctx(), pal);
        }
    }

    /// Rail de gauche : les pages de la machine (Fiche, Pilotage, Calibrage, Réglages Grbl,
    /// Entretien et journal), puis Bibliothèque d'outils et Liste d'achat ; en bas Documentation,
    /// Aide, Options. Ce qui n'existe pas encore est grisé ; le mode élève masque les réglages.
    fn rail_ui(&mut self, ui: &mut Ui, pal: &Palette, req: &mut Vec<Request>) {
        let full = ui.max_rect();
        ui.painter().rect_filled(full, CornerRadius::same(12), pal.panel);
        let expert = self.rights().expert_tabs;
        let item_h = 62.0;
        let mut y = full.top() + 8.0;
        let pages = [(Page::Fiche, Icon::IdCard), (Page::Pilotage, Icon::Jog), (Page::Calibrage, Icon::Zero), (Page::Reglages, Icon::Sliders), (Page::Entretien, Icon::Journal)];
        for (page, icon) in pages {
            if page.expert() && !expert {
                continue;
            }
            let r = Rect::from_min_size(pos2(full.left(), y), vec2(full.width(), item_h));
            if rail_item(ui, pal, r, icon, page.label(), true, self.page == page) {
                self.page = page;
            }
            y += item_h + 2.0;
        }
        ui.painter().hline(full.x_range().shrink(14.0), y + 4.0, Stroke::new(1.0, pal.card_line));
        y += 10.0;
        for (icon, label) in [(Icon::Library, "Bibliothèque d'outils"), (Icon::Cart, "Liste d'achat")] {
            let r = Rect::from_min_size(pos2(full.left(), y), vec2(full.width(), item_h));
            rail_item(ui, pal, r, icon, label, false, false);
            y += item_h + 2.0;
        }
        let mut y = full.bottom() - 8.0 - 3.0 * (item_h + 2.0);
        for (icon, label, on) in [(Icon::Book, "Documentation", false), (Icon::Help, "Aide", false), (Icon::Settings, "Options", true)] {
            let r = Rect::from_min_size(pos2(full.left(), y), vec2(full.width(), item_h));
            if rail_item(ui, pal, r, icon, label, on, false) && on {
                req.push(Request::Options);
            }
            y += item_h + 2.0;
        }
    }

    /// Onglet Calibrage : en attendant les assistants (t026g), la prise du zéro table à la main.
    fn calibrage_ui(&mut self, ui: &mut Ui, pal: &Palette, req: &mut Vec<Request>) {
        ScrollArea::vertical().id_salt("mach-calib").auto_shrink([false, false]).show(ui, |ui| {
            ui.set_max_width(820.0);
            widgets::card(ui, pal, Some("Zéro table (en attendant les fins de course)"), |ui| {
                ui.add(egui::Label::new(RichText::new("Amener l'outil au coin avant gauche du martyr : X et Y au laser croix, Z à la feuille de papier sur le dessus du martyr. La position actuelle devient alors le zéro table pour la séance ; il se perd après une alarme, un arrêt, une réinitialisation ou une coupure de liaison.").size(13.0).color(pal.text)).wrap());
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let on = self.connected() && self.run != Run::Running && self.rights().edit_fiche;
                    if widgets::button(ui, pal, Some(Icon::Zero), "Zéro table ici", Kind::Component, on).clicked() {
                        self.command("machine.zero_table", req);
                    }
                    ui.label(RichText::new(format!("État : {}", self.reference.label())).color(pal.weak));
                });
            });
            ui.add_space(12.0);
            soon(ui, pal, "Procédures avec assistant (à venir)", &[
                "Test des entrées : arrêt d'urgence, fins de course, sonde",
                "Décalage laser / outil",
                "Zéro table guidé, vérification des 3 autres coins",
                "Pas par mm",
                "Équerrage X / Y (correction appliquée d'office)",
                "Planéité et surfaçage du martyr",
                "Perpendicularité de la broche (mesure notée)",
                "Vérification de l'épaisseur de la sonde",
            ]);
        });
    }

    fn confirm_window(&mut self, ctx: &egui::Context, pal: &Palette) {
        let Some(what) = self.confirm else { return };
        let (title, text, hint, ok) = match what {
            Confirm::Program => {
                let n = self.program.as_ref().map_or(0, |p| p.text.lines.len());
                ("Envoyer à la machine ?", format!("{n} lignes vont être envoyées à {} : la machine va bouger.", self.port), "Vérifier le zéro pièce, le brut bridé et la zone dégagée.".to_owned(), "Envoyer")
            }
            Confirm::Spindle => ("Mettre la broche en marche ?", format!("M3 à {:.0} tr/min sur {}.", self.rpm.min(self.rpm_max()), self.port), "Outil serré, mains et objets à l'écart, lunettes.".to_owned(), "Mettre en marche"),
            Confirm::Jog { d, absolute, n, of } => {
                let t = if absolute { "Retour au zéro pièce en X et Y".to_owned() } else { format!("Déplacement de {:.1} mm", d.iter().map(|v| v * v).sum::<f64>().sqrt()) };
                ("Déplacement sans zéro valable", format!("{t} alors que la position machine n'est pas référencée (validation {n} / {of})."), "La machine ne connaît pas ses limites : vérifier qu'il y a la place. La validation vaut pour la séance.".to_owned(), "Valider")
            }
        };
        let mut go = false;
        let mut cancel = false;
        egui::Window::new(title).collapsible(false).resizable(false).fixed_size(vec2(440.0, 150.0)).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
            ui.add(egui::Label::new(text).wrap());
            ui.add(egui::Label::new(RichText::new(hint).color(pal.weak)).wrap());
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                go = widgets::button(ui, pal, Some(Icon::Ok), ok, Kind::Component, true).clicked();
                cancel = widgets::button(ui, pal, None, "Annuler", Kind::Plain, true).clicked();
            });
        });
        if cancel {
            self.confirm = None;
        } else if go {
            self.confirm = None;
            match what {
                Confirm::Program => self.start(),
                Confirm::Spindle => self.spindle_on(),
                Confirm::Jog { d, absolute, n, of } => {
                    self.jog_ok = n;
                    if n < of {
                        self.confirm = Some(Confirm::Jog { d, absolute, n: n + 1, of });
                    } else {
                        self.do_jog(d, absolute);
                    }
                }
            }
        }
    }
}

/// Nombre entier avec espace des milliers (20 000).
fn thousands(v: f64) -> String {
    let d = format!("{:.0}", v.abs());
    let mut out = String::new();
    for (i, c) in d.chars().enumerate() {
        if i > 0 && (d.len() - i) % 3 == 0 {
            out.push('\u{a0}');
        }
        out.push(c);
    }
    if v < 0.0 { format!("-{out}") } else { out }
}

/// Largeur du rail de gauche.
const RAIL_W: f32 = 86.0;

/// Élément du rail : icône au trait, libellé dessous ; renvoie `true` au clic.
fn rail_item(ui: &mut Ui, pal: &Palette, r: Rect, icon: Icon, label: &str, enabled: bool, active: bool) -> bool {
    let resp = ui.interact(r, ui.make_persistent_id(("rail", label)), if enabled { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, enabled, active, label));
    let resp = if enabled { resp } else { resp.on_hover_text("à venir") };
    let p = ui.painter();
    if active {
        p.rect_filled(r.shrink2(vec2(6.0, 2.0)), CornerRadius::same(8), theme::alpha(pal.orange, 0.14));
        p.line_segment([pos2(r.left() + 3.0, r.top() + 10.0), pos2(r.left() + 3.0, r.bottom() - 10.0)], Stroke::new(3.0, pal.orange));
    } else if enabled && resp.hovered() {
        p.rect_filled(r.shrink2(vec2(6.0, 2.0)), CornerRadius::same(8), pal.hover);
    }
    let c = if active { pal.orange } else if enabled { pal.text } else { pal.dim };
    crate::icons::paint(p, Rect::from_center_size(pos2(r.center().x, r.top() + 22.0), vec2(26.0, 26.0)), icon, c);
    let g = p.layout(label.to_owned(), theme::regular(11.0), c, r.width() - 8.0);
    let gw = g.size().x;
    p.galley(pos2(r.center().x - gw / 2.0, r.top() + 40.0), g, c);
    resp.clicked()
}

/// Carte « à venir » : ce que l'onglet contiendra.
fn soon(ui: &mut Ui, pal: &Palette, title: &str, items: &[&str]) {
    widgets::card(ui, pal, Some(title), |ui| {
        ui.label(RichText::new("Cet onglet arrive dans une prochaine étape. Il contiendra :").size(13.0).color(pal.weak));
        ui.add_space(4.0);
        for i in items {
            widgets::status_line(ui, pal, Icon::Soon, pal.weak, i);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait_until(m: &mut Machines, ctx: &egui::Context, what: impl Fn(&Machines) -> bool) -> bool {
        for _ in 0..200 {
            m.poll(ctx);
            if what(m) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        false
    }

    #[test]
    fn emulator_session_runs_a_program() {
        let ctx = egui::Context::default();
        let mut m = Machines::default();
        assert!(m.emulator, "l'émulateur doit être le choix par défaut");
        assert!(m.connect().is_ok());
        assert!(wait_until(&mut m, &ctx, |m| m.status.state == MachineState::Idle), "pas d'état Idle");
        let dir = std::env::temp_dir().join("makercraft-test-prog");
        let _ = std::fs::create_dir_all(&dir);
        let f = dir.join("carre.nc");
        let _ = std::fs::write(&f, "G21 G90\nG0 X0 Y0\n\n(carré de 10 mm)\nG1 X10 F600\nG1 Y10\nG1 X0\nG1 Y0\nM2\n");
        assert_eq!(m.load_program(&f).ok().as_deref(), Some("7 ligne(s) chargée(s)"));
        m.start();
        assert!(wait_until(&mut m, &ctx, |m| m.run == Run::Finished), "programme non terminé : {:?}", m.progress);
        assert_eq!(m.progress.acked, 7);
        assert_eq!(m.progress.errors, 0);
        m.disconnect();
        assert!(!m.connected());
    }

    #[test]
    fn real_port_requires_a_port_name() {
        let mut m = Machines { emulator: false, ..Machines::default() };
        assert!(m.connect().is_err());
        assert!(!m.connected());
    }

    #[test]
    fn long_jog_without_zero_asks_twice_on_a_real_machine() {
        let ctx = egui::Context::default();
        let mut m = Machines::default();
        assert!(m.connect().is_ok());
        assert!(wait_until(&mut m, &ctx, |m| m.status.state == MachineState::Idle));
        // émulateur : aucune validation
        m.jog([10.0, 0.0, 0.0], false);
        assert!(m.confirm.is_none());
        // même liaison vue comme une vraie machine (la garde ne regarde que `emulator`)
        m.emulator = false;
        m.jog([0.5, 0.0, 0.0], false);
        assert!(m.confirm.is_none(), "moins de 1 mm : libre");
        m.jog([10.0, 0.0, 0.0], false);
        assert_eq!(m.confirm, Some(Confirm::Jog { d: [10.0, 0.0, 0.0], absolute: false, n: 1, of: 2 }));
        m.confirm = None;
        m.jog_ok = 1;
        m.jog([3.0, 0.0, 0.0], false);
        assert!(m.confirm.is_none(), "1re validation : jusqu'à 5 mm");
        m.reference = Reference::TableZero;
        m.jog([50.0, 0.0, 0.0], false);
        assert!(m.confirm.is_none(), "zéro table valable : libre");
        m.command("machine.arret", &mut Vec::new());
        assert_eq!(m.reference, Reference::None, "l'arrêt invalide le zéro table");
        assert_eq!(m.jog_ok, 0);
        m.disconnect();
    }

    #[test]
    fn pupil_mode_blocks_spindle_unlock_and_expert_tabs() {
        let mut m = Machines { mode: Mode::Eleve, ..Machines::default() };
        assert!(m.connect().is_ok());
        m.command("machine.broche.marche", &mut Vec::new());
        assert!(!m.log.iter().any(|l| l.contains("Broche en marche")), "broche interdite en mode élève");
        m.command("machine.deverrouiller", &mut Vec::new());
        m.command("machine.reset", &mut Vec::new());
        assert!(!m.log.iter().any(|l| l.contains("Réinitialisation")));
        assert!(!m.rights().expert_tabs);
        assert!(Page::Reglages.expert() && !Page::Pilotage.expert());
        m.disconnect();
    }

    #[test]
    fn homing_needs_declared_limit_switches() {
        let mut m = Machines::default();
        assert!(m.connect().is_ok());
        m.command("machine.origine", &mut Vec::new());
        assert!(!m.homing, "sans fins de course : $H refusé");
        m.equipment.limits = [true; 3];
        m.command("machine.origine", &mut Vec::new());
        assert!(m.homing);
        m.disconnect();
    }

    #[test]
    fn thousands_separator() {
        assert_eq!(thousands(20_000.0), "20\u{a0}000");
        assert_eq!(thousands(775.0), "775");
        assert_eq!(thousands(1_234_567.0), "1\u{a0}234\u{a0}567");
    }

    #[test]
    fn tool_change_pause_reminds_to_redo_z() {
        let m = Machines { pause_note: Some("Changer outil T2 : fraise plate 3.175 - pince ER11 3.175".into()), ..Machines::default() };
        let (alarm, t) = m.banner().unwrap_or_default();
        assert!(!alarm);
        assert!(t.contains("refaire le Z"));
    }
}
