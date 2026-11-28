//! « Mettre à jour » : au clic sur le bouton de la barre d'accès rapide, MakerCraft demande à
//! GitHub la dernière version publiée et l'affiche dans une fenêtre : Valider ouvre la page de
//! téléchargement, « Passer cette version » ne la signale plus, Annuler revient. Au démarrage,
//! seulement si l'utilisateur a coché l'option (non par défaut) : vérification silencieuse, le
//! bouton passe en orange. La requête part de `curl`, fourni par Windows 10 et 11.

use std::sync::mpsc::{Receiver, channel};

use eframe::egui::{self, RichText, ScrollArea};
use serde_json::Value;

use crate::about;
use crate::prefs::Prefs;
use crate::theme::{self, Palette};

/// Version publiée sur GitHub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub tag: String,
    pub name: String,
    pub date: String,
    pub notes: String,
    pub url: String,
}

enum State {
    Idle,
    Running(Receiver<Result<Release, String>>),
    Done(Result<Release, String>),
}

pub struct Updater {
    pub open: bool,
    state: State,
}

impl Default for Updater {
    fn default() -> Self {
        Self { open: false, state: State::Idle }
    }
}

/// Étiquette Git de cette version : `v0.1.0-beta.20261121`.
pub fn current_tag() -> String {
    let date: String = about::STAGE.chars().filter(char::is_ascii_digit).collect();
    if date.is_empty() { format!("v{}", env!("CARGO_PKG_VERSION")) } else { format!("v{}-beta.{date}", env!("CARGO_PKG_VERSION")) }
}

/// Clé de comparaison d'une étiquette `v0.1.1-beta.20261128` : (0, 1, 1, 20261128). Une version
/// sans suffixe passe après ses bêtas. None si l'étiquette n'est pas de cette forme.
fn version_key(tag: &str) -> Option<[u64; 4]> {
    let t = tag.trim().trim_start_matches(['v', 'V']);
    let (num, pre) = t.split_once('-').map_or((t, None), |(a, b)| (a, Some(b)));
    let mut k = [0u64; 4];
    let parts: Vec<&str> = num.split('.').collect();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }
    for (i, s) in parts.iter().enumerate() {
        k[i] = s.parse().ok()?;
    }
    k[3] = match pre {
        None => u64::MAX,
        Some(p) => p.chars().filter(char::is_ascii_digit).collect::<String>().parse().unwrap_or(0),
    };
    Some(k)
}

/// L'étiquette `tag` est-elle une version plus récente que `current` ?
fn is_newer(tag: &str, current: &str) -> bool {
    match (version_key(tag), version_key(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

/// `propriétaire/dépôt` d'après l'adresse du dépôt GitHub (vide s'il n'est pas publié).
fn repo_path(url: &str) -> Option<String> {
    let p = url.trim().trim_end_matches('/').trim_end_matches(".git");
    let p = p.strip_prefix("https://github.com/").unwrap_or(p);
    (p.split('/').count() == 2 && !p.is_empty()).then(|| p.to_owned())
}

/// Lit la réponse JSON de l'API GitHub : la liste des versions (la plus récente d'abord,
/// préversions comprises) ou une seule version.
pub fn parse_release(text: &str) -> Result<Release, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("réponse de GitHub illisible : {e}"))?;
    let v = match v {
        Value::Array(list) => list.into_iter().find(|r| !r.get("draft").and_then(Value::as_bool).unwrap_or(false)).ok_or_else(|| "aucune version publiée".to_owned())?,
        other => other,
    };
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let tag = s("tag_name");
    if tag.is_empty() {
        let m = s("message");
        return Err(if m.is_empty() { "aucune version publiée".to_owned() } else { format!("GitHub : {m}") });
    }
    Ok(Release { name: s("name"), date: s("published_at").chars().take(10).collect(), notes: s("body"), url: s("html_url"), tag })
}

fn fetch(repo: &str) -> Result<Release, String> {
    // pas `releases/latest` : il ignore les préversions (bêta), et MakerCraft n'a que des bêtas
    let url = format!("https://api.github.com/repos/{repo}/releases?per_page=5");
    let mut cmd = std::process::Command::new("curl");
    cmd.args(["-s", "-L", "--max-time", "15", "-H", "Accept: application/vnd.github+json", "-H", "User-Agent: MakerCraft", &url]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // pas de fenêtre de console
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().map_err(|e| format!("curl introuvable ({e})"))?;
    if !out.status.success() {
        return Err("pas de réponse de GitHub (connexion internet ?)".to_owned());
    }
    parse_release(&String::from_utf8_lossy(&out.stdout))
}

impl Updater {
    /// Vérification silencieuse (option « au démarrage ») : pas de fenêtre.
    pub fn check_quietly(&mut self) {
        self.start();
        self.open = false;
    }

    /// Lance la vérification et ouvre la fenêtre.
    pub fn start(&mut self) {
        self.open = true;
        let Some(repo) = repo_path(about::GITHUB_REPO) else {
            self.state = State::Done(Err("Le dépôt GitHub de MakerCraft n'est pas encore publié : pas de vérification possible pour l'instant.".to_owned()));
            return;
        };
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(fetch(&repo));
        });
        self.state = State::Running(rx);
    }

    /// Nouvelle version trouvée lors de la dernière vérification (ni l'actuelle, ni passée).
    pub fn newer(&self, prefs: &Prefs) -> Option<&Release> {
        match &self.state {
            State::Done(Ok(r)) if is_newer(&r.tag, &current_tag()) && r.tag != prefs.skipped_version => Some(r),
            _ => None,
        }
    }

    /// Fenêtre ; renvoie vrai si les préférences ont changé (version passée).
    pub fn ui(&mut self, ctx: &egui::Context, pal: &Palette, prefs: &mut Prefs) -> bool {
        if let State::Running(rx) = &self.state
            && let Ok(r) = rx.try_recv()
        {
            self.state = State::Done(r);
        }
        if matches!(self.state, State::Running(_)) {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
        if !self.open {
            return false;
        }
        let mut changed = false;
        let mut close = false;
        let current = current_tag();
        egui::Window::new("Mettre à jour MakerCraft").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.label(RichText::new(format!("Version installée : MakerCraft {}", about::version_label())).color(pal.weak));
            ui.add_space(6.0);
            match &self.state {
                State::Idle | State::Running(_) => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Recherche de la dernière version sur GitHub...");
                    });
                    ctx.request_repaint_after(std::time::Duration::from_millis(200));
                    ui.add_space(6.0);
                    if ui.button("Annuler").clicked() {
                        close = true;
                    }
                }
                State::Done(Err(e)) => {
                    ui.label(e);
                    ui.add_space(6.0);
                    if ui.button("Fermer").clicked() {
                        close = true;
                    }
                }
                // la dernière version publiée n'est pas plus récente que celle installée
                State::Done(Ok(r)) if !is_newer(&r.tag, &current) => {
                    ui.label(RichText::new("MakerCraft est à jour.").font(theme::bold(14.0)).color(pal.ok));
                    ui.add_space(6.0);
                    if ui.button("Fermer").clicked() {
                        close = true;
                    }
                }
                State::Done(Ok(r)) => {
                    let title = if r.name.is_empty() { r.tag.clone() } else { r.name.clone() };
                    ui.label(RichText::new(format!("Nouvelle version : {title}")).font(theme::bold(14.0)).color(pal.orange));
                    if !r.date.is_empty() {
                        ui.label(RichText::new(format!("Publiée le {}", r.date)).color(pal.weak));
                    }
                    if r.tag == prefs.skipped_version {
                        ui.label(RichText::new("Vous aviez choisi de passer cette version.").color(pal.weak));
                    }
                    if !r.notes.is_empty() {
                        ui.add_space(4.0);
                        ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                            ui.label(RichText::new(&r.notes).size(12.5));
                        });
                    }
                    ui.add_space(8.0);
                    ui.label(RichText::new("Valider ouvre la page de téléchargement dans le navigateur.").size(12.0).color(pal.weak));
                    ui.horizontal(|ui| {
                        if ui.button("Valider").clicked() {
                            ctx.open_url(egui::OpenUrl::new_tab(&r.url));
                            close = true;
                        }
                        if ui.button("Passer cette version").clicked() {
                            prefs.skipped_version = r.tag.clone();
                            changed = true;
                            close = true;
                        }
                        if ui.button("Annuler").clicked() {
                            close = true;
                        }
                    });
                }
            }
        });
        if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.open = false;
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_github_answer() {
        let r = parse_release(r#"{"tag_name":"v0.1.1-beta.20261205","name":"v0.1.1 bêta","published_at":"2026-12-05T10:00:00Z","body":"Notes","html_url":"https://github.com/FeBeSoft/makercraft/releases/tag/v0.1.1-beta.20261205"}"#);
        let Ok(r) = r else { panic!("version attendue") };
        assert_eq!(r.tag, "v0.1.1-beta.20261205");
        assert_eq!(r.date, "2026-12-05");
        assert!(parse_release(r#"{"message":"Not Found"}"#).is_err());
        let list = parse_release(r#"[{"tag_name":"v0.2.0-beta.1","draft":true},{"tag_name":"v0.1.0-beta.20261121","prerelease":true}]"#);
        assert_eq!(list.map(|r| r.tag).ok().as_deref(), Some("v0.1.0-beta.20261121"));
    }

    #[test]
    fn repo_and_tag() {
        assert_eq!(repo_path("https://github.com/FeBeSoft/makercraft").as_deref(), Some("FeBeSoft/makercraft"));
        assert_eq!(repo_path(""), None);
        assert!(current_tag().starts_with('v'));
    }

    #[test]
    fn compares_versions() {
        assert!(is_newer("v0.1.1-beta.20261128", "v0.1.0-beta.20261121"));
        assert!(!is_newer("v0.1.0-beta.20261121", "v0.1.1-beta.20261128"));
        assert!(!is_newer("v0.1.1-beta.20261128", "v0.1.1-beta.20261128"));
        assert!(is_newer("v0.1.1-beta.20261205", "v0.1.1-beta.20261128"));
        assert!(is_newer("v0.2.0-beta.1", "v0.1.9-beta.20991231"));
        assert!(is_newer("v0.1.1", "v0.1.1-beta.20261128"));
        assert!(!is_newer("nimporte", "v0.1.0"));
    }
}
