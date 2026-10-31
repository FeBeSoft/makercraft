//! Sessions complètes contrôleur ↔ émulateur Grbl : connexion, jog, envoi d'un programme,
//! erreurs, alarme, reset. C'est le filet de sécurité de toute mise à jour (UPSTREAM.md).

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use makercraft_ext_machine::transport::emulator::GrblEmulator;
use makercraft_ext_machine::{Controller, ControllerOptions, Event, Issue, MachineState, MachineStatus, Origin, Progress, StreamMode};
use makercraft_geom::Vec3;

const TIMEOUT: Duration = Duration::from_secs(20);

fn fast_opts() -> ControllerOptions {
    ControllerOptions { status_interval: Duration::from_millis(20), ..ControllerOptions::default() }
}

/// Attend un événement qui satisfait `pred` ; renvoie aussi tous les événements vus.
fn wait_for(c: &Controller, mut pred: impl FnMut(&Event) -> bool) -> (Option<Event>, Vec<Event>) {
    let start = Instant::now();
    let mut seen = Vec::new();
    while start.elapsed() < TIMEOUT {
        if let Some(e) = c.next_event(Duration::from_millis(50)) {
            if pred(&e) {
                return (Some(e), seen);
            }
            seen.push(e);
        }
    }
    (None, seen)
}

fn connect(emu: GrblEmulator, opts: ControllerOptions) -> Controller {
    let c = Controller::with_transport(Box::new(emu), opts);
    let (ok, seen) = wait_for(&c, |e| matches!(e, Event::Connected { .. }));
    assert!(ok.is_some(), "pas de connexion : {seen:?}");
    c
}

fn wait_status(c: &Controller, pred: impl Fn(&MachineStatus) -> bool) -> MachineStatus {
    let (e, seen) = wait_for(c, |e| matches!(e, Event::Status(s) if pred(s)));
    match e {
        Some(Event::Status(s)) => s,
        _ => panic!("état attendu jamais vu ; derniers événements : {:?}", seen.iter().rev().take(5).collect::<Vec<_>>()),
    }
}

#[test]
fn connect_jog_and_work_zero() {
    let c = connect(GrblEmulator::new(), fast_opts());
    c.jog(10.0, 5.0, 0.0, 1000.0);
    let s = wait_status(&c, |s| s.mpos == Vec3::new(10.0, 5.0, 0.0));
    assert_eq!(s.state, MachineState::Idle);
    c.set_work_zero(true, true, false);
    let s = wait_status(&c, |s| s.wpos == Vec3::ZERO && s.wco == Vec3::new(10.0, 5.0, 0.0));
    assert_eq!(s.mpos, Vec3::new(10.0, 5.0, 0.0));
    c.disconnect();
}

#[test]
fn streams_a_long_program_without_overflow() {
    let emu = GrblEmulator::new();
    let overflows = emu.overflow_counter();
    let c = connect(emu, fast_opts());
    let program: Vec<String> = (0..2000).map(|i| format!("G1 X{:.3} Y{:.3} F1500 ; ligne {i}", (i % 100) as f64 * 0.5, (i / 100) as f64)).collect();
    c.run_program(program);
    let (done, seen) = wait_for(&c, |e| matches!(e, Event::ProgramFinished(_)));
    let Some(Event::ProgramFinished(p)) = done else { panic!("programme non terminé : {:?}", seen.last()) };
    assert_eq!(p, Progress { total: 2000, sent: 2000, acked: 2000, errors: 0 });
    assert_eq!(overflows.load(Ordering::Relaxed), 0, "le tampon de 128 octets de la carte a débordé");
    // le streaming a vraiment rempli le tampon : plusieurs lignes envoyées avant le premier ok
    let mut in_flight = 0usize;
    let mut max_in_flight = 0usize;
    for e in &seen {
        match e {
            Event::Sent { origin: Origin::Program(_), .. } => in_flight += 1,
            Event::Ack { origin: Origin::Program(_), .. } => in_flight = in_flight.saturating_sub(1),
            _ => {}
        }
        max_in_flight = max_in_flight.max(in_flight);
    }
    assert!(max_in_flight >= 4, "le tampon n'a pas été rempli (max {max_in_flight} lignes en vol)");
    let s = wait_status(&c, |s| s.state == MachineState::Idle);
    assert_eq!(s.mpos, Vec3::new(49.5, 19.0, 0.0));
}

#[test]
fn reports_errors_with_line_numbers_in_sync_mode() {
    let c = connect(GrblEmulator::new(), ControllerOptions { mode: StreamMode::Synchronous, ..fast_opts() });
    c.run_program(vec!["G0 X1".into(), "(commentaire)".into(), "G1 X2 Q5".into(), "G0 X3".into()]);
    let (done, seen) = wait_for(&c, |e| matches!(e, Event::ProgramFinished(_)));
    assert!(matches!(done, Some(Event::ProgramFinished(Progress { total: 3, acked: 3, errors: 1, .. }))), "{done:?}");
    let err = seen.iter().find_map(|e| match e {
        Event::Ack { origin, result: Err(code), .. } => Some((*origin, *code)),
        _ => None,
    });
    assert_eq!(err, Some((Origin::Program(3), 20)), "l'erreur doit pointer la ligne 3 du fichier");
}

#[test]
fn alarm_stops_the_program() {
    let c = connect(GrblEmulator::new().start_in_alarm(), fast_opts());
    c.run_program((0..50).map(|i| format!("G1X{i}F500")).collect());
    // la carte est en alarme : les lignes répondent error:9 ; on débloque puis on relance
    let (done, _) = wait_for(&c, |e| matches!(e, Event::ProgramFinished(_)));
    assert!(matches!(done, Some(Event::ProgramFinished(p)) if p.errors == 50));
    c.unlock();
    c.run_program(vec!["G0X7".into()]);
    let (done, _) = wait_for(&c, |e| matches!(e, Event::ProgramFinished(_)));
    assert!(matches!(done, Some(Event::ProgramFinished(p)) if p.errors == 0));
    wait_status(&c, |s| s.mpos.x == 7.0 && s.state == MachineState::Idle);
}

#[test]
fn hold_resume_and_soft_reset() {
    let c = connect(GrblEmulator::new(), fast_opts());
    c.hold();
    c.run_program((0..20).map(|i| format!("G1X{i}F500")).collect());
    wait_status(&c, |s| s.state == MachineState::Hold);
    c.cycle_start();
    let (done, _) = wait_for(&c, |e| matches!(e, Event::ProgramFinished(_)));
    assert!(done.is_some());

    // reset au milieu d'un travail : programme abandonné, la carte se réannonce
    c.hold();
    c.run_program((0..200).map(|i| format!("G1X{i}F500")).collect());
    wait_status(&c, |s| s.state == MachineState::Hold);
    c.soft_reset();
    let (aborted, seen) = wait_for(&c, |e| matches!(e, Event::ProgramAborted(_)));
    assert!(aborted.is_some(), "{:?}", seen.last());
    let (fw, before_fw) = wait_for(&c, |e| matches!(e, Event::Firmware(_)));
    assert!(fw.is_some());
    assert!(!before_fw.iter().any(|e| matches!(e, Event::Issue(Issue::UnexpectedReset))), "un reset demandé n'est pas « inattendu »");
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(500) {
        let e = c.next_event(Duration::from_millis(20));
        assert!(!matches!(e, Some(Event::Issue(Issue::UnexpectedReset))), "un reset demandé n'est pas « inattendu »");
    }
}

#[test]
fn learns_buffer_size_from_dollar_i() {
    let emu = GrblEmulator::with_rx_size(256);
    let overflows = emu.overflow_counter();
    let c = connect(emu, fast_opts());
    let (opt, _) = wait_for(&c, |e| matches!(e, Event::Line(l) if l.starts_with("[OPT:")));
    assert_eq!(opt, Some(Event::Line("[OPT:V,15,256]".into())));
    c.run_program((0..500).map(|i| format!("G1X{}Y{}F1200", i % 37, i % 11)).collect());
    let (done, _) = wait_for(&c, |e| matches!(e, Event::ProgramFinished(_)));
    assert!(matches!(done, Some(Event::ProgramFinished(p)) if p.acked == 500));
    assert_eq!(overflows.load(Ordering::Relaxed), 0);
}

#[test]
fn connect_timeout_when_nothing_answers() {
    struct Silent;
    impl makercraft_ext_machine::transport::Transport for Silent {
        fn name(&self) -> String {
            "muet".into()
        }
        fn write(&mut self, _: &[u8]) -> Result<(), makercraft_ext_machine::TransportError> {
            Ok(())
        }
        fn read_line(&mut self, t: Duration) -> Result<Option<String>, makercraft_ext_machine::TransportError> {
            std::thread::sleep(t);
            Ok(None)
        }
        fn close(&mut self) {}
    }
    let c = Controller::with_transport(Box::new(Silent), ControllerOptions { connect_timeout: Duration::from_millis(300), ..fast_opts() });
    let (e, _) = wait_for(&c, |e| matches!(e, Event::Disconnected { .. }));
    assert!(matches!(e, Some(Event::Disconnected { reason: Some(_) })), "{e:?}");
}
