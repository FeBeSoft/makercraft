//! Préférences de l'utilisateur, enregistrées en JSON dans
//! `%APPDATA%\FeBe\MakerCraft\preferences.json` (ou `~/.config/FeBe/MakerCraft` ailleurs).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ateliers::machines::equip::Equipment;

/// Nombre de fichiers récents gardés.
pub const MAX_RECENTS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Units {
    #[default]
    Mm,
    Inch,
}

impl Units {
    pub fn label(self) -> &'static str {
        match self {
            Self::Mm => "mm",
            Self::Inch => "pouce",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeChoice {
    Light,
    #[default]
    Dark,
}

/// Mode d'utilisation : maker (seul à l'atelier), prof ou élève (en classe).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Mode {
    #[default]
    Maker,
    Prof,
    Eleve,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Maker => "maker",
            Self::Prof => "prof",
            Self::Eleve => "élève",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// Mode d'utilisation (choix dans les Options à venir, t100 ; maker par défaut).
    pub mode: Mode,
    /// Équipements déclarés par machine (identifiant du profil de la banque).
    pub equipment: BTreeMap<String, Equipment>,
    pub units: Units,
    pub theme: ThemeChoice,
    /// Fichiers ouverts récemment, le plus récent d'abord.
    pub recents: Vec<PathBuf>,
    /// Dossiers utilisés récemment (ouverture et export), le plus récent d'abord.
    pub recent_dirs: Vec<PathBuf>,
    /// Dernier port série et vitesse utilisés.
    pub port: String,
    pub baud: u32,
    /// Rouvrir au démarrage l'atelier utilisé en dernier.
    pub remember_atelier: bool,
    /// Identifiant du dernier atelier (voir `Atelier::id`).
    pub last_atelier: String,
    /// Dessin : commandes de CadCraft tapées dans la ligne de commande.
    pub cadcraft_commands: bool,
    /// Dessin : marquer l'origine (zéro pièce) dans la vue 2D et dans la vue 3D.
    pub origin_2d: bool,
    pub origin_3d: bool,
    /// Version que l'utilisateur a choisi de passer (« Mettre à jour »).
    pub skipped_version: String,
    /// Vérifier les mises à jour au démarrage (accord de l'utilisateur : non par défaut).
    pub check_updates: bool,
    /// Menu « Extensions » dans la barre de titre (outil de test des commandes, caché par défaut).
    pub extensions_menu: bool,
    /// Dernière variante utilisée de chaque commande du ruban (bouton -> variante).
    pub ribbon_variants: BTreeMap<String, String>,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            mode: Mode::Maker,
            equipment: BTreeMap::new(),
            units: Units::Mm,
            theme: ThemeChoice::Dark,
            recents: Vec::new(),
            recent_dirs: Vec::new(),
            port: String::new(),
            baud: 115_200,
            remember_atelier: true,
            last_atelier: String::new(),
            cadcraft_commands: true,
            origin_2d: true,
            origin_3d: true,
            skipped_version: String::new(),
            check_updates: false,
            extensions_menu: false,
            ribbon_variants: BTreeMap::new(),
        }
    }
}

fn push_front(list: &mut Vec<PathBuf>, p: &Path) {
    list.retain(|q| q != p);
    list.insert(0, p.to_path_buf());
    list.truncate(MAX_RECENTS);
}

impl Prefs {
    pub fn add_recent(&mut self, file: &Path) {
        push_front(&mut self.recents, file);
        if let Some(dir) = file.parent() {
            self.add_recent_dir(dir);
        }
    }

    pub fn add_recent_dir(&mut self, dir: &Path) {
        push_front(&mut self.recent_dirs, dir);
    }

    /// Dossier proposé par défaut dans les fenêtres d'ouverture.
    pub fn start_dir(&self) -> Option<&Path> {
        self.recent_dirs.iter().find(|d| d.is_dir()).map(PathBuf::as_path)
    }

    pub fn dir() -> PathBuf {
        let base = std::env::var_os("APPDATA").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
        base.unwrap_or_else(|| PathBuf::from(".")).join("FeBe").join("MakerCraft")
    }

    fn file() -> PathBuf {
        Self::dir().join("preferences.json")
    }

    /// Lit les préférences ; valeurs par défaut si le fichier manque ou est illisible.
    pub fn load() -> Self {
        std::fs::read_to_string(Self::file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let dir = Self::dir();
        std::fs::create_dir_all(&dir).map_err(|e| format!("{} : {e}", dir.display()))?;
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(Self::file(), text).map_err(|e| format!("{} : {e}", Self::file().display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recents_are_unique_and_bounded() {
        let mut p = Prefs::default();
        for i in 0..12 {
            p.add_recent(Path::new(&format!("C:/a/f{i}.c2d")));
        }
        p.add_recent(Path::new("C:/a/f5.c2d"));
        assert_eq!(p.recents.len(), MAX_RECENTS);
        assert_eq!(p.recents[0], PathBuf::from("C:/a/f5.c2d"));
        assert_eq!(p.recents.iter().filter(|r| r.as_path() == Path::new("C:/a/f5.c2d")).count(), 1);
        assert_eq!(p.recent_dirs, vec![PathBuf::from("C:/a")]);
    }

    #[test]
    fn json_roundtrip_and_defaults() {
        let mut p = Prefs { units: Units::Inch, theme: ThemeChoice::Light, ..Prefs::default() };
        p.add_recent(Path::new("C:/x/y.dxf"));
        let t = serde_json::to_string(&p).unwrap_or_default();
        let back: Prefs = serde_json::from_str(&t).unwrap_or_default();
        assert_eq!(back, p);
        // champ manquant : valeur par défaut
        let partial: Prefs = serde_json::from_str(r#"{"units":"Inch"}"#).unwrap_or_default();
        assert_eq!(partial.units, Units::Inch);
        assert_eq!(partial.baud, 115_200);
        assert_eq!(partial.theme, ThemeChoice::Dark, "thème sombre par défaut");
    }
}
