//! Streamer : décide quelle ligne envoyer et quand, sans thread ni entrée/sortie (testable seul).
//!
//! Porté de LaserGRBL 7.14.1 `LaserGRBL/Core/GrblCore.cs` : `CanSend`, `PeekNextCommand`,
//! `HasSpaceInBuffer`, `SendLine`, `ManageCommandResponse`, `EnlargeBuffer`, `StreamingMode`.
//! Organisation des files reprise de UGS 2.1.27 `communicator/ICommunicator.java` : les
//! commandes manuelles passent avant le programme en cours.
//!
//! Comptage de caractères : Grbl a un tampon de réception (127/128 octets par défaut). On garde
//! la somme des longueurs des lignes envoyées et pas encore acquittées (`ok`/`error`) ; une ligne
//! ne part que si elle tient dans la place restante. Exception : une ligne plus longue que le
//! tampon part quand plus rien n'est en attente (sinon l'envoi se bloquerait).

use std::collections::VecDeque;

use crate::protocol::prepare_line;

/// Taille de tampon supposée avant d'en savoir plus (LaserGRBL `DEFAULT_BUFFER_SIZE`).
pub const DEFAULT_RX_BUFFER: usize = 127;

/// Tailles fiables lues dans `Bf:` (LaserGRBL `EnlargeBuffer`) : on ignore toute autre valeur,
/// qui peut venir de bruit sur la ligne.
const KNOWN_RX_SIZES: [usize; 5] = [128, 254, 255, 256, 10240];

/// Nombre maximal de répétitions d'une ligne en erreur (`RepeatOnError`).
const MAX_REPEAT: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StreamMode {
    /// Remplit le tampon de la carte (rapide, mouvements fluides). Par défaut.
    #[default]
    Buffered,
    /// Une ligne à la fois, on attend sa réponse (lent, mais sûr avec une liaison douteuse).
    Synchronous,
    /// Comme `Synchronous`, et une ligne du programme en erreur est renvoyée jusqu'à 3 fois.
    RepeatOnError,
}

/// Que faire quand une ligne du programme répond `error:N`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OnError {
    /// Continuer (comportement de Grbl et de LaserGRBL) ; l'erreur est comptée et signalée.
    #[default]
    Continue,
    /// Suspendre l'envoi du programme jusqu'à `resume()`.
    Pause,
}

/// Provenance d'une ligne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Tapée ou demandée par l'interface (jog, `$H`, console…).
    Manual,
    /// Ligne `n` (à partir de 1) du programme chargé.
    Program(usize),
}

/// Une ligne prête à partir, ou partie et en attente de réponse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Queued {
    pub id: u64,
    /// Ligne préparée, sans `\n`.
    pub line: String,
    pub origin: Origin,
    pub repeat: u8,
}

impl Queued {
    /// Octets occupés dans le tampon de la carte (ligne + `\n`).
    pub fn wire_len(&self) -> usize {
        self.line.len() + 1
    }
}

/// Avancement du programme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Progress {
    pub total: usize,
    pub sent: usize,
    pub acked: usize,
    pub errors: usize,
}

impl Progress {
    pub fn percent(&self) -> f64 {
        if self.total == 0 { 100.0 } else { self.acked as f64 * 100.0 / self.total as f64 }
    }
}

/// Résultat d'un acquittement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ack {
    pub item: Queued,
    /// `Err(code)` pour `error:code`.
    pub result: Result<(), u16>,
    /// Vrai quand cet acquittement termine le programme.
    pub program_finished: bool,
    /// Vrai si la ligne en erreur va être renvoyée.
    pub will_repeat: bool,
}

#[derive(Debug, Default)]
pub struct Streamer {
    mode: StreamMode,
    on_error: OnError,
    capacity: usize,
    capacity_confirmed: bool,
    manual: VecDeque<Queued>,
    program: VecDeque<Queued>,
    in_flight: VecDeque<Queued>,
    used: usize,
    paused: bool,
    job_active: bool,
    progress: Progress,
    next_id: u64,
}

impl Streamer {
    pub fn new(mode: StreamMode, on_error: OnError) -> Self {
        Self { mode, on_error, capacity: DEFAULT_RX_BUFFER, ..Self::default() }
    }

    pub fn mode(&self) -> StreamMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: StreamMode) {
        self.mode = mode;
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Taille annoncée par `$I` (`[OPT:…,blocs,octets]`) : fiable, appliquée telle quelle.
    pub fn set_capacity_from_opt(&mut self, rx: usize) {
        if rx >= 16 {
            self.capacity = rx;
            self.capacity_confirmed = true;
        }
    }

    /// Place libre lue dans un rapport `Bf:`. Ne sert qu'à découvrir la taille réelle, une fois,
    /// et seulement pour des tailles connues (LaserGRBL `EnlargeBuffer(force: false)`).
    pub fn observe_free_rx(&mut self, free: usize) {
        if !self.capacity_confirmed && self.capacity == DEFAULT_RX_BUFFER && KNOWN_RX_SIZES.contains(&free) {
            self.capacity = free;
        }
    }

    fn make(&mut self, raw: &str, origin: Origin) -> Option<Queued> {
        let line = prepare_line(raw)?;
        self.next_id += 1;
        Some(Queued { id: self.next_id, line, origin, repeat: 0 })
    }

    /// Ajoute une commande manuelle. `None` si la ligne est vide après nettoyage.
    pub fn push_manual(&mut self, raw: &str) -> Option<u64> {
        let q = self.make(raw, Origin::Manual)?;
        let id = q.id;
        self.manual.push_back(q);
        Some(id)
    }

    /// Charge et démarre un programme (remplace un programme non envoyé). Renvoie le nombre de
    /// lignes utiles (commentaires et lignes vides retirés).
    pub fn start_program<S: AsRef<str>>(&mut self, lines: impl IntoIterator<Item = S>) -> usize {
        self.program.clear();
        let items: Vec<Queued> = lines.into_iter().enumerate().filter_map(|(i, l)| self.make(l.as_ref(), Origin::Program(i + 1))).collect();
        self.progress = Progress { total: items.len(), ..Progress::default() };
        self.program.extend(items);
        self.paused = false;
        self.job_active = self.progress.total > 0;
        self.progress.total
    }

    /// Abandonne le reste du programme. Les lignes déjà dans la carte s'exécutent encore : pour
    /// un arrêt immédiat, envoyer `!` puis un reset logiciel.
    pub fn abort_program(&mut self) -> Progress {
        self.program.clear();
        self.job_active = false;
        self.paused = false;
        self.progress
    }

    /// La carte a redémarré (reset) : tout ce qui était en attente est perdu.
    pub fn reset(&mut self) {
        self.manual.clear();
        self.program.clear();
        self.in_flight.clear();
        self.used = 0;
        self.paused = false;
        self.job_active = false;
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn job_active(&self) -> bool {
        self.job_active
    }

    pub fn progress(&self) -> Progress {
        self.progress
    }

    pub fn in_flight(&self) -> usize {
        self.in_flight.len()
    }

    /// Octets envoyés et non acquittés.
    pub fn used(&self) -> usize {
        self.used
    }

    /// Plus rien à envoyer ni à attendre.
    pub fn is_idle(&self) -> bool {
        self.manual.is_empty() && self.program.is_empty() && self.in_flight.is_empty()
    }

    fn fits(&self, q: &Queued) -> bool {
        match self.mode {
            StreamMode::Buffered => self.in_flight.is_empty() || self.used + q.wire_len() <= self.capacity,
            StreamMode::Synchronous | StreamMode::RepeatOnError => self.in_flight.is_empty(),
        }
    }

    /// Prochaine ligne à écrire maintenant, s'il y en a une. Elle passe « en vol ».
    pub fn next_to_send(&mut self) -> Option<Queued> {
        let from_manual = match (self.manual.front(), self.program.front()) {
            (Some(m), _) if self.fits(m) => true,
            (Some(_), _) => return None, // la commande manuelle passe d'abord : on attend la place
            (None, Some(p)) if !self.paused && self.fits(p) => false,
            _ => return None,
        };
        let q = if from_manual { self.manual.pop_front()? } else { self.program.pop_front()? };
        if matches!(q.origin, Origin::Program(_)) && q.repeat == 0 {
            self.progress.sent += 1;
        }
        self.used += q.wire_len();
        self.in_flight.push_back(q.clone());
        Some(q)
    }

    /// Réponse `ok` (`Ok(())`) ou `error:N` (`Err(N)`) à la plus ancienne ligne en vol.
    /// `None` si rien n'était en attente (réponse inattendue).
    pub fn on_response(&mut self, result: Result<(), u16>) -> Option<Ack> {
        let item = self.in_flight.pop_front()?;
        self.used = self.used.saturating_sub(item.wire_len());
        let mut will_repeat = false;
        if let Origin::Program(_) = item.origin {
            let repeat = result.is_err() && self.job_active && self.mode == StreamMode::RepeatOnError && item.repeat < MAX_REPEAT;
            if repeat {
                will_repeat = true;
                self.program.push_front(Queued { repeat: item.repeat + 1, ..item.clone() });
            } else {
                self.progress.acked += 1;
                if result.is_err() {
                    self.progress.errors += 1;
                    if self.on_error == OnError::Pause && self.job_active {
                        self.paused = true;
                    }
                }
            }
        }
        let program_left = self.program.is_empty() && !self.in_flight.iter().any(|q| matches!(q.origin, Origin::Program(_)));
        let program_finished = self.job_active && program_left && matches!(item.origin, Origin::Program(_)) && !will_repeat;
        if program_finished {
            self.job_active = false;
        }
        Some(Ack { item, result, program_finished, will_repeat })
    }

    /// Considère acquittées toutes les lignes en vol (« ok » perdus : LaserGRBL `CreateFakeOK`).
    pub fn force_ack_all(&mut self) -> Vec<Ack> {
        let mut acks = Vec::new();
        while let Some(a) = self.on_response(Ok(())) {
            acks.push(a);
        }
        acks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(s: &mut Streamer) -> Vec<String> {
        let mut v = Vec::new();
        while let Some(q) = s.next_to_send() {
            v.push(q.line);
        }
        v
    }

    #[test]
    fn buffered_fills_up_to_capacity() {
        let mut s = Streamer::new(StreamMode::Buffered, OnError::Continue);
        // 9 octets par ligne (8 + \n) : 14 lignes = 126 ≤ 127, la 15e ne tient pas
        let n = s.start_program((0..20).map(|i| format!("G1X{:05}", i)));
        assert_eq!(n, 20);
        assert_eq!(ids(&mut s).len(), 14);
        assert_eq!(s.used(), 126);
        s.on_response(Ok(()));
        assert_eq!(ids(&mut s).len(), 1);
        assert!(s.used() <= s.capacity());
    }

    #[test]
    fn synchronous_sends_one_at_a_time() {
        let mut s = Streamer::new(StreamMode::Synchronous, OnError::Continue);
        s.start_program(["G0X1", "G0X2"]);
        assert_eq!(ids(&mut s), ["G0X1"]);
        s.on_response(Ok(()));
        assert_eq!(ids(&mut s), ["G0X2"]);
    }

    #[test]
    fn manual_commands_go_first_and_skip_empty() {
        let mut s = Streamer::new(StreamMode::Synchronous, OnError::Continue);
        s.start_program(["G0X1", "(commentaire)", "G0X2"]);
        assert_eq!(s.progress().total, 2);
        assert!(s.push_manual("  ; rien ").is_none());
        s.push_manual("$X");
        assert_eq!(ids(&mut s), ["$X"]);
        s.on_response(Ok(()));
        assert_eq!(ids(&mut s), ["G0X1"]);
    }

    #[test]
    fn long_line_goes_when_buffer_empty() {
        let mut s = Streamer::new(StreamMode::Buffered, OnError::Continue);
        let long = format!("G1X{}", "1".repeat(200));
        s.push_manual(&long);
        assert_eq!(ids(&mut s).len(), 1);
    }

    #[test]
    fn progress_errors_and_finish() {
        let mut s = Streamer::new(StreamMode::Buffered, OnError::Continue);
        s.start_program(["G0X1", "G0X2"]);
        ids(&mut s);
        let a = s.on_response(Err(20));
        assert!(a.is_some_and(|a| !a.program_finished && a.item.origin == Origin::Program(1)));
        let a = s.on_response(Ok(()));
        assert!(a.is_some_and(|a| a.program_finished));
        assert_eq!(s.progress(), Progress { total: 2, sent: 2, acked: 2, errors: 1 });
        assert!(!s.job_active());
        assert!(s.on_response(Ok(())).is_none());
    }

    #[test]
    fn repeat_on_error_up_to_three_times() {
        let mut s = Streamer::new(StreamMode::RepeatOnError, OnError::Continue);
        s.start_program(["G1X1"]);
        for _ in 0..3 {
            assert_eq!(ids(&mut s), ["G1X1"]);
            assert!(s.on_response(Err(2)).is_some_and(|a| a.will_repeat));
        }
        assert_eq!(ids(&mut s), ["G1X1"]);
        assert!(s.on_response(Err(2)).is_some_and(|a| !a.will_repeat && a.program_finished));
        assert_eq!(s.progress(), Progress { total: 1, sent: 1, acked: 1, errors: 1 });
    }

    #[test]
    fn pause_on_error() {
        let mut s = Streamer::new(StreamMode::Synchronous, OnError::Pause);
        s.start_program(["G0X1", "G0X2"]);
        ids(&mut s);
        s.on_response(Err(9));
        assert!(s.is_paused());
        assert!(ids(&mut s).is_empty());
        s.resume();
        assert_eq!(ids(&mut s), ["G0X2"]);
    }

    #[test]
    fn buffer_size_discovery() {
        let mut s = Streamer::new(StreamMode::Buffered, OnError::Continue);
        s.observe_free_rx(100);
        assert_eq!(s.capacity(), DEFAULT_RX_BUFFER);
        s.observe_free_rx(128);
        assert_eq!(s.capacity(), 128);
        s.observe_free_rx(256);
        assert_eq!(s.capacity(), 128, "découverte une seule fois");
        s.set_capacity_from_opt(1024);
        assert_eq!(s.capacity(), 1024);
    }

    #[test]
    fn reset_and_abort() {
        let mut s = Streamer::new(StreamMode::Buffered, OnError::Continue);
        s.start_program(["G0X1", "G0X2", "G0X3"]);
        s.next_to_send();
        let p = s.abort_program();
        assert_eq!(p.sent, 1);
        assert_eq!(s.in_flight(), 1);
        s.reset();
        assert!(s.is_idle());
        assert_eq!(s.used(), 0);
    }
}
