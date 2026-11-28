//! Textes de la fenêtre « À propos », organisée selon la règle FeBe du 15/11/2026 (Le logiciel,
//! Auteur, Droits d'auteur, RGPD, Sources, Mises à jour, Contact). Les éléments communs aux
//! logiciels FeBe (bio, photo, liens, RGPD) viennent de H:\_src\_commun, repris à la compilation.

pub const AUTHOR: &str = "Emmanuel Firre";
pub const COPYRIGHT: &str = "© 2026 Emmanuel Firre";
pub const LICENSE: &str = "GPL-3.0 ou ultérieure";
pub const SITE: &str = "https://makercraft.fe-be.fr/";
/// Adresse propre au logiciel (les autres contacts FeBe sont sur son site).
pub const CONTACT: &str = "makercraft@fe-be.fr";
/// Étape de développement affichée avec le numéro de version.
pub const STAGE: &str = "bêta 20261128";

/// « v0.1.0 bêta ».
pub fn version_label() -> String {
    format!("v{} {STAGE}", env!("CARGO_PKG_VERSION"))
}
/// Dépôt GitHub du logiciel (publié le 21/11/2026).
pub const GITHUB_REPO: &str = "https://github.com/FeBeSoft/makercraft";
pub const GITHUB_ACCOUNT: &str = "https://github.com/FeBeSoft";

pub const TAGLINE: &str = "Ateliers CNC, Laser, PCB et Machines pour le fablab et l'enseignement de la Technologie";

/// Présentation du logiciel (onglet « Le logiciel »).
pub const PRESENTATION: &str = "MakerCraft réunit dans un seul logiciel libre le dessin (DAO 2D, CAO 3D), la préparation de l'usinage (FAO) et le pilotage des machines du fablab : fraiseuse CNC, laser et bientôt circuits imprimés. Chaque atelier peut aussi fonctionner seul. Il s'adresse aux makers comme aux classes de technologie, avec un mode élève, un mode prof et une banque d'outils et de paramètres de coupe éprouvés à l'atelier.";

/// Bio commune à tous les logiciels FeBe (_commun\auteur\bio.txt), liens `[libellé](adresse)`.
pub const BIO: &str = include_str!(concat!(env!("OUT_DIR"), "/bio.txt"));
/// Photo de l'auteur (_commun\auteur\auteur.jpg).
pub const AUTHOR_JPG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/auteur.jpg"));
/// Liens communs FeBe (_commun\liens\liens.txt), une ligne « libellé|adresse ».
const LIENS: &str = include_str!(concat!(env!("OUT_DIR"), "/liens.txt"));
/// Déclaration RGPD commune (_commun\rgpd\rgpd.txt).
pub const RGPD_COMMUN: &str = include_str!(concat!(env!("OUT_DIR"), "/rgpd.txt"));
/// Texte du RGPD en vigueur (_commun\rgpd\reference.txt).
const RGPD_REFERENCE: &str = include_str!(concat!(env!("OUT_DIR"), "/rgpd-reference.txt"));
/// Version de la déclaration RGPD FeBe (_commun\rgpd\version.txt).
const RGPD_VERSION: &str = include_str!(concat!(env!("OUT_DIR"), "/rgpd-version.txt"));
/// Versions des bibliothèques, relevées dans Cargo.lock par build.rs (« nom|version »).
const VERSIONS: &str = include_str!(concat!(env!("OUT_DIR"), "/versions.txt"));

pub fn rgpd_reference() -> &'static str {
    RGPD_REFERENCE.trim()
}

pub fn rgpd_version() -> &'static str {
    RGPD_VERSION.trim()
}

/// Lignes « libellé|adresse », sans les lignes vides ni les commentaires.
fn pairs(text: &'static str) -> Vec<(&'static str, &'static str)> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once('|'))
        .map(|(a, b)| (a.trim(), b.trim()))
        .collect()
}

/// Liens communs FeBe dans l'ordre du fichier (FeBe, Patreon, Facebook).
pub fn liens() -> Vec<(&'static str, &'static str)> {
    pairs(LIENS)
}

/// Adresse d'un lien commun par son libellé (ex. « Facebook »).
pub fn lien(label: &str) -> Option<&'static str> {
    liens().into_iter().find(|(l, _)| l.eq_ignore_ascii_case(label)).map(|(_, u)| u)
}

/// Texte des droits sans les deux premières lignes (nom et ©), affichées à part.
pub fn rights_body() -> &'static str {
    RIGHTS.splitn(3, '\n').nth(2).unwrap_or(RIGHTS).trim_start()
}

/// Bibliothèques et leur version, lues dans Cargo.lock à la compilation.
pub fn versions() -> Vec<(&'static str, &'static str)> {
    pairs(VERSIONS)
}

/// Version d'une bibliothèque (« ? » si absente de Cargo.lock).
pub fn version_of(name: &str) -> &'static str {
    versions().into_iter().find(|(n, _)| *n == name).map_or("?", |(_, v)| v)
}

/// Bibliothèque principale, pour la fiche du logiciel (« egui / eframe 0.37.0 »).
pub fn main_library() -> String {
    format!("egui / eframe {}", version_of("eframe"))
}

/// Ce que fait MakerCraft en particulier, affiché après la déclaration commune.
pub const RGPD_MAKERCRAFT: &str = "MakerCraft en particulier : il ne fait aucune connexion réseau de lui-même. Il dialogue avec les machines (CNC, laser) uniquement par les ports USB ou série de votre ordinateur. Dessins, projets, banque d'outils, réglages et fichiers récents sont enregistrés sur votre ordinateur et n'en sortent que si vous les envoyez vous-même. La vérification des mises à jour interroge GitHub (GitHub Inc., États-Unis) : quand vous cliquez sur « Mettre à jour », et au démarrage seulement si vous avez coché l'option « Vérifier les mises à jour au démarrage » (Options > Général > Démarrage, non cochée par défaut ; vous pouvez la décocher à tout moment). La requête demande la dernière version publiée ; seule l'adresse IP de votre ordinateur est transmise à GitHub, comme pour toute visite d'un site, et MakerCraft n'envoie aucune autre donnée.";

/// Texte complet de l'onglet « RGPD ».
pub fn rgpd() -> String {
    format!("{}\n\n{RGPD_MAKERCRAFT}", RGPD_COMMUN.trim_end())
}

/// Paragraphe propre à MakerCraft, affiché après la bio commune.
pub const ORIGIN: &str = "MakerCraft est né de là : réunir dans un seul logiciel libre le dessin, la fabrication et le pilotage des machines du fablab, en reprenant l'esprit des ateliers de la suite Galaad, avec des outils modernes et ouverts.";

pub const THANKS: &str = "Merci à tous ceux dont le travail a rendu MakerCraft possible :

- les auteurs de CadCraft et de SolidCraft, dont MakerCraft utilise la lecture DXF et le noyau 3D ;
- les auteurs de LaserGRBL, de Candle et d'Universal G-Code Sender, dont le pilotage des machines Grbl s'inspire ;
- les équipes de Grbl et de grblHAL ;
- Emil Ernerfeldt et les contributeurs d'egui et d'eframe, la bibliothèque d'interface de tous les outils FeBe ;
- les concepteurs de la suite Galaad, qui a équipé tant d'ateliers de technologie ;
- les équipes de Rust, serde, rusqlite et geo.

Et merci à vous d'utiliser MakerCraft : vos retours (onglet Contact) aident à l'améliorer.";

pub const RIGHTS: &str = "MakerCraft
© 2026 Emmanuel Firre

Ce programme est un logiciel libre : vous pouvez le redistribuer et/ou le modifier selon les termes de la Licence publique générale GNU (GPL) publiée par la Free Software Foundation, version 3 de la licence ou (à votre choix) toute version ultérieure.

Ce programme est distribué dans l'espoir qu'il sera utile, mais SANS AUCUNE GARANTIE, sans même la garantie implicite de QUALITÉ MARCHANDE ou d'ADÉQUATION À UN USAGE PARTICULIER. Le texte complet de la licence est dans le fichier LICENSE fourni avec le programme.

En pratique, la GPL-3 impose à toute personne qui redistribue MakerCraft, modifié ou non :
- de conserver les mentions de droits d'auteur (donc le nom de l'auteur) et la licence ;
- de fournir le code source, y compris de ses modifications, sous la même licence ;
- d'indiquer clairement qu'une version a été modifiée.

Les fichiers que vous créez avec MakerCraft (dessins, programmes G-code) vous appartiennent.

Interopérabilité : MakerCraft lit les fichiers d'autres logiciels (Carbide Create ; l'import des dessins Galaad est envisagé) dans le respect de l'article L122-6-1 du Code de la propriété intellectuelle, sans copie de leur code ni contournement de protection. Il n'est affilié ni à ArtCraft, ni aux auteurs de SolidCraft, ni à Carbide 3D.

Composants tiers (licences compatibles avec la GPL-3) :
- CadCraft, SolidCraft : MIT / Apache-2.0, utilisés tels quels
- LaserGRBL, Candle, Universal G-Code Sender : GPL-3, portages
- egui, eframe, serde, rusqlite, geo, rfd, image : MIT / Apache-2.0";

/// (titre, bibliothèque de Cargo.lock pour la version ou "", description, adresse)
pub const SOURCES: &[(&str, &str, &str, &str)] = &[
    ("CadCraft", "cadcraft-engine", "Moteur de dessin et lecture DXF / DWG, utilisé tel quel", "https://github.com/storytold/cadcraft"),
    ("SolidCraft", "solidcraft-kernel", "Noyau 3D B-rep et lecture STEP, utilisé tel quel", "https://github.com/tangency-labs/solidcraft"),
    ("LaserGRBL 7.14.1", "", "Pilotage Grbl et émulateur, portés en Rust", "https://github.com/arkypita/LaserGRBL"),
    ("Candle", "", "G-code et carte de hauteur, portés en Rust", "https://github.com/Denvi/Candle"),
    ("Universal G-Code Sender 2.1.27", "", "Architecture du contrôleur", "https://github.com/winder/Universal-G-Code-Sender"),
    ("egui / eframe", "eframe", "Bibliothèque d'interface", "https://github.com/emilk/egui"),
    ("Rust", "", "Langage de MakerCraft", "https://www.rust-lang.org/"),
];

/// Informations à copier pour un signalement.
pub fn version_info() -> String {
    format!(
        "MakerCraft {} {STAGE}\nCadCraft {}, SolidCraft {}, eframe/egui {}\nSystème : {} {}\nRéglages : {}",
        env!("CARGO_PKG_VERSION"),
        version_of("cadcraft-engine"),
        version_of("solidcraft-kernel"),
        version_of("eframe"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        crate::prefs::Prefs::dir().display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_files_are_embedded_and_texts_have_no_em_dash() {
        assert!(!BIO.trim().is_empty(), "la bio commune doit être reprise à la compilation");
        assert!(!AUTHOR_JPG.is_empty(), "photo de l'auteur");
        assert!(!rgpd_reference().is_empty() && !rgpd_version().is_empty());
        for t in [BIO, ORIGIN, THANKS, RIGHTS, TAGLINE, PRESENTATION] {
            assert!(!t.contains('\u{2014}'), "pas de tiret cadratin");
        }
    }

    #[test]
    fn links_and_versions_are_read() {
        assert!(lien("Facebook").is_some_and(|u| u.starts_with("https://")));
        assert!(lien("Patreon").is_some());
        assert!(liens().iter().all(|(_, u)| u.starts_with("https://")), "liens FeBe en https");
        assert_ne!(version_of("eframe"), "?");
        assert!(main_library().starts_with("egui / eframe 0."));
        assert!(rights_body().starts_with("Ce programme"));
        assert!(version_label().contains("bêta"));
    }
}
