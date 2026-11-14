//! Intègre l'icône MakerCraft et les informations de version dans l'exécutable Windows, et
//! reprend la bio de l'auteur commune aux logiciels FeBe.
//! Sans outil de ressources (rc.exe absent), le build continue avec un simple avertissement.

fn main() {
    bio_commune();
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

/// Bio de l'auteur commune à tous les logiciels FeBe : H:\_src\_commun\auteur\bio.txt.
/// Copiée dans OUT_DIR ; Cargo recompile dès qu'elle change. Sans le dossier commun
/// (dépôt cloné seul), la copie de secours assets/bio.txt du dépôt est utilisée.
fn bio_commune() {
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let commun = manifest.join("../../../_commun/auteur/bio.txt");
    let secours = manifest.join("../../assets/bio.txt");
    // surveiller les deux : la bio commune peut apparaître ou changer à tout moment
    println!("cargo:rerun-if-changed={}", commun.display());
    println!("cargo:rerun-if-changed={}", secours.display());
    let src = if commun.exists() { commun } else { secours };
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()).join("bio.txt");
    if let Err(e) = std::fs::copy(&src, &out) {
        println!("cargo:warning=bio introuvable ({}) : {e}", src.display());
        let _ = std::fs::write(&out, "");
    }
}
