//! Textes de la page « À propos » (sur le modèle de GravéoCraft et d'UIdeCraft).
//! Fichier à modifier librement ; la bio de l'auteur vient du dossier commun FeBe.

pub const AUTHOR: &str = "Emmanuel Firre";
pub const COPYRIGHT: &str = "© 2026-11 Emmanuel Firre";
pub const LICENSE: &str = "GPL-3.0 ou ultérieure";
pub const SITE: &str = "https://makercraft.fe-be.fr/";
pub const CONTACT: &str = "makercraft@fe-be.fr";
pub const FACEBOOK: &str = "https://www.facebook.com/FeBeSoft";
pub const PATREON: &str = "https://www.patreon.com/c/FeBeSoft";
/// Dépôt GitHub : vide tant qu'il n'est pas publié.
pub const GITHUB: &str = "";

pub const TAGLINE: &str = "Ateliers CNC, Laser, PCB et Machines pour le fablab et l'enseignement de la Technologie";

/// Bio commune à tous les logiciels FeBe (H:\_src\_commun\auteur\bio.txt), reprise à la compilation.
pub const BIO: &str = include_str!(concat!(env!("OUT_DIR"), "/bio.txt"));

/// Paragraphe propre à MakerCraft, affiché après la bio commune.
pub const ORIGIN: &str = "MakerCraft est né de là : réunir dans un seul logiciel libre le dessin, la fabrication et le pilotage des machines du fablab, en reprenant l'esprit des ateliers de la suite Galaad, avec des outils modernes et ouverts.";

pub const THANKS: &str = "Merci à tous ceux dont le travail a rendu MakerCraft possible :

• les auteurs de CadCraft et de SolidCraft, dont MakerCraft utilise la lecture DXF et le noyau 3D ;
• les auteurs de LaserGRBL, de Candle et d'Universal G-Code Sender, dont le pilotage des machines Grbl s'inspire ;
• les équipes de Grbl et de grblHAL ;
• Emil Ernerfeldt et les contributeurs d'egui et d'eframe, la bibliothèque d'interface de tous les outils FeBe ;
• les concepteurs de la suite Galaad, qui a équipé tant d'ateliers de technologie ;
• les équipes de Rust, serde, rusqlite et geo.

Et merci à vous d'utiliser MakerCraft : vos retours (onglet Contact) aident à l'améliorer.";

pub const RIGHTS: &str = "MakerCraft
© 2026-11 Emmanuel Firre

Ce programme est un logiciel libre : vous pouvez le redistribuer et/ou le modifier selon les termes de la Licence publique générale GNU (GPL) publiée par la Free Software Foundation, version 3 de la licence ou (à votre choix) toute version ultérieure.

Ce programme est distribué dans l'espoir qu'il sera utile, mais SANS AUCUNE GARANTIE, sans même la garantie implicite de QUALITÉ MARCHANDE ou d'ADÉQUATION À UN USAGE PARTICULIER. Le texte complet de la licence est dans le fichier LICENSE fourni avec le programme.

En pratique, la GPL-3 impose à toute personne qui redistribue MakerCraft, modifié ou non :
• de conserver les mentions de droits d'auteur (donc le nom de l'auteur) et la licence ;
• de fournir le code source, y compris de ses modifications, sous la même licence ;
• d'indiquer clairement qu'une version a été modifiée.

Les fichiers que vous créez avec MakerCraft (dessins, programmes G-code) vous appartiennent.

Interopérabilité : MakerCraft lit les fichiers d'autres logiciels (Carbide Create, Galaad) dans le respect de l'article L122-6-1 du Code de la propriété intellectuelle, sans copie de leur code ni contournement de protection. Il n'est affilié ni à ArtCraft, ni aux auteurs de SolidCraft, ni à Carbide 3D.

Composants tiers (licences compatibles avec la GPL-3) :
• CadCraft, SolidCraft : MIT / Apache-2.0, utilisés tels quels
• LaserGRBL, Candle, Universal G-Code Sender : GPL-3, portages
• egui, eframe, serde, rusqlite, geo, rfd, image : MIT / Apache-2.0";

/// (titre, description, adresse)
pub const SOURCES: &[(&str, &str, &str)] = &[
    ("CadCraft 0.20.1", "Lecture des dessins DXF (cadcraft-dxf), utilisé tel quel", "https://github.com/storytold/cadcraft"),
    ("SolidCraft 0.41.0", "Noyau 3D B-rep et lecture STEP (solidcraft-kernel), utilisé tel quel", "https://github.com/tangency-labs/solidcraft"),
    ("LaserGRBL 7.14.1", "Pilotage Grbl et émulateur, portés en Rust", "https://github.com/arkypita/LaserGRBL"),
    ("Candle", "G-code et carte de hauteur, portés en Rust", "https://github.com/Denvi/Candle"),
    ("Universal G-Code Sender 2.1.27", "Architecture du contrôleur", "https://github.com/winder/Universal-G-Code-Sender"),
    ("egui / eframe 0.37", "Bibliothèque d'interface", "https://github.com/emilk/egui"),
    ("Rust", "Langage de MakerCraft", "https://www.rust-lang.org/"),
];

/// Informations à copier pour un signalement.
pub fn version_info() -> String {
    format!(
        "MakerCraft {}\nCadCraft 0.20.1, SolidCraft 0.41.0, eframe/egui 0.37\nSystème : {} {}\nRéglages : {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        crate::prefs::Prefs::dir().display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bio_is_embedded_and_texts_have_no_em_dash() {
        assert!(!BIO.trim().is_empty(), "la bio commune doit être reprise à la compilation");
        for t in [BIO, ORIGIN, THANKS, RIGHTS, TAGLINE] {
            assert!(!t.contains('\u{2014}'), "pas de tiret cadratin");
        }
    }
}
