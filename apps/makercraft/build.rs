//! Intègre l'icône MakerCraft et les informations de version dans l'exécutable Windows, reprend
//! les fichiers communs aux logiciels FeBe (H:\_src\_commun) et relève dans Cargo.lock la
//! version des bibliothèques affichées dans l'À propos.
//! Sans outil de ressources (rc.exe absent), le build continue avec un simple avertissement.

use std::path::PathBuf;

fn main() {
    // (fichier commun, copie de secours dans assets/, nom dans OUT_DIR)
    for (commun, secours, nom) in [
        ("auteur/bio.txt", "bio.txt", "bio.txt"),
        ("auteur/auteur.jpg", "auteur.jpg", "auteur.jpg"),
        ("liens/liens.txt", "liens.txt", "liens.txt"),
        ("rgpd/rgpd.txt", "rgpd.txt", "rgpd.txt"),
        ("rgpd/reference.txt", "rgpd-reference.txt", "rgpd-reference.txt"),
        ("rgpd/version.txt", "rgpd-version.txt", "rgpd-version.txt"),
    ] {
        fichier_commun(commun, secours, nom);
    }
    versions();
    let icon = "../../assets/branding/makercraft.ico";
    println!("cargo:rerun-if-changed={icon}");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon(icon)
        .set("ProductName", "MakerCraft")
        .set("FileDescription", "MakerCraft - ateliers CNC, Laser, PCB et Machines")
        .set("LegalCopyright", "GPL-3.0-or-later - Emmanuel Firre (FeBe)");
    if let Err(e) = res.compile() {
        println!("cargo:warning=icône non intégrée : {e}");
    }
}

fn manifest() -> PathBuf {
    PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default())
}

fn out_dir() -> PathBuf {
    PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default())
}

/// Copie un fichier commun FeBe dans OUT_DIR ; Cargo recompile dès qu'il change. Sans le dossier
/// commun (dépôt cloné seul), la copie de secours du dépôt est utilisée.
fn fichier_commun(commun: &str, secours: &str, nom: &str) {
    let commun = manifest().join("../../../_commun").join(commun);
    let secours = manifest().join("../../assets").join(secours);
    // surveiller les deux : le fichier commun peut apparaître ou changer à tout moment
    println!("cargo:rerun-if-changed={}", commun.display());
    println!("cargo:rerun-if-changed={}", secours.display());
    let src = if commun.exists() { commun } else { secours };
    let out = out_dir().join(nom);
    if let Err(e) = std::fs::copy(&src, &out) {
        println!("cargo:warning=fichier commun introuvable ({}) : {e}", src.display());
        let _ = std::fs::write(&out, "");
    }
}

/// Versions des bibliothèques principales, lues dans Cargo.lock : une ligne « nom|version ».
fn versions() {
    let lock = manifest().join("../../Cargo.lock");
    println!("cargo:rerun-if-changed={}", lock.display());
    let text = std::fs::read_to_string(&lock).unwrap_or_default();
    let wanted = ["eframe", "egui", "cadcraft-engine", "solidcraft-kernel", "serde", "rusqlite", "geo", "rfd", "image", "serialport"];
    let mut found = Vec::new();
    let mut name = String::new();
    for line in text.lines() {
        if let Some(n) = line.strip_prefix("name = \"") {
            name = n.trim_end_matches('"').to_owned();
        } else if let Some(v) = line.strip_prefix("version = \"")
            && wanted.contains(&name.as_str())
            && !found.iter().any(|(n, _): &(String, String)| *n == name)
        {
            found.push((name.clone(), v.trim_end_matches('"').to_owned()));
        }
    }
    found.sort_by_key(|(n, _)| wanted.iter().position(|w| w == n));
    let body: Vec<String> = found.iter().map(|(n, v)| format!("{n}|{v}")).collect();
    let _ = std::fs::write(out_dir().join("versions.txt"), body.join("\n"));
}
