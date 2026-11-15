//! makercraft-send : console Grbl et envoi de fichiers G-code.
//!
//! Ctrl-C pendant un envoi : pause immédiate puis reset logiciel de la carte (le laser s'arrête).

use std::io::{BufRead, Write};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use makercraft_ext_machine::codes;
use makercraft_ext_machine::protocol::rt;
use makercraft_ext_machine::transport::list_ports;
use makercraft_ext_machine::{Controller, ControllerOptions, Event, MachineState, OnError, Origin, StreamMode, TransportConfig};

const HELP: &str = "\
makercraft-send — console Grbl et envoi de G-code

  makercraft-send ports                         liste les ports série
  makercraft-send --port COM3 [--baud 115200]   console interactive
  makercraft-send --port COM3 fichier.nc        envoie un fichier
  makercraft-send --tcp hôte:port [fichier.nc]  liaison réseau (ESP32, grblHAL)
  makercraft-send --emulator [fichier.nc]       émulateur intégré, sans machine

options : --sync            une ligne à la fois (liaison douteuse)
          --no-reset        ne pas réinitialiser la carte à la connexion
          --pause-on-error  suspendre l'envoi à la première erreur

console : G-code ou commande $ ; ?  !  ~  reset  home  unlock  jogstop  quit
Ctrl-C : arrêt d'urgence (pause + reset de la carte)";

struct Args {
    config: Option<TransportConfig>,
    file: Option<String>,
    opts: ControllerOptions,
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut config = None;
    let mut port = None;
    let mut baud = 115_200u32;
    let mut file = None;
    let mut opts = ControllerOptions::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--port" => port = Some(it.next().ok_or("--port attend un nom de port (COM3)")?.clone()),
            "--baud" => baud = it.next().and_then(|b| b.parse().ok()).ok_or("--baud attend un nombre (115200)")?,
            "--tcp" => config = Some(TransportConfig::Tcp { address: it.next().ok_or("--tcp attend hôte:port")?.clone() }),
            "--emulator" => config = Some(TransportConfig::Emulator),
            "--sync" => opts.mode = StreamMode::Synchronous,
            "--no-reset" => opts.reset_on_connect = false,
            "--pause-on-error" => opts.on_error = OnError::Pause,
            s if s.starts_with("--") => return Err(format!("option inconnue : {s}")),
            s => file = Some(s.to_owned()),
        }
    }
    if let Some(port) = port {
        config = Some(TransportConfig::Serial { port, baud });
    }
    Ok(Args { config, file, opts })
}

fn describe_error(code: u16) -> String {
    codes::error(code).map(|c| format!("error:{code} - {}", c.explain())).unwrap_or_else(|| format!("error:{code}"))
}

fn describe_alarm(code: u16) -> String {
    codes::alarm(code).map(|c| format!("ALARM:{code} - {}", c.explain())).unwrap_or_else(|| format!("ALARM:{code}"))
}

fn fmt_duration(d: Duration) -> String {
    let s = d.as_secs();
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.is_empty() || raw.iter().any(|a| a == "-h" || a == "--help") {
        println!("{HELP}");
        return ExitCode::SUCCESS;
    }
    if raw.first().is_some_and(|a| a == "ports") {
        return match list_ports() {
            Ok(ports) if ports.is_empty() => {
                println!("Aucun port série. Machine branchée ? Pilote CH340/CP210x installé ?");
                ExitCode::SUCCESS
            }
            Ok(ports) => {
                for p in ports {
                    println!("{:<10} {}", p.name, p.description);
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        };
    }
    let args = match parse_args(&raw) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}\n\n{HELP}");
            return ExitCode::FAILURE;
        }
    };
    let Some(config) = args.config.clone() else {
        eprintln!("Indiquer --port, --tcp ou --emulator.\n\n{HELP}");
        return ExitCode::FAILURE;
    };
    let program = match &args.file {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(text) => Some(text.lines().map(str::to_owned).collect::<Vec<_>>()),
            Err(e) => {
                eprintln!("{path} : {e}");
                return ExitCode::FAILURE;
            }
        },
        None => None,
    };

    let controller = match Controller::connect(&config, args.opts.clone()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Connexion impossible : {e}");
            return ExitCode::FAILURE;
        }
    };

    let interrupted = Arc::new(AtomicBool::new(false));
    {
        let flag = Arc::clone(&interrupted);
        if let Err(e) = ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst)) {
            eprintln!("(Ctrl-C non intercepté : {e})");
        }
    }

    let ok = match program {
        Some(lines) => send_file(&controller, lines, &interrupted),
        None => console(&controller, &interrupted),
    };
    controller.disconnect();
    if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

/// Affiche un événement commun aux deux modes ; renvoie `false` si la liaison est perdue.
fn show_common(e: &Event, last_state: &mut MachineState) -> bool {
    match e {
        Event::Connected { transport } => println!("● connecté : {transport}"),
        Event::Firmware(f) => println!("● firmware {:?} {}", f.firmware, f.version()),
        Event::Alarm(code) => println!("⚠ {}", describe_alarm(*code)),
        Event::Issue(i) => println!("⚠ {}", i.label()),
        Event::Status(s) if s.state != *last_state => {
            *last_state = s.state;
            println!("● {} — X{:.3} Y{:.3} Z{:.3}", s.state.label(), s.wpos.x, s.wpos.y, s.wpos.z);
        }
        Event::Disconnected { reason } => {
            println!("● déconnecté{}", reason.as_ref().map(|r| format!(" : {r}")).unwrap_or_default());
            return false;
        }
        _ => {}
    }
    true
}

fn emergency(c: &Controller) {
    println!("\n■ ARRÊT D'URGENCE : pause + reset de la carte");
    c.emergency_stop();
    std::thread::sleep(Duration::from_millis(300));
}

fn send_file(c: &Controller, lines: Vec<String>, interrupted: &AtomicBool) -> bool {
    let mut state = MachineState::Unknown;
    // attendre la connexion
    loop {
        if interrupted.load(Ordering::SeqCst) {
            return false;
        }
        match c.next_event(Duration::from_millis(100)) {
            Some(Event::Connected { transport }) => {
                println!("● connecté : {transport}");
                break;
            }
            Some(e) if !show_common(&e, &mut state) => return false,
            Some(_) | None => {}
        }
    }
    // laisser passer le reset, $I et le premier rapport d'état
    let settle = Instant::now();
    while settle.elapsed() < Duration::from_millis(800) {
        if let Some(e) = c.next_event(Duration::from_millis(50)) {
            show_common(&e, &mut state);
        }
    }
    if state == MachineState::Alarm {
        println!("La machine est en alarme : faire la prise d'origine ($H) ou déverrouiller ($X) avant d'envoyer.");
        return false;
    }
    let start = Instant::now();
    println!("▶ envoi de {} lignes", lines.len());
    c.run_program(lines);
    let mut last_percent = -1i64;
    loop {
        if interrupted.swap(false, Ordering::SeqCst) {
            emergency(c);
            return false;
        }
        let Some(e) = c.next_event(Duration::from_millis(100)) else { continue };
        match &e {
            Event::Ack { origin: Origin::Program(n), line, result: Err(code), .. } => {
                println!("\n✗ ligne {n} « {line} » : {}", describe_error(*code))
            }
            Event::Progress(p) => {
                let pct = p.percent().floor() as i64;
                if pct != last_percent {
                    last_percent = pct;
                    print!("\r  {pct:>3} %  {}/{} lignes  {} erreur(s)  {}   ", p.acked, p.total, p.errors, fmt_duration(start.elapsed()));
                    let _ = std::io::stdout().flush();
                }
            }
            Event::ProgramFinished(p) => {
                println!("\n✔ terminé : {} lignes, {} erreur(s), {}", p.acked, p.errors, fmt_duration(start.elapsed()));
                // attendre la fin des mouvements déjà dans la carte
                let wait = Instant::now();
                while wait.elapsed() < Duration::from_secs(3600) {
                    if interrupted.swap(false, Ordering::SeqCst) {
                        emergency(c);
                        return false;
                    }
                    if let Some(Event::Status(s)) = c.next_event(Duration::from_millis(200))
                        && s.state == MachineState::Idle
                    {
                        break;
                    }
                }
                return p.errors == 0;
            }
            Event::ProgramAborted(p) => {
                println!("\n■ programme interrompu à la ligne {}/{}", p.acked, p.total);
                return false;
            }
            other => {
                if matches!(other, Event::Alarm(_) | Event::Issue(_)) {
                    println!();
                }
                if !show_common(other, &mut state) {
                    return false;
                }
            }
        }
    }
}

fn console(c: &Controller, interrupted: &AtomicBool) -> bool {
    println!("Console Grbl — « quit » pour sortir, Ctrl-C = arrêt d'urgence.");
    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut state = MachineState::Unknown;
    let mut show_next_status = false;
    loop {
        if interrupted.swap(false, Ordering::SeqCst) {
            emergency(c);
        }
        loop {
            match rx.try_recv() {
                Ok(input) => match input.trim() {
                    "" => {}
                    "quit" | "exit" => {
                        drain(c, &mut state, &mut show_next_status);
                        return true;
                    }
                    "?" | "status" => show_next_status = true,
                    "!" => c.hold(),
                    "~" => c.cycle_start(),
                    "reset" => c.soft_reset(),
                    "home" => c.home(),
                    "unlock" => c.unlock(),
                    "jogstop" => c.realtime(rt::JOG_CANCEL),
                    other => c.send(other),
                },
                // entrée fermée (fichier redirigé terminé) : on sort proprement
                Err(mpsc::TryRecvError::Disconnected) => {
                    drain(c, &mut state, &mut show_next_status);
                    return true;
                }
                Err(mpsc::TryRecvError::Empty) => break,
            }
        }
        let Some(e) = c.next_event(Duration::from_millis(50)) else { continue };
        if !print_console_event(&e, &mut state, &mut show_next_status) {
            return false;
        }
    }
}

/// Avant de quitter : affiche les réponses encore attendues (au plus 3 s, ou 700 ms de silence,
/// plus long que l'intervalle des rapports d'état pour qu'un `?` en attente soit servi).
fn drain(c: &Controller, state: &mut MachineState, show_next_status: &mut bool) {
    let start = Instant::now();
    let mut last = Instant::now();
    while start.elapsed() < Duration::from_secs(3) && (last.elapsed() < Duration::from_millis(700) || *show_next_status) {
        if let Some(e) = c.next_event(Duration::from_millis(50)) {
            if !matches!(e, Event::Status(_)) || *show_next_status {
                last = Instant::now();
            }
            print_console_event(&e, state, show_next_status);
        }
    }
}

fn print_console_event(e: &Event, state: &mut MachineState, show_next_status: &mut bool) -> bool {
    match e {
        Event::Line(l) => println!("< {l}"),
        Event::Ack { line, result, origin: Origin::Manual, .. } => match result {
            Ok(()) => println!("< ok   ({line})"),
            Err(code) => println!("< {}   ({line})", describe_error(*code)),
        },
        Event::Status(s) if *show_next_status => {
            *show_next_status = false;
            println!(
                "< {} | travail X{:.3} Y{:.3} Z{:.3} | machine X{:.3} Y{:.3} Z{:.3} | F{} S{} | Ov {}/{}/{}",
                s.state.label(),
                s.wpos.x,
                s.wpos.y,
                s.wpos.z,
                s.mpos.x,
                s.mpos.y,
                s.mpos.z,
                s.feed,
                s.spindle,
                s.overrides.0,
                s.overrides.1,
                s.overrides.2
            );
            *state = s.state;
        }
        other => return show_common(other, state),
    }
    true
}
