//! Contrôleur Grbl : un thread qui possède le transport, reçoit des [`Command`] et publie des
//! [`Event`]. L'interface ne bloque jamais l'envoi (recommandation de UGS `ICommunicator`).
//!
//! Porté de LaserGRBL 7.14.1 `LaserGRBL/Core/GrblCore.cs` : `OpenCom`/`StartTX` (reset à la
//! connexion, délai de connexion), `ThreadTX`/`ThreadRX`, `QueryPosition`, `ManageRealTimeStatus`
//! (positions, WCO mémorisé), `DetectHang` (machine muette), `HandleMissingOK` (tampon coincé),
//! `OnStartupMessage` (reset inattendu), `AbortProgram` (M5), `JogToDirection` (`$J=`).
//! Actions : UGS 2.1.27 `IController.java`.

pub mod stream;

use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use makercraft_geom::Vec3;

use crate::protocol::{FirmwareInfo, MachineState, Response, StatusReport, parse_opt, rt};
use crate::transport::{Transport, TransportConfig, TransportError};
use stream::Streamer;
pub use stream::{OnError, Origin, Progress, StreamMode};

/// Réglages du contrôleur.
#[derive(Debug, Clone)]
pub struct ControllerOptions {
    /// Intervalle des demandes d'état `?` (LaserGRBL « Fast » : 500 ms).
    pub status_interval: Duration,
    /// Envoyer un reset logiciel à la connexion (LaserGRBL « Reset Grbl On Connect »).
    pub reset_on_connect: bool,
    /// Délai maximal pour que la machine se manifeste.
    pub connect_timeout: Duration,
    pub mode: StreamMode,
    pub on_error: OnError,
    /// Interroger `$I` après le message de bienvenue (taille réelle du tampon).
    pub query_info: bool,
}

impl Default for ControllerOptions {
    fn default() -> Self {
        Self {
            status_interval: Duration::from_millis(500),
            reset_on_connect: true,
            connect_timeout: Duration::from_secs(10),
            mode: StreamMode::Buffered,
            on_error: OnError::Continue,
            query_info: true,
        }
    }
}

/// État consolidé de la machine (le WCO n'arrive pas dans chaque rapport : on le mémorise).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MachineStatus {
    pub state: MachineState,
    pub mpos: Vec3,
    pub wpos: Vec3,
    pub wco: Vec3,
    pub feed: f64,
    pub spindle: f64,
    /// Avance, rapides, broche/laser (%).
    pub overrides: (u32, u32, u32),
    pub buffer: Option<(u32, u32)>,
}

/// Problèmes détectés pendant un travail (LaserGRBL `DetectedIssue`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Issue {
    /// Plus de réponse aux demandes d'état pendant le travail.
    StopResponding,
    /// La carte a redémarré pendant le travail (alimentation, câble, bruit).
    UnexpectedReset,
    /// Alarme pendant le travail : l'envoi du programme est arrêté.
    MachineAlarm,
    /// Des « ok » se sont perdus ; l'envoi a été débloqué.
    BufferStuckRecovered,
}

impl Issue {
    pub fn label(self) -> &'static str {
        match self {
            Self::StopResponding => "la machine ne répond plus",
            Self::UnexpectedReset => "redémarrage inattendu de la carte",
            Self::MachineAlarm => "alarme pendant le travail",
            Self::BufferStuckRecovered => "réponses perdues, envoi débloqué",
        }
    }
}

/// Ordres envoyés au thread du contrôleur.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Ligne G-code ou commande `$` (file manuelle, prioritaire sur le programme).
    Send(String),
    /// Octet temps réel, écrit immédiatement.
    Realtime(u8),
    /// Charge et démarre un programme.
    Run(Vec<String>),
    /// Reprend l'envoi du programme suspendu (après erreur en mode `OnError::Pause`).
    ResumeStream,
    /// Abandonne le reste du programme et éteint le laser/la broche (`M5`).
    Abort,
    SetMode(StreamMode),
    Disconnect,
}

/// Ce que le contrôleur publie.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Connected {
        transport: String,
    },
    Firmware(FirmwareInfo),
    Status(MachineStatus),
    /// Ligne écrite vers la machine.
    Sent {
        id: u64,
        line: String,
        origin: Origin,
    },
    /// Réponse `ok`/`error:N` associée à sa ligne.
    Ack {
        id: u64,
        line: String,
        origin: Origin,
        result: Result<(), u16>,
    },
    Alarm(u16),
    /// Toute autre ligne reçue (messages `[MSG:…]`, réglages `$n=…`, bienvenue…), pour la console.
    Line(String),
    Progress(Progress),
    ProgramFinished(Progress),
    ProgramAborted(Progress),
    Issue(Issue),
    Disconnected {
        reason: Option<String>,
    },
}

/// Poignée sur un contrôleur en marche. Le fermer (ou le lâcher) déconnecte proprement.
pub struct Controller {
    tx: Sender<Command>,
    events: Receiver<Event>,
    thread: Option<JoinHandle<()>>,
}

impl Controller {
    /// Ouvre le transport et démarre le contrôleur.
    pub fn connect(config: &TransportConfig, opts: ControllerOptions) -> Result<Self, TransportError> {
        Ok(Self::with_transport(config.open()?, opts))
    }

    /// Démarre sur un transport déjà ouvert (tests, transports sur mesure).
    pub fn with_transport(transport: Box<dyn Transport>, opts: ControllerOptions) -> Self {
        let (tx, rx_cmd) = mpsc::channel();
        let (tx_ev, events) = mpsc::channel();
        let thread =
            std::thread::Builder::new().name("makercraft-controller".into()).spawn(move || Worker::new(transport, opts, rx_cmd, tx_ev).run()).ok();
        Self { tx, events, thread }
    }

    /// File des événements (à vider régulièrement par l'interface).
    pub fn events(&self) -> &Receiver<Event> {
        &self.events
    }

    /// Attend le prochain événement.
    pub fn next_event(&self, timeout: Duration) -> Option<Event> {
        self.events.recv_timeout(timeout).ok()
    }

    pub fn command(&self, cmd: Command) {
        let _ = self.tx.send(cmd);
    }

    pub fn send(&self, line: impl Into<String>) {
        self.command(Command::Send(line.into()));
    }

    pub fn realtime(&self, b: u8) {
        self.command(Command::Realtime(b));
    }

    pub fn run_program(&self, lines: Vec<String>) {
        self.command(Command::Run(lines));
    }

    /// Pause immédiate (`!`) : la machine ralentit et s'arrête, le programme reste chargé.
    pub fn hold(&self) {
        self.realtime(rt::FEED_HOLD);
    }

    /// Reprise après pause (`~`).
    pub fn cycle_start(&self) {
        self.realtime(rt::CYCLE_START);
    }

    /// Reset logiciel (Ctrl-X) : arrêt net, tout ce qui était en attente est perdu.
    pub fn soft_reset(&self) {
        self.realtime(rt::SOFT_RESET);
    }

    /// Arrêt d'urgence logiciel : pause puis reset (comme UGS `cancelSend` pour Grbl).
    pub fn emergency_stop(&self) {
        self.realtime(rt::FEED_HOLD);
        self.realtime(rt::SOFT_RESET);
    }

    /// Abandon en douceur : plus de nouvelles lignes, puis `M5`.
    pub fn abort(&self) {
        self.command(Command::Abort);
    }

    pub fn home(&self) {
        self.send("$H");
    }

    pub fn unlock(&self) {
        self.send("$X");
    }

    /// Déplacement manuel relatif (mm, mm/min) ; un axe à 0 n'est pas déplacé.
    pub fn jog(&self, dx: f64, dy: f64, dz: f64, feed: f64) {
        if let Some(line) = jog_line(dx, dy, dz, feed) {
            self.send(line);
        }
    }

    pub fn jog_cancel(&self) {
        self.realtime(rt::JOG_CANCEL);
    }

    /// Fixe le zéro pièce à la position actuelle sur les axes choisis (`G10 L20 P0`).
    pub fn set_work_zero(&self, x: bool, y: bool, z: bool) {
        let mut line = String::from("G10L20P0");
        for (on, axis) in [(x, 'X'), (y, 'Y'), (z, 'Z')] {
            if on {
                line.push(axis);
                line.push('0');
            }
        }
        if x || y || z {
            self.send(line);
        }
    }

    pub fn disconnect(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        let _ = self.tx.send(Command::Disconnect);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn fmt_num(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" { "0".to_owned() } else { s.to_owned() }
}

/// `$J=G91G21X10Y-5F1000`, ou `None` si aucun déplacement ou avance invalide.
pub fn jog_line(dx: f64, dy: f64, dz: f64, feed: f64) -> Option<String> {
    if !(feed > 0.0 && feed.is_finite()) {
        return None;
    }
    let mut line = String::from("$J=G91G21");
    for (v, axis) in [(dx, 'X'), (dy, 'Y'), (dz, 'Z')] {
        if v != 0.0 && v.is_finite() {
            line.push(axis);
            line.push_str(&fmt_num(v));
        }
    }
    (line.len() > 9).then(|| format!("{line}F{}", fmt_num(feed)))
}

const STUCK_AFTER: Duration = Duration::from_secs(10);
const READ_SLICE: Duration = Duration::from_millis(5);

struct Worker {
    transport: Box<dyn Transport>,
    opts: ControllerOptions,
    commands: Receiver<Command>,
    events: Sender<Event>,
    streamer: Streamer,
    status: MachineStatus,
    connected: bool,
    started: Instant,
    last_query: Option<Instant>,
    last_status: Instant,
    last_activity: Instant,
    hang_reported: bool,
}

impl Worker {
    fn new(transport: Box<dyn Transport>, opts: ControllerOptions, commands: Receiver<Command>, events: Sender<Event>) -> Self {
        let now = Instant::now();
        let streamer = Streamer::new(opts.mode, opts.on_error);
        Self {
            transport,
            opts,
            commands,
            events,
            streamer,
            status: MachineStatus { state: MachineState::Connecting, overrides: (100, 100, 100), ..MachineStatus::default() },
            connected: false,
            started: now,
            last_query: None,
            last_status: now,
            last_activity: now,
            hang_reported: false,
        }
    }

    fn emit(&self, e: Event) {
        let _ = self.events.send(e);
    }

    fn run(mut self) {
        let reason = self.main_loop().err();
        self.transport.close();
        if self.streamer.job_active() {
            self.emit(Event::ProgramAborted(self.streamer.progress()));
        }
        self.emit(Event::Disconnected { reason });
    }

    fn write(&mut self, data: &[u8]) -> Result<(), String> {
        self.transport.write(data).map_err(|e| e.to_string())
    }

    fn main_loop(&mut self) -> Result<(), String> {
        if self.opts.reset_on_connect {
            self.write(&[rt::SOFT_RESET])?;
        }
        loop {
            // 1. ordres de l'interface
            loop {
                match self.commands.try_recv() {
                    Ok(Command::Disconnect) | Err(TryRecvError::Disconnected) => return Ok(()),
                    Ok(cmd) => self.handle_command(cmd)?,
                    Err(TryRecvError::Empty) => break,
                }
            }
            // 2. envoyer tout ce qui tient dans le tampon de la carte
            if self.connected {
                while let Some(q) = self.streamer.next_to_send() {
                    let mut data = q.line.clone().into_bytes();
                    data.push(b'\n');
                    self.write(&data)?;
                    self.emit(Event::Sent { id: q.id, line: q.line, origin: q.origin });
                }
            }
            // 3. demande d'état périodique
            let due = self.last_query.is_none_or(|t| t.elapsed() >= self.opts.status_interval);
            if due {
                self.write(&[rt::STATUS])?;
                self.last_query = Some(Instant::now());
            }
            // 4. réception
            match self.transport.read_line(READ_SLICE) {
                Ok(Some(line)) => self.handle_line(&line),
                Ok(None) => {}
                Err(e) => return Err(e.to_string()),
            }
            // 5. sûreté
            if !self.connected && self.started.elapsed() > self.opts.connect_timeout {
                return Err("la machine ne répond pas (pas de message Grbl)".into());
            }
            self.detect_hang();
            self.detect_stuck_buffer();
        }
    }

    fn handle_command(&mut self, cmd: Command) -> Result<(), String> {
        match cmd {
            Command::Send(line) => {
                self.streamer.push_manual(&line);
            }
            Command::Realtime(b) => {
                self.write(&[b])?;
                if b == rt::SOFT_RESET {
                    self.after_reset();
                }
            }
            Command::Run(lines) => {
                let n = self.streamer.start_program(lines);
                self.hang_reported = false;
                self.emit(Event::Progress(self.streamer.progress()));
                if n == 0 {
                    self.emit(Event::ProgramFinished(self.streamer.progress()));
                }
            }
            Command::ResumeStream => self.streamer.resume(),
            Command::Abort => {
                if self.streamer.job_active() {
                    let p = self.streamer.abort_program();
                    self.streamer.push_manual("M5");
                    self.emit(Event::ProgramAborted(p));
                }
            }
            Command::SetMode(m) => self.streamer.set_mode(m),
            Command::Disconnect => {}
        }
        Ok(())
    }

    /// Après un reset (demandé ou non) : tout ce qui était en attente est perdu.
    fn after_reset(&mut self) {
        if self.streamer.job_active() {
            self.emit(Event::ProgramAborted(self.streamer.progress()));
        }
        self.streamer.reset();
        self.status.overrides = (100, 100, 100);
    }

    fn handle_line(&mut self, line: &str) {
        self.last_activity = Instant::now();
        match Response::parse(line) {
            Response::Ok => self.ack(Ok(()), line),
            Response::Error(code) => self.ack(Err(code), line),
            Response::Status(r) => self.on_status(r),
            Response::Welcome(info) => {
                if self.streamer.job_active() {
                    self.emit(Event::Issue(Issue::UnexpectedReset));
                }
                self.after_reset();
                self.mark_connected();
                self.emit(Event::Line(line.to_owned()));
                if self.opts.query_info && info.supports_v11() {
                    self.streamer.push_manual("$I");
                }
                self.emit(Event::Firmware(info));
            }
            Response::Alarm(code) => {
                if self.streamer.job_active() {
                    // Sécurité : on n'envoie plus rien du programme après une alarme.
                    let p = self.streamer.abort_program();
                    self.emit(Event::Issue(Issue::MachineAlarm));
                    self.emit(Event::ProgramAborted(p));
                }
                self.emit(Event::Alarm(code));
            }
            Response::Feedback(f) => {
                if let Some((_, rx)) = parse_opt(&f) {
                    self.streamer.set_capacity_from_opt(rx as usize);
                }
                self.emit(Event::Line(line.to_owned()));
            }
            Response::Setting { .. } | Response::Other(_) => self.emit(Event::Line(line.to_owned())),
        }
    }

    fn mark_connected(&mut self) {
        if !self.connected {
            self.connected = true;
            self.emit(Event::Connected { transport: self.transport.name() });
        }
    }

    fn ack(&mut self, result: Result<(), u16>, raw: &str) {
        match self.streamer.on_response(result) {
            Some(a) => {
                let is_program = matches!(a.item.origin, Origin::Program(_));
                self.emit(Event::Ack { id: a.item.id, line: a.item.line, origin: a.item.origin, result: a.result });
                if is_program {
                    self.emit(Event::Progress(self.streamer.progress()));
                }
                if a.program_finished {
                    self.emit(Event::ProgramFinished(self.streamer.progress()));
                }
            }
            None => self.emit(Event::Line(raw.to_owned())),
        }
    }

    fn on_status(&mut self, r: StatusReport) {
        self.last_status = Instant::now();
        self.hang_reported = false;
        self.mark_connected();
        let s = &mut self.status;
        // LaserGRBL : Grbl répond « Idle » pendant un G4 (temporisation) au milieu d'un travail.
        s.state = if r.state == MachineState::Idle && self.streamer.job_active() { MachineState::Run } else { r.state };
        if let Some(wco) = r.wco {
            s.wco = wco;
        }
        match (r.mpos, r.wpos) {
            (Some(m), _) => {
                s.mpos = m;
                s.wpos = m - s.wco;
            }
            (None, Some(w)) => {
                s.wpos = w;
                s.mpos = w + s.wco;
            }
            (None, None) => {}
        }
        if let Some(f) = r.feed {
            s.feed = f;
        }
        if let Some(sp) = r.spindle {
            s.spindle = sp;
        }
        if let Some(ov) = r.overrides {
            s.overrides = ov;
        }
        s.buffer = r.buffer;
        if let Some((_, rx_free)) = r.buffer {
            self.streamer.observe_free_rx(rx_free as usize);
        }
        let snapshot = self.status.clone();
        self.emit(Event::Status(snapshot));
    }

    /// LaserGRBL `DetectHang` : plus de rapport d'état depuis 10 intervalles (au moins 5 s).
    fn detect_hang(&mut self) {
        let limit = (self.opts.status_interval * 10).max(Duration::from_secs(5));
        if self.streamer.job_active() && !self.hang_reported && self.last_status.elapsed() > limit {
            self.hang_reported = true;
            self.emit(Event::Issue(Issue::StopResponding));
        }
    }

    /// LaserGRBL `HandleMissingOK` : en cours d'exécution, des lignes attendent une réponse
    /// depuis 10 s alors que la carte dit avoir un tampon de réception vide → « ok » perdus.
    fn detect_stuck_buffer(&mut self) {
        let card_empty = self.status.buffer.is_some_and(|(_, free)| free as usize >= self.streamer.capacity());
        if self.status.state == MachineState::Run && self.streamer.in_flight() > 0 && card_empty && self.last_activity.elapsed() > STUCK_AFTER {
            for a in self.streamer.force_ack_all() {
                if a.program_finished {
                    self.emit(Event::ProgramFinished(self.streamer.progress()));
                }
            }
            self.last_activity = Instant::now();
            self.emit(Event::Issue(Issue::BufferStuckRecovered));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jog_lines() {
        assert_eq!(jog_line(10.0, -5.5, 0.0, 1000.0).as_deref(), Some("$J=G91G21X10Y-5.5F1000"));
        assert_eq!(jog_line(0.0, 0.0, 0.1, 300.0).as_deref(), Some("$J=G91G21Z0.1F300"));
        assert_eq!(jog_line(0.0, 0.0, 0.0, 300.0), None);
        assert_eq!(jog_line(1.0, 0.0, 0.0, 0.0), None);
        assert_eq!(jog_line(f64::NAN, 0.0, 0.0, 100.0), None);
    }
}
