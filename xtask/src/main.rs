//! `cargo xtask upstream [--fetch]` : état des projets amont.
//! `cargo xtask ci` : fmt, clippy, tests (porte de qualité avant chaque commit).

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde::Deserialize;

#[derive(Deserialize)]
struct Upstream {
    #[serde(default)]
    repo: Vec<Repo>,
    #[serde(default)]
    port: Vec<Port>,
}

#[derive(Deserialize)]
struct Repo {
    name: String,
    url: String,
    path: String,
    pinned: String,
    kind: String,
    license: String,
}

#[derive(Deserialize)]
struct Port {
    ours: String,
    repo: String,
    source: String,
    status: String,
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().map(Path::to_path_buf).unwrap_or_default()
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn upstream(fetch: bool) -> Result<(), String> {
    let root = root();
    let text = std::fs::read_to_string(root.join("upstream.toml")).map_err(|e| format!("upstream.toml : {e}"))?;
    let up: Upstream = toml::from_str(&text).map_err(|e| format!("upstream.toml : {e}"))?;

    for r in &up.repo {
        let dir = root.join(&r.path);
        println!("\n== {} ({}, {}) — {}", r.name, r.kind, r.license, r.url);
        if !dir.join(".git").exists() {
            println!("   clone absent : git clone --filter=blob:none {} {}", r.url, dir.display());
            continue;
        }
        if fetch {
            print!("   fetch… ");
            println!("{}", if git(&dir, &["fetch", "--tags", "--quiet", "origin"]).is_some() { "ok" } else { "ÉCHEC" });
        }
        let head = "origin/HEAD";
        let latest_tag = git(&dir, &["describe", "--tags", "--abbrev=0", head]).unwrap_or_else(|| "?".into());
        let behind = git(&dir, &["rev-list", "--count", &format!("{}..{head}", r.pinned)]).unwrap_or_else(|| "?".into());
        println!("   épinglé {}   dernier tag {}   commits amont depuis : {}", r.pinned, latest_tag, behind);

        for p in up.port.iter().filter(|p| p.repo == r.name) {
            let range = format!("{}..{head}", r.pinned);
            match git(&dir, &["diff", "--stat", &range, "--", &p.source]) {
                Some(s) if s.is_empty() => println!("   ✓ {}  [{}]", p.source, p.status),
                Some(s) => {
                    println!("   ⚠ {}  [{}] modifié → {}", p.source, p.status, p.ours);
                    println!("     {}", s.lines().last().unwrap_or(""));
                    println!("     voir : git -C {} diff {range} -- {}", dir.display(), p.source);
                }
                None => println!("   ? {}  (chemin ou épingle introuvable)", p.source),
            }
        }
    }
    Ok(())
}

fn ci() -> Result<(), String> {
    let steps: [&[&str]; 3] =
        [&["fmt", "--all", "--check"], &["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"], &["test", "--workspace"]];
    for args in steps {
        println!("\n>> cargo {}", args.join(" "));
        let ok = Command::new("cargo").args(args).current_dir(root()).status().map(|s| s.success()).unwrap_or(false);
        if !ok {
            return Err(format!("échec : cargo {}", args.join(" ")));
        }
    }
    println!("\nCI : tout est vert.");
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("upstream") => upstream(args.iter().any(|a| a == "--fetch")),
        Some("ci") => ci(),
        _ => Err("usage : cargo xtask upstream [--fetch] | cargo xtask ci".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
