//! makercraft-c2d : lit un projet Carbide Create (.c2d) et produit le G-code pour Grbl.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use makercraft_cam::ext::{load, write_outputs};
use makercraft_cam::job::tool_label;

const HELP: &str = "\
makercraft-c2d — projet Carbide Create (.c2d) vers G-code Grbl

  makercraft-c2d info   projet.c2d                    ce que contient le projet
  makercraft-c2d gcode  projet.c2d [options]          écrit le G-code et un aperçu SVG

options de gcode :
  -o, --sortie DOSSIER   dossier de sortie (par défaut : celui du projet)
  --un-fichier           un seul fichier, pause M0 à chaque changement d'outil
                         (par défaut : un fichier par outil, à lancer après le zéro)
  --seul TEXTE           seulement les parcours dont le nom contient TEXTE

Le G-code chiffré que Carbide Create range dans le .c2d n'est pas utilisé : les parcours
sont recalculés par MakerCraft à partir du dessin et des réglages du projet.
Envoi : makercraft-send --port COM3 \"fichier.nc\"";

fn info(path: &Path) -> Result<(), String> {
    let (pr, job) = load(path, None)?;
    let (w, h, t) = job.stock_size;
    println!("Projet    : {}", job.title);
    println!("Brut      : {w} x {h} x {t} mm   machine du projet : {}", if pr.machine.is_empty() { "?" } else { &pr.machine });
    println!("Zéro      : X0 Y0 coin avant gauche, Z0 dessus du brut ; dégagement Z{}", job.heights.safe_z);
    println!("Formes    : {}   calques : {}", pr.elements.len(), pr.layers.iter().map(|l| l.name.as_str()).collect::<Vec<_>>().join(", "));
    if pr.has_encrypted_gcode() {
        println!("G-code    : celui de Carbide Create est chiffré (ignoré) : MakerCraft recalcule les parcours");
    }
    println!("\nParcours du projet :");
    for tp in &pr.toolpaths {
        let n = pr.elements_for(tp).len();
        let layers: Vec<String> = tp.layers.iter().map(|u| pr.layer_name(u)).collect();
        println!(
            "  {} {:<24} {:<18} {:>4} forme(s) [{}]  prof. {}→{} mm, passe {} mm  {}",
            if tp.enabled { "✔" } else { "✗" },
            tp.name,
            tp.kind.label(),
            n,
            layers.join(", "),
            tp.start_depth,
            tp.end_depth,
            tp.stepdown,
            tool_label(&tp.tool)
        );
    }
    println!("\nG-code prévu :");
    let mut total = 0.0;
    for (i, g) in job.groups.iter().enumerate() {
        let st = g.stats(job.heights);
        total += st.seconds;
        println!("  fichier {} : {}", i + 1, tool_label(&g.tool));
        for op in &g.ops {
            println!("     - {} : {}", op.kind, op.name);
            for w in &op.warnings {
                println!("       ⚠ {w}");
            }
        }
        println!("     usinage {:.1} m, durée estimée {:.0} min", st.cut_length / 1000.0, st.seconds / 60.0);
    }
    if job.groups.len() > 1 {
        println!("  total estimé : {:.0} min (hors changements d'outil)", total / 60.0);
    }
    for w in &job.warnings {
        println!("⚠ {w}");
    }
    for w in &pr.warnings {
        println!("⚠ {w}");
    }
    Ok(())
}

fn gcode(path: &Path, out: Option<PathBuf>, single: bool, only: Option<&str>) -> Result<(), String> {
    let (_, job) = load(path, only)?;
    if job.groups.is_empty() {
        for w in &job.warnings {
            println!("⚠ {w}");
        }
        return Err("aucun parcours à usiner".into());
    }
    let dir = out.unwrap_or_else(|| path.parent().map(Path::to_path_buf).unwrap_or_default());
    let files = write_outputs(&job, &dir, single, true)?;
    for w in job.warnings.iter().chain(job.groups.iter().flat_map(|g| g.ops.iter().flat_map(|o| o.warnings.iter()))) {
        println!("⚠ {w}");
    }
    println!("Fichiers écrits :");
    for f in &files {
        println!("  {}", f.display());
    }
    if job.groups.len() > 1 && !single {
        println!("\nUn fichier par outil : monter l'outil, faire le zéro X Y Z, puis lancer le fichier suivant.");
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut it = args.iter();
    let cmd = it.next().map(String::as_str);
    let mut file = None;
    let mut out = None;
    let mut single = false;
    let mut only = None;
    while let Some(a) = it.next() {
        match a.as_str() {
            "-o" | "--sortie" => out = it.next().map(PathBuf::from),
            "--un-fichier" => single = true,
            "--seul" => only = it.next().cloned(),
            s if s.starts_with('-') => {
                eprintln!("option inconnue : {s}\n\n{HELP}");
                return ExitCode::FAILURE;
            }
            s => file = Some(PathBuf::from(s)),
        }
    }
    let result = match (cmd, file) {
        (Some("info"), Some(f)) => info(&f),
        (Some("gcode"), Some(f)) => gcode(&f, out, single, only.as_deref()),
        _ => {
            println!("{HELP}");
            return if cmd.is_none() || cmd == Some("--help") || cmd == Some("-h") { ExitCode::SUCCESS } else { ExitCode::FAILURE };
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Erreur : {e}");
            ExitCode::FAILURE
        }
    }
}
