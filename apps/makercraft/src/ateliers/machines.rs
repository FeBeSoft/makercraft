//! Atelier Machines : pilotage d'une machine Grbl 1.1 / grblHAL par l'extension `machine`
//! (port série USB ou émulateur intégré) : connexion, état et position, déplacement manuel,
//! envoi d'un programme .nc avec progression, pause, reprise et arrêt.
//!
//! Sûreté : l'émulateur est choisi par défaut ; avant d'envoyer un programme à une vraie
//! machine, une confirmation est demandée.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::Duration;

use eframe::egui::{self, Key, RichText, ScrollArea, Sense, TextEdit, Ui, UiBuilder, pos2, vec2};
use makercraft_ext_machine::protocol::prepare_line;
use makercraft_ext_machine::transport::{PortInfo, list_ports};
use makercraft_ext_machine::{Controller, ControllerOptions, Event, MachineState, MachineStatus, Progress, TransportConfig};

use crate::icons::Icon;
use crate::prefs::Units;
use crate::theme::{self, Palette};
use crate::widgets::{self, Kind};

/// Vitesses série proposées.
pub const BAUDS: [u32; 6] = [9_600, 19_200, 38_400, 57_600, 115_200, 250_000];
/// Pas de déplacement manuel proposés (mm).
pub const STEPS: [f64; 4] = [0.1, 1.0, 10.0, 50.0];
const LOG_MAX: usize = 400;

/// Ce que l'atelier demande à l'application.
pub enum Request {
    Status(String),
    /// Mémoriser le port et la vitesse choisis.
    Port(String, u32),
}

struct Program {
    path: PathBuf,
    lines: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Run {
    Idle,
    Running,
    Finished,
    Aborted,
}

pub struct Machines {
    ports: Vec<PortInfo>,
    pub port: String,
    pub baud: u32,
    /// Émulateur intégré (aucune machine réelle).
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
    step: f64,
    feed: f64,
    /// Confirmation demandée avant l'envoi à une vraie machine.
    confirm: bool,
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
            step: 1.0,
            feed: 600.0,
            confirm: false,
        }
    }
}

/// Lit un programme G-code : seules les lignes que le contrôleur enverra sont gardées (sans les
/// commentaires ni les lignes vides), pour que le compte affiché soit celui de l'envoi.
pub fn read_program(path: &Path) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{} : {e}", path.display()))?;
    Ok(text.lines().filter(|l| prepare_line(l).is_some()).map(str::to_owned).collect())
}

impl Machines {
    pub fn with_port(port: String, baud: u32) -> Self {
        Self { port, baud, ..Self::default() }
    }

    pub fn connected(&self) -> bool {
        self.ctrl.is_some()
    }

    pub fn state_label(&self) -> &'static str {
        if self.connected() { self.status.state.label() } else { "déconnectée" }
    }

    fn log(&mut self, s: impl Into<String>) {
        self.log.push_back(s.into());
        while self.log.len() > LOG_MAX {
            self.log.pop_front();
        }
    }

    pub fn refresh_ports(&mut self) {
        match list_ports() {
            Ok(p) => self.ports = p,
            Err(e) => self.log(format!("Ports série : {e}")),
        }
        if self.port.is_empty()
            && let Some(p) = self.ports.first()
        {
            self.port = p.name.clone();
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
                Event::Status(s) => self.status = s,
                Event::Sent { .. } => {}
                Event::Ack { line, result: Err(code), .. } => {
                    let d = makercraft_ext_machine::codes::error(code).map(|c| c.brief).unwrap_or("erreur inconnue");
                    self.log(format!("error:{code} ({d}) sur « {line} »"));
                }
                Event::Ack { .. } => {}
                Event::Alarm(code) => {
                    let d = makercraft_ext_machine::codes::alarm(code).map(|c| c.brief).unwrap_or("alarme");
                    self.log(format!("ALARME {code} : {d}"));
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
        }
        if self.ctrl.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    pub fn load_program(&mut self, path: &Path) -> Result<String, String> {
        let lines = read_program(path)?;
        let n = lines.len();
        self.program = Some(Program { path: path.to_path_buf(), lines });
        self.progress = Progress { total: n, ..Progress::default() };
        self.run = Run::Idle;
        Ok(format!("{n} ligne(s) chargée(s)"))
    }

    fn start(&mut self) {
        let (Some(c), Some(p)) = (&self.ctrl, &self.program) else { return };
        c.run_program(p.lines.clone());
        self.progress = Progress { total: p.lines.len(), ..Progress::default() };
        self.run = Run::Running;
        self.paused = false;
        let name = p.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.log(format!("Envoi de {name}"));
    }

    pub fn ui(&mut self, ui: &mut Ui, pal: &Palette, units: Units, req: &mut Vec<Request>) {
        let full = ui.available_rect_before_wrap();
        ui.allocate_rect(full, Sense::hover());
        let side_w = 360.0_f32.min(full.width() * 0.42);
        let side = egui::Rect::from_min_size(full.min, vec2(side_w, full.height()));
        let main = egui::Rect::from_min_max(pos2(side.right() + 16.0, full.top()), full.max);
        let mut s = ui.new_child(UiBuilder::new().max_rect(side));
        ScrollArea::vertical().id_salt("machines-side").auto_shrink([false, false]).show(&mut s, |ui| {
            ui.set_max_width(side_w - 14.0);
            ui.spacing_mut().item_spacing.y = 14.0;
            self.connection_card(ui, pal, req);
            self.state_card(ui, pal, units);
            self.jog_card(ui, pal);
        });
        let mut m = ui.new_child(UiBuilder::new().max_rect(main));
        m.spacing_mut().item_spacing.y = 14.0;
        self.program_card(&mut m, pal, req);
        self.console_card(&mut m, pal);
        if self.confirm {
            self.confirm_window(ui.ctx(), pal);
        }
    }

    fn connection_card(&mut self, ui: &mut Ui, pal: &Palette, req: &mut Vec<Request>) {
        widgets::card(ui, pal, Some("Connexion"), |ui| {
            let busy = self.connected();
            ui.add_enabled_ui(!busy, |ui| {
                ui.radio_value(&mut self.emulator, true, "Émulateur (essai sans machine)");
                ui.radio_value(&mut self.emulator, false, "Port série USB");
                if !self.emulator {
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("port").width(150.0).selected_text(if self.port.is_empty() { "aucun" } else { &self.port }).show_ui(ui, |ui| {
                            for p in &self.ports {
                                ui.selectable_value(&mut self.port, p.name.clone(), format!("{}  {}", p.name, p.description));
                            }
                        });
                        if widgets::button(ui, pal, None, "Actualiser", Kind::Plain, true).clicked() {
                            self.refresh_ports();
                        }
                    });
                    egui::ComboBox::from_id_salt("baud").width(150.0).selected_text(format!("{} bauds", self.baud)).show_ui(ui, |ui| {
                        for b in BAUDS {
                            ui.selectable_value(&mut self.baud, b, format!("{b} bauds"));
                        }
                    });
                }
            });
            ui.add_space(4.0);
            if busy {
                if widgets::button(ui, pal, Some(Icon::Unplug), "Déconnecter", Kind::Plain, true).clicked() {
                    self.disconnect();
                    req.push(Request::Status("Machine déconnectée".into()));
                }
            } else if widgets::button(ui, pal, Some(Icon::Plug), "Connecter", Kind::Component, true).clicked() {
                let r = self.connect();
                if r.is_ok() && !self.emulator {
                    req.push(Request::Port(self.port.clone(), self.baud));
                }
                req.push(Request::Status(r.unwrap_or_else(|e| format!("Connexion impossible : {e}"))));
            }
        });
    }

    fn state_card(&self, ui: &mut Ui, pal: &Palette, units: Units) {
        widgets::card(ui, pal, Some("État"), |ui| {
            let st = self.status.state;
            let color = match st {
                _ if !self.connected() => pal.weak,
                MachineState::Idle => pal.ok,
                MachineState::Run | MachineState::Jog | MachineState::Home => pal.blue,
                MachineState::Hold | MachineState::Door | MachineState::Check | MachineState::Tool | MachineState::Sleep => pal.warn,
                MachineState::Alarm => pal.bad,
                _ => pal.weak,
            };
            ui.horizontal(|ui| {
                widgets::pill(ui, self.state_label(), color, pal.on_blue);
                if let Some(f) = &self.firmware {
                    ui.label(RichText::new(f).size(12.5).color(pal.weak));
                }
            });
            egui::Grid::new("pos").num_columns(3).spacing(vec2(18.0, 6.0)).show(ui, |ui| {
                ui.label("");
                ui.label(RichText::new("Pièce").size(12.0).color(pal.weak));
                ui.label(RichText::new("Machine").size(12.0).color(pal.weak));
                ui.end_row();
                for (axis, w, m) in [("X", self.status.wpos.x, self.status.mpos.x), ("Y", self.status.wpos.y, self.status.mpos.y), ("Z", self.status.wpos.z, self.status.mpos.z)] {
                    ui.label(RichText::new(axis).font(theme::bold(14.0)).color(pal.orange));
                    ui.label(RichText::new(units.fmt(w)).font(theme::mono(14.0)));
                    ui.label(RichText::new(units.fmt(m)).font(theme::mono(13.0)).color(pal.weak));
                    ui.end_row();
                }
            });
            ui.label(RichText::new(format!("Avance {:.0} mm/min   Broche {:.0} tr/min", self.status.feed, self.status.spindle)).size(12.5).color(pal.weak));
        });
    }

    fn jog_card(&mut self, ui: &mut Ui, pal: &Palette) {
        widgets::card(ui, pal, Some("Déplacement manuel"), |ui| {
            let on = self.connected() && self.run != Run::Running;
            ui.horizontal(|ui| {
                widgets::caption(ui, pal, "Pas");
                for s in STEPS {
                    let label = if s < 1.0 { format!("{s}") } else { format!("{s:.0}") };
                    ui.selectable_value(&mut self.step, s, format!("{label} mm"));
                }
            });
            ui.horizontal(|ui| {
                widgets::caption(ui, pal, "Avance");
                ui.add(egui::DragValue::new(&mut self.feed).range(10.0..=10_000.0).speed(10.0).suffix(" mm/min"));
            });
            let mut jog = None;
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(44.0);
                        if widgets::button(ui, pal, Some(Icon::Up), "", Kind::Plain, on).on_hover_text("Y+").clicked() {
                            jog = Some((0.0, 1.0, 0.0));
                        }
                    });
                    ui.horizontal(|ui| {
                        if widgets::button(ui, pal, Some(Icon::Left), "", Kind::Plain, on).on_hover_text("X-").clicked() {
                            jog = Some((-1.0, 0.0, 0.0));
                        }
                        ui.add_space(36.0);
                        if widgets::button(ui, pal, Some(Icon::Right), "", Kind::Plain, on).on_hover_text("X+").clicked() {
                            jog = Some((1.0, 0.0, 0.0));
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(44.0);
                        if widgets::button(ui, pal, Some(Icon::Down), "", Kind::Plain, on).on_hover_text("Y-").clicked() {
                            jog = Some((0.0, -1.0, 0.0));
                        }
                    });
                });
                ui.add_space(18.0);
                ui.vertical(|ui| {
                    if widgets::button(ui, pal, Some(Icon::Up), "Z+", Kind::Plain, on).clicked() {
                        jog = Some((0.0, 0.0, 1.0));
                    }
                    ui.add_space(30.0);
                    if widgets::button(ui, pal, Some(Icon::Down), "Z-", Kind::Plain, on).clicked() {
                        jog = Some((0.0, 0.0, -1.0));
                    }
                });
            });
            if let (Some((x, y, z)), Some(c)) = (jog, &self.ctrl) {
                c.jog(x * self.step, y * self.step, z * self.step, self.feed);
            }
            ui.horizontal_wrapped(|ui| {
                if widgets::button(ui, pal, Some(Icon::Home), "Origine ($H)", Kind::Plain, on).clicked()
                    && let Some(c) = &self.ctrl
                {
                    c.home();
                }
                if widgets::button(ui, pal, Some(Icon::Unlock), "Déverrouiller ($X)", Kind::Plain, on).clicked()
                    && let Some(c) = &self.ctrl
                {
                    c.unlock();
                }
                if widgets::button(ui, pal, Some(Icon::Zero), "Zéro pièce XYZ", Kind::Plain, on).clicked()
                    && let Some(c) = &self.ctrl
                {
                    c.set_work_zero(true, true, true);
                }
            });
        });
    }

    fn program_card(&mut self, ui: &mut Ui, pal: &Palette, req: &mut Vec<Request>) {
        widgets::card(ui, pal, Some("Programme"), |ui| {
            ui.horizontal(|ui| {
                let can_load = self.run != Run::Running;
                if widgets::button(ui, pal, Some(Icon::Open), "Ouvrir un .nc...", Kind::Plain, can_load).clicked()
                    && let Some(p) = rfd::FileDialog::new().add_filter("G-code", &["nc", "gcode", "ngc", "tap", "txt"]).pick_file()
                {
                    req.push(Request::Status(self.load_program(&p).unwrap_or_else(|e| format!("Programme illisible : {e}"))));
                }
                match &self.program {
                    Some(p) => {
                        let name = p.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        ui.label(RichText::new(name).font(theme::mono(13.5)));
                        ui.label(RichText::new(format!("{} lignes", p.lines.len())).color(pal.weak));
                    }
                    None => {
                        ui.label(RichText::new("aucun programme chargé").color(pal.weak));
                    }
                }
            });
            let pr = self.progress;
            let frac = if pr.total == 0 { 0.0 } else { pr.acked as f32 / pr.total as f32 };
            let text = match self.run {
                Run::Idle => format!("prêt  -  {} ligne(s)", pr.total),
                Run::Running if self.paused => format!("en pause  -  {} / {}", pr.acked, pr.total),
                Run::Running => format!("{:.1} %  -  {} / {}", pr.percent(), pr.acked, pr.total),
                Run::Finished => format!("terminé  -  {} erreur(s)", pr.errors),
                Run::Aborted => format!("arrêté à la ligne {}", pr.acked),
            };
            widgets::progress(ui, pal, frac, &text);
            let connected = self.connected();
            let running = self.run == Run::Running;
            ui.horizontal(|ui| {
                if widgets::button(ui, pal, Some(Icon::Play), "Démarrer", Kind::Component, connected && !running && self.program.is_some()).clicked() {
                    if self.emulator {
                        self.start();
                    } else {
                        self.confirm = true;
                    }
                }
                if widgets::button(ui, pal, Some(Icon::Pause), "Pause", Kind::Plain, running && !self.paused).clicked()
                    && let Some(c) = &self.ctrl
                {
                    c.hold();
                    self.paused = true;
                }
                if widgets::button(ui, pal, Some(Icon::Play), "Reprise", Kind::Plain, running && self.paused).clicked()
                    && let Some(c) = &self.ctrl
                {
                    c.cycle_start();
                    self.paused = false;
                }
                if widgets::button(ui, pal, Some(Icon::Stop), "Arrêt", Kind::Danger, connected).on_hover_text("Arrêt immédiat : pause puis reset de la carte").clicked()
                    && let Some(c) = &self.ctrl
                {
                    c.emergency_stop();
                    if running {
                        self.run = Run::Aborted;
                    }
                    self.paused = false;
                    self.log("Arrêt demandé (pause + reset)");
                }
            });
        });
    }

    fn console_card(&mut self, ui: &mut Ui, pal: &Palette) {
        widgets::card(ui, pal, Some("Console"), |ui| {
            let h = (ui.available_height() - 60.0).max(80.0);
            ScrollArea::vertical().id_salt("console").max_height(h).stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
                for l in &self.log {
                    ui.label(RichText::new(l).font(theme::mono(12.5)));
                }
            });
            ui.horizontal(|ui| {
                let on = self.connected();
                let r = ui.add_enabled(on, TextEdit::singleline(&mut self.input).hint_text("G-code ou commande $, Entrée pour envoyer").desired_width(ui.available_width() - 110.0));
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                let click = widgets::button(ui, pal, None, "Envoyer", Kind::Plain, on).clicked();
                if (enter || click) && !self.input.trim().is_empty() {
                    let line = self.input.trim().to_owned();
                    if let Some(c) = &self.ctrl {
                        c.send(line.clone());
                    }
                    self.log(format!("> {line}"));
                    self.input.clear();
                    r.request_focus();
                }
            });
        });
    }

    fn confirm_window(&mut self, ctx: &egui::Context, pal: &Palette) {
        let n = self.program.as_ref().map_or(0, |p| p.lines.len());
        let mut go = false;
        let mut cancel = false;
        egui::Window::new("Envoyer à la machine ?").collapsible(false).resizable(false).fixed_size(vec2(420.0, 160.0)).anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
            ui.label(format!("{n} lignes vont être envoyées à {} : la machine va bouger.", self.port));
            ui.label(RichText::new("Vérifier le zéro pièce, le brut bridé et la zone dégagée.").color(pal.weak));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                go = widgets::button(ui, pal, Some(Icon::Play), "Envoyer", Kind::Component, true).clicked();
                cancel = widgets::button(ui, pal, None, "Annuler", Kind::Plain, true).clicked();
            });
        });
        if go {
            self.confirm = false;
            self.start();
        } else if cancel {
            self.confirm = false;
        }
    }
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
}
