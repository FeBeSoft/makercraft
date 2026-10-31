//! Dictionnaires des codes Grbl : erreurs, alarmes, réglages `$`.
//! Données : LaserGRBL 7.14.1, `LaserGRBL/CSV/*.csv` (voir `data/README.md`).

use std::sync::OnceLock;

/// Un code d'erreur ou d'alarme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeInfo {
    pub code: u16,
    pub brief: &'static str,
    pub description: &'static str,
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

fn parse_codes(csv: &'static str) -> Vec<CodeInfo> {
    csv.lines()
        .filter_map(|line| {
            let mut f = line.splitn(3, ',');
            let code = f.next()?.trim().parse().ok()?;
            Some(CodeInfo { code, brief: f.next()?.trim(), description: f.next().unwrap_or("").trim() })
        })
        .collect()
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

/// Description d'un `error:N`.
pub fn error(code: u16) -> Option<CodeInfo> {
    ERRORS.get_or_init(|| parse_codes(ERRORS_CSV)).iter().find(|c| c.code == code).copied()
}

/// Description d'un `ALARM:N`.
pub fn alarm(code: u16) -> Option<CodeInfo> {
    ALARMS.get_or_init(|| parse_codes(ALARMS_CSV)).iter().find(|c| c.code == code).copied()
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
}
