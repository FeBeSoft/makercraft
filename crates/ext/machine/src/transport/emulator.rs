//! Émulateur Grbl 1.1 en mémoire, pour tester sans machine.
//!
//! Inspiré de LaserGRBL 7.14.1 `GrblEmulator/Grblv11Emulator.cs` (réglages par défaut, `$I`,
//! `$C`, `$H`), avec en plus ce qui sert à tester un envoyeur :
//! - un **tampon de réception de 128 octets** comme une vraie carte : un envoyeur qui le dépasse
//!   est détecté (`overflow_count`) ;
//! - **une ligne exécutée par lecture** : les réponses arrivent avec du retard, le tampon se
//!   remplit vraiment pendant un envoi ;
//! - positions machine/travail (G90/G91, G92, G10 L20, `$J=`), pause `!`/`~`, reset Ctrl-X,
//!   alarme verrouillée jusqu'à `$X` ou `$H`, codes d'erreur réalistes.

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use makercraft_geom::Vec3;

use super::{Transport, TransportError};
use crate::protocol::rt;

pub const WELCOME: &str = "Grbl 1.1h ['$' for help]";
const PLANNER_BLOCKS: usize = 15;

pub struct GrblEmulator {
    open: bool,
    rx_size: usize,
    /// Octets reçus et pas encore exécutés (lignes complètes + ligne en cours).
    rx_used: usize,
    partial: Vec<u8>,
    queue: VecDeque<String>,
    out: VecDeque<String>,
    overflows: Arc<AtomicUsize>,
    hold: bool,
    check: bool,
    alarm: bool,
    absolute: bool,
    mpos: Vec3,
    wco: Vec3,
    feed: f64,
    spindle: f64,
    ov: [u32; 3],
    settings: BTreeMap<u16, String>,
}

impl Default for GrblEmulator {
    fn default() -> Self {
        Self::new()
    }
}

impl GrblEmulator {
    pub fn new() -> Self {
        Self::with_rx_size(128)
    }

    pub fn with_rx_size(rx_size: usize) -> Self {
        // Réglages par défaut repris de l'émulateur LaserGRBL.
        let defaults: [(u16, &str); 34] = [
            (0, "10"),
            (1, "25"),
            (2, "0"),
            (3, "0"),
            (4, "0"),
            (5, "0"),
            (6, "0"),
            (10, "1"),
            (11, "0.010"),
            (12, "0.002"),
            (13, "0"),
            (20, "0"),
            (21, "0"),
            (22, "0"),
            (23, "0"),
            (24, "25.000"),
            (25, "500.000"),
            (26, "250"),
            (27, "1.000"),
            (30, "1000"),
            (31, "0"),
            (32, "1"),
            (100, "250.000"),
            (101, "250.000"),
            (102, "250.000"),
            (110, "500.000"),
            (111, "500.000"),
            (112, "500.000"),
            (120, "10.000"),
            (121, "10.000"),
            (122, "10.000"),
            (130, "200.000"),
            (131, "200.000"),
            (132, "200.000"),
        ];
        Self {
            open: true,
            rx_size,
            rx_used: 0,
            partial: Vec::new(),
            queue: VecDeque::new(),
            out: VecDeque::from([WELCOME.to_owned()]),
            overflows: Arc::new(AtomicUsize::new(0)),
            hold: false,
            check: false,
            alarm: false,
            absolute: true,
            mpos: Vec3::ZERO,
            wco: Vec3::ZERO,
            feed: 0.0,
            spindle: 0.0,
            ov: [100, 100, 100],
            settings: defaults.iter().map(|(k, v)| (*k, (*v).to_owned())).collect(),
        }
    }

    /// Compteur de débordements du tampon de réception (partagé : lisible après avoir confié
    /// l'émulateur à un contrôleur).
    pub fn overflow_counter(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.overflows)
    }

    /// Démarre en alarme, comme une carte avec prise d'origine obligatoire.
    pub fn start_in_alarm(mut self) -> Self {
        self.alarm = true;
        self.out.push_back("[MSG:'$H'|'$X' to unlock]".to_owned());
        self
    }

    fn state(&self) -> &'static str {
        if self.alarm {
            "Alarm"
        } else if self.check {
            "Check"
        } else if self.hold {
            "Hold:0"
        } else if self.queue.is_empty() {
            "Idle"
        } else {
            "Run"
        }
    }

    fn status(&self) -> String {
        let m = self.mpos;
        let w = self.wco;
        format!(
            "<{}|MPos:{:.3},{:.3},{:.3}|Bf:{},{}|FS:{},{}|Ov:{},{},{}|WCO:{:.3},{:.3},{:.3}>",
            self.state(),
            m.x,
            m.y,
            m.z,
            PLANNER_BLOCKS.saturating_sub(self.queue.len()),
            self.rx_size.saturating_sub(self.rx_used),
            self.feed,
            self.spindle,
            self.ov[0],
            self.ov[1],
            self.ov[2],
            w.x,
            w.y,
            w.z
        )
    }

    fn reset(&mut self) {
        self.queue.clear();
        self.partial.clear();
        self.rx_used = 0;
        self.hold = false;
        self.check = false;
        self.absolute = true;
        self.ov = [100, 100, 100];
        self.out.clear();
        self.out.push_back(WELCOME.to_owned());
    }

    fn realtime(&mut self, b: u8) {
        match b {
            rt::STATUS => {
                let s = self.status();
                self.out.push_back(s);
                return;
            }
            rt::FEED_HOLD => self.hold = true,
            rt::CYCLE_START => self.hold = false,
            rt::SOFT_RESET => self.reset(),
            _ => {}
        }
        let ov = &mut self.ov;
        match b {
            rt::FEED_OV_RESET => ov[0] = 100,
            rt::FEED_OV_PLUS_10 => ov[0] = (ov[0] + 10).min(200),
            rt::FEED_OV_MINUS_10 => ov[0] = ov[0].saturating_sub(10).max(10),
            rt::FEED_OV_PLUS_1 => ov[0] = (ov[0] + 1).min(200),
            rt::FEED_OV_MINUS_1 => ov[0] = ov[0].saturating_sub(1).max(10),
            rt::RAPID_OV_100 => ov[1] = 100,
            rt::RAPID_OV_50 => ov[1] = 50,
            rt::RAPID_OV_25 => ov[1] = 25,
            rt::SPINDLE_OV_RESET => ov[2] = 100,
            rt::SPINDLE_OV_PLUS_10 => ov[2] = (ov[2] + 10).min(200),
            rt::SPINDLE_OV_MINUS_10 => ov[2] = ov[2].saturating_sub(10).max(10),
            rt::SPINDLE_OV_PLUS_1 => ov[2] = (ov[2] + 1).min(200),
            rt::SPINDLE_OV_MINUS_1 => ov[2] = ov[2].saturating_sub(1).max(10),
            _ => {}
        }
    }

    /// Exécute une ligne et renvoie ses réponses (la dernière est `ok` ou `error:N`).
    fn execute(&mut self, line: &str) -> Vec<String> {
        let up = line.trim().to_ascii_uppercase();
        let mut out = Vec::new();
        let result: Result<(), u16> = match up.as_str() {
            "" => Ok(()),
            "$$" => {
                out.extend(self.settings.iter().map(|(k, v)| format!("${k}={v}")));
                Ok(())
            }
            "$I" => {
                out.push("[VER:1.1h.20190825:MakerCraft]".to_owned());
                out.push(format!("[OPT:V,{PLANNER_BLOCKS},{}]", self.rx_size));
                Ok(())
            }
            "$G" => {
                out.push(format!("[GC:G0 G54 G17 G21 {} G94 M5 M9 T0 F{} S{}]", if self.absolute { "G90" } else { "G91" }, self.feed, self.spindle));
                Ok(())
            }
            "$#" => {
                out.push(format!("[G54:{:.3},{:.3},{:.3}]", self.wco.x, self.wco.y, self.wco.z));
                Ok(())
            }
            "$H" => {
                self.mpos = Vec3::ZERO;
                self.alarm = false;
                Ok(())
            }
            "$X" => {
                self.alarm = false;
                out.push("[MSG:Caution: Unlocked]".to_owned());
                Ok(())
            }
            "$C" => {
                self.check = !self.check;
                out.push(if self.check { "[MSG:Enabled]" } else { "[MSG:Disabled]" }.to_owned());
                Ok(())
            }
            _ if up.starts_with("$J=") => {
                if self.alarm {
                    Err(9)
                } else {
                    self.jog(&up[3..])
                }
            }
            _ if up.starts_with('$') => self.set_setting(&up[1..]),
            _ if self.alarm => Err(9),
            _ => self.gcode(&up),
        };
        out.push(match result {
            Ok(()) => "ok".to_owned(),
            Err(code) => format!("error:{code}"),
        });
        out
    }

    fn set_setting(&mut self, s: &str) -> Result<(), u16> {
        let (k, v) = s.split_once('=').ok_or(3u16)?;
        let k: u16 = k.parse().map_err(|_| 3u16)?;
        v.parse::<f64>().map_err(|_| 2u16)?;
        let slot = self.settings.get_mut(&k).ok_or(3u16)?;
        *slot = v.to_owned();
        Ok(())
    }

    fn jog(&mut self, words: &str) -> Result<(), u16> {
        let words = parse_words(words)?;
        if !words.iter().any(|(l, _)| *l == 'F') {
            return Err(22);
        }
        let mut absolute = self.absolute;
        let mut machine = false;
        for (l, v) in &words {
            match (l, code10(*v)) {
                ('G', 900) => absolute = true,
                ('G', 910) => absolute = false,
                ('G', 530) => machine = true,
                ('G', 200 | 210) | ('X' | 'Y' | 'Z' | 'F', _) => {}
                _ => return Err(20),
            }
        }
        self.move_to(&words, absolute, machine);
        Ok(())
    }

    fn gcode(&mut self, line: &str) -> Result<(), u16> {
        let words = parse_words(line)?;
        let (mut set_offset, mut machine, mut absolute) = (false, false, self.absolute);
        for (l, v) in &words {
            match (*l, code10(*v)) {
                ('G', 0 | 10 | 20 | 30 | 40 | 170 | 180 | 190 | 200 | 210 | 400 | 490 | 610 | 640 | 800 | 930 | 940 | 540..=590) => {}
                ('G', 900) => absolute = true,
                ('G', 910) => absolute = false,
                ('G', 920 | 100) => set_offset = true,
                ('G', 530) => machine = true,
                ('G', _) => return Err(20),
                ('M', 0 | 10 | 20 | 30 | 40 | 50 | 70 | 80 | 90 | 300) => {}
                ('M', _) => return Err(20),
                ('F', _) => self.feed = *v,
                ('S', _) => self.spindle = *v,
                ('X' | 'Y' | 'Z' | 'I' | 'J' | 'K' | 'R' | 'P' | 'L' | 'N' | 'T', _) => {}
                _ => return Err(20),
            }
        }
        self.absolute = absolute;
        if self.check {
            return Ok(());
        }
        if set_offset {
            // G92 / G10 L20 : la position actuelle prend les coordonnées de travail données.
            for (l, v) in &words {
                match l {
                    'X' => self.wco.x = self.mpos.x - v,
                    'Y' => self.wco.y = self.mpos.y - v,
                    'Z' => self.wco.z = self.mpos.z - v,
                    _ => {}
                }
            }
        } else {
            self.move_to(&words, absolute, machine);
        }
        Ok(())
    }

    fn move_to(&mut self, words: &[(char, f64)], absolute: bool, machine: bool) {
        let origin = if machine { Vec3::ZERO } else { self.wco };
        for (l, v) in words {
            let (pos, off) = match l {
                'X' => (&mut self.mpos.x, origin.x),
                'Y' => (&mut self.mpos.y, origin.y),
                'Z' => (&mut self.mpos.z, origin.z),
                _ => continue,
            };
            *pos = if absolute { v + off } else { *pos + v };
        }
    }
}

/// G-code × 10 arrondi : G38.2 → 382, G90 → 900 (les sous-codes restent distincts).
fn code10(v: f64) -> i64 {
    (v * 10.0).round() as i64
}

/// `G1X10.5Y-3F1000` → [('G',1), ('X',10.5), …] ; error:1 sans lettre, error:2 nombre invalide.
fn parse_words(s: &str) -> Result<Vec<(char, f64)>, u16> {
    let mut words = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if !c.is_ascii_alphabetic() {
            return Err(1);
        }
        let mut num = String::new();
        while let Some(&d) = chars.peek() {
            if d.is_ascii_digit() || d == '.' || d == '-' || d == '+' {
                num.push(d);
                chars.next();
            } else {
                break;
            }
        }
        let v: f64 = num.parse().map_err(|_| 2u16)?;
        if !v.is_finite() {
            return Err(2);
        }
        words.push((c, v));
    }
    Ok(words)
}

impl Transport for GrblEmulator {
    fn name(&self) -> String {
        "émulateur Grbl 1.1".to_owned()
    }

    fn write(&mut self, data: &[u8]) -> Result<(), TransportError> {
        if !self.open {
            return Err(TransportError::Closed);
        }
        for &b in data {
            if rt::is_realtime(b) {
                self.realtime(b);
                continue;
            }
            self.rx_used += 1;
            if self.rx_used > self.rx_size {
                self.overflows.fetch_add(1, Ordering::Relaxed);
            }
            if b == b'\n' {
                let line = String::from_utf8_lossy(&self.partial).trim_end_matches('\r').to_owned();
                self.partial.clear();
                self.queue.push_back(line);
            } else {
                self.partial.push(b);
            }
        }
        Ok(())
    }

    fn read_line(&mut self, timeout: Duration) -> Result<Option<String>, TransportError> {
        if !self.open {
            return Err(TransportError::Closed);
        }
        if self.out.is_empty()
            && !self.hold
            && let Some(line) = self.queue.pop_front()
        {
            self.rx_used = self.rx_used.saturating_sub(line.len() + 1);
            let responses = self.execute(&line);
            self.out.extend(responses);
        }
        match self.out.pop_front() {
            Some(l) => Ok(Some(l)),
            None => {
                std::thread::sleep(timeout.min(Duration::from_millis(1)));
                Ok(None)
            }
        }
    }

    fn close(&mut self) {
        self.open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drain(e: &mut GrblEmulator) -> Vec<String> {
        let mut v = Vec::new();
        while let Ok(Some(l)) = e.read_line(Duration::ZERO) {
            v.push(l);
        }
        v
    }

    fn send(e: &mut GrblEmulator, line: &str) -> Vec<String> {
        let _ = e.write(format!("{line}\n").as_bytes());
        drain(e)
    }

    #[test]
    fn welcome_moves_and_offsets() {
        let mut e = GrblEmulator::new();
        assert_eq!(drain(&mut e), [WELCOME]);
        assert_eq!(send(&mut e, "G0X10Y5"), ["ok"]);
        assert_eq!(send(&mut e, "G91G1X1F100"), ["ok"]);
        assert_eq!(e.mpos, Vec3::new(11.0, 5.0, 0.0));
        assert_eq!(send(&mut e, "G10L20P0X0Y0"), ["ok"]);
        assert_eq!(send(&mut e, "G90G0X2"), ["ok"]);
        assert_eq!(e.mpos.x, 13.0);
        assert_eq!(send(&mut e, "$J=G91X-3F500"), ["ok"]);
        assert_eq!(e.mpos.x, 10.0);
        let _ = e.write(b"?");
        let status = drain(&mut e);
        assert!(status[0].starts_with("<Idle|MPos:10.000,5.000,0.000|Bf:15,128|"), "{status:?}");
    }

    #[test]
    fn realistic_errors_and_alarm() {
        let mut e = GrblEmulator::new().start_in_alarm();
        drain(&mut e);
        assert_eq!(send(&mut e, "G0X1"), ["error:9"]);
        assert_eq!(send(&mut e, "$X"), ["[MSG:Caution: Unlocked]", "ok"]);
        assert_eq!(send(&mut e, "G1X1Q2"), ["error:20"]);
        assert_eq!(send(&mut e, "G1XA"), ["error:2"]);
        assert_eq!(send(&mut e, "$J=G91X1"), ["error:22"]);
        assert_eq!(send(&mut e, "$999=1"), ["error:3"]);
        assert_eq!(send(&mut e, "$110=800"), ["ok"]);
        assert!(send(&mut e, "$$").contains(&"$110=800".to_owned()));
    }

    #[test]
    fn detects_rx_overflow_and_reset_clears() {
        let mut e = GrblEmulator::new();
        drain(&mut e);
        let _ = e.write(&[b'G'; 200]);
        assert!(e.overflow_counter().load(Ordering::Relaxed) > 0);
        let _ = e.write(&[rt::SOFT_RESET]);
        assert_eq!(drain(&mut e), [WELCOME]);
        assert_eq!(e.rx_used, 0);
    }

    #[test]
    fn hold_stops_execution() {
        let mut e = GrblEmulator::new();
        drain(&mut e);
        let _ = e.write(b"!G0X5\n");
        assert_eq!(drain(&mut e), Vec::<String>::new());
        let _ = e.write(b"~");
        assert_eq!(drain(&mut e), ["ok"]);
    }
}
