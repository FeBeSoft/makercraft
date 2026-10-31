//! Intègre l'icône MakerCraft et les informations de version dans l'exécutable Windows.
//! Sans outil de ressources (rc.exe absent), le build continue avec un simple avertissement.

fn main() {
    let icon = "../../assets/branding/makercraft.ico";
    println!("cargo:rerun-if-changed={icon}");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon(icon)
        .set("ProductName", "MakerCraft")
        .set("FileDescription", "MakerCraft — commandes des extensions en ligne de commande")
        .set("LegalCopyright", "GPL-3.0-or-later — MakerCraft contributors");
    if let Err(e) = res.compile() {
        println!("cargo:warning=icône non intégrée : {e}");
    }
}
