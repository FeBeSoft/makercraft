//! Dictionnaires des codes Grbl : erreurs, alarmes, réglages `$`.
//! Textes anglais : LaserGRBL 7.14.1, `LaserGRBL/CSV/*.csv` (voir `data/README.md`).
//! Textes français (libellé, cause probable, quoi faire) : rédigés pour MakerCraft (`codes/fr.rs`).

use std::sync::OnceLock;

mod fr;

/// Un code d'erreur ou d'alarme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeInfo {
    pub code: u16,
    /// Libellé anglais d'origine (vide si le code n'est connu qu'en français).
    pub brief: &'static str,
    pub description: &'static str,
    /// Libellé français court.
    pub fr: &'static str,
    /// Cause probable.
    pub cause: &'static str,
    /// Quoi faire.
    pub action: &'static str,
}

impl CodeInfo {
    /// Libellé à afficher : le français s'il existe, sinon l'anglais.
    pub fn label(&self) -> &'static str {
        if self.fr.is_empty() { self.brief } else { self.fr }
    }

    /// Libellé, cause et action sur une ligne (console).
    pub fn explain(&self) -> String {
        let mut s = self.label().to_owned();
        if !self.cause.is_empty() {
            s.push_str(" - cause probable : ");
            s.push_str(self.cause);
        }
        if !self.action.is_empty() {
            s.push_str(" - à faire : ");
            s.push_str(self.action);
        }
        s
    }
}

/// Un réglage `$n`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingInfo {
    pub id: u16,
    pub name: &'static str,
    pub unit: &'static str,
    pub description: &'static str,
}

const ERRORS_CSV: &str = include_str!("../data/error_codes.csv");
const ALARMS_CSV: &str = include_str!("../data/alarm_codes.csv");
const SETTINGS_CSV: &str = include_str!("../data/setting_codes.v1.1.csv");

/// (code, libellé, cause probable, quoi faire)
type FrTable = &'static [(u16, &'static str, &'static str, &'static str)];

fn parse_codes(csv: &'static str, table: FrTable) -> Vec<CodeInfo> {
    let mut v: Vec<CodeInfo> = csv
        .lines()
        .filter_map(|line| {
            let mut f = line.splitn(3, ',');
            let code = f.next()?.trim().parse().ok()?;
            Some(CodeInfo { code, brief: f.next()?.trim(), description: f.next().unwrap_or("").trim(), fr: "", cause: "", action: "" })
        })
        .collect();
    for &(code, fr, cause, action) in table {
        match v.iter_mut().find(|c| c.code == code) {
            Some(c) => {
                c.fr = fr;
                c.cause = cause;
                c.action = action;
            }
            None => v.push(CodeInfo { code, brief: "", description: "", fr, cause, action }),
        }
    }
    v.sort_by_key(|c| c.code);
    v
}

fn parse_settings(csv: &'static str) -> Vec<SettingInfo> {
    csv.lines()
        .filter_map(|line| {
            let mut f = line.splitn(4, ',');
            let id = f.next()?.trim().parse().ok()?; // l'en-tête n'est pas numérique : ignoré
            Some(SettingInfo { id, name: f.next()?.trim(), unit: f.next().unwrap_or("").trim(), description: f.next().unwrap_or("").trim() })
        })
        .collect()
}

static ERRORS: OnceLock<Vec<CodeInfo>> = OnceLock::new();
static ALARMS: OnceLock<Vec<CodeInfo>> = OnceLock::new();
static SETTINGS: OnceLock<Vec<SettingInfo>> = OnceLock::new();

/// Toutes les erreurs connues.
pub fn errors() -> &'static [CodeInfo] {
    ERRORS.get_or_init(|| parse_codes(ERRORS_CSV, fr::ERRORS))
}

/// Toutes les alarmes connues.
pub fn alarms() -> &'static [CodeInfo] {
    ALARMS.get_or_init(|| parse_codes(ALARMS_CSV, fr::ALARMS))
}

/// Description d'un `error:N`.
pub fn error(code: u16) -> Option<CodeInfo> {
    errors().iter().find(|c| c.code == code).copied()
}

/// Description d'un `ALARM:N`.
pub fn alarm(code: u16) -> Option<CodeInfo> {
    alarms().iter().find(|c| c.code == code).copied()
}

/// Description d'un réglage `$n` (Grbl 1.1).
pub fn setting(id: u16) -> Option<SettingInfo> {
    SETTINGS.get_or_init(|| parse_settings(SETTINGS_CSV)).iter().find(|s| s.id == id).copied()
}

/// Tous les réglages connus (Grbl 1.1).
pub fn settings() -> &'static [SettingInfo] {
    SETTINGS.get_or_init(|| parse_settings(SETTINGS_CSV))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookups() {
        assert_eq!(error(1).map(|c| c.brief), Some("Expected command letter"));
        assert_eq!(alarm(1).map(|c| c.brief), Some("Hard limit"));
        assert!(error(9999).is_none());
        let s = setting(110).map(|s| s.unit);
        assert!(s.is_some_and(|u| u.contains("mm")), "{s:?}");
        assert!(settings().len() > 30);
        assert!(settings().iter().all(|s| !s.name.is_empty()));
    }

    #[test]
    fn every_code_has_french() {
        for c in errors().iter().chain(alarms()) {
            for t in [c.fr, c.cause, c.action] {
                assert!(!t.trim().is_empty(), "code {} : texte français manquant", c.code);
                assert!(!t.contains('\u{2014}'), "code {} : tiret cadratin", c.code);
            }
        }
        assert!((1..=10).all(|n| alarm(n).is_some()), "alarmes 1 à 10");
        assert!((1..=38).filter(|n| *n != 19).all(|n| error(n).is_some()), "erreurs 1 à 38");
    }

    #[test]
    fn french_labels() {
        assert_eq!(alarm(1).map(|c| c.label()), Some("Fin de course atteinte"));
        assert_eq!(error(9).map(|c| c.label()), Some("Verrouillé : machine en alarme"));
        assert_eq!(error(22).map(|c| c.label()), Some("Avance non définie"));
        let e = error(22).map(|c| c.explain()).unwrap_or_default();
        assert!(e.contains("cause probable") && e.contains("à faire") && e.contains('F'), "{e}");
    }
}
