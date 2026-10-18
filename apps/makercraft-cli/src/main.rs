//! `makercraft-cli list` : extensions et commandes chargées.
//! `makercraft-cli run <commande> [json]` : exécute une commande, ex.
//! `makercraft-cli run machine.parse_state "{\"state\":\"Idle\"}"`.

use std::process::ExitCode;

use makercraft_ext_api::{Extension, Host, Value};

fn extensions() -> Vec<Box<dyn Extension>> {
    vec![Box::new(makercraft_ext_machine::MachineExtension)]
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut host = Host::new();
    if let Err(e) = host.load(extensions()) {
        eprintln!("erreur de chargement : {e}");
        return ExitCode::FAILURE;
    }
    match args.first().map(String::as_str) {
        Some("list") => {
            println!("MakerCraft {}", env!("CARGO_PKG_VERSION"));
            for (id, label) in host.extensions() {
                println!("\n[{id}] {label}");
                for c in host.commands().iter().filter(|c| c.id.starts_with(&format!("{id}."))) {
                    println!("  {:<28} {}  ({})", c.id, c.label, c.menu);
                }
            }
            ExitCode::SUCCESS
        }
        Some("run") => {
            let Some(id) = args.get(1) else {
                eprintln!("usage : makercraft-cli run <commande> [json]");
                return ExitCode::FAILURE;
            };
            let params = match args.get(2).map(|s| serde_json::from_str::<Value>(s)).transpose() {
                Ok(p) => p.unwrap_or(Value::Null),
                Err(e) => {
                    eprintln!("paramètres JSON invalides : {e}");
                    return ExitCode::FAILURE;
                }
            };
            match host.run(id, &params) {
                Ok(v) => {
                    println!("{v:#}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("erreur : {e}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!("usage : makercraft-cli list | run <commande> [json]");
            ExitCode::FAILURE
        }
    }
}
