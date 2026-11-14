//! Hôte d'extensions de l'application : charge les extensions compilées avec MakerCraft,
//! construit le menu « Extensions » à partir de leurs `CommandSpec` et exécute une commande
//! avec ses paramètres JSON dans une petite fenêtre.

use std::collections::BTreeMap;

use eframe::egui::{self, RichText, ScrollArea, TextEdit, vec2};
use makercraft_ext_api::{CommandSpec, Extension, Host, Value};

use crate::theme::{self, Palette};

/// Extensions compilées dans cette version de MakerCraft.
pub fn builtin() -> Vec<Box<dyn Extension>> {
    vec![
        Box::new(makercraft_ext_machine::MachineExtension),
        Box::new(makercraft_c2d::C2dExtension),
        Box::new(makercraft_cam::CamExtension),
        Box::new(makercraft_dao::DaoExtension),
        Box::new(makercraft_cao::CaoExtension),
        Box::new(makercraft_mkc::MkcExtension),
    ]
}

/// Charge les extensions ; une erreur est rapportée sans empêcher le démarrage.
pub fn load() -> (Host, Option<String>) {
    let mut host = Host::new();
    let err = host.load(builtin()).err().map(|e| e.to_string());
    (host, err)
}

/// Arbre de menus : chemin `A/B` -> commandes.
pub fn menu_tree(host: &Host) -> BTreeMap<String, Vec<CommandSpec>> {
    let mut m: BTreeMap<String, Vec<CommandSpec>> = BTreeMap::new();
    for c in host.commands().iter() {
        m.entry(c.menu.to_owned()).or_default().push(c.clone());
    }
    m
}

/// Fenêtre d'exécution d'une commande.
pub struct CommandDialog {
    pub id: &'static str,
    pub label: &'static str,
    pub params: String,
    pub result: Option<Result<String, String>>,
}

impl CommandDialog {
    pub fn new(spec: &CommandSpec) -> Self {
        Self { id: spec.id, label: spec.label, params: "{}".to_owned(), result: None }
    }

    /// Affiche la fenêtre ; renvoie `false` quand elle doit être fermée.
    pub fn show(&mut self, ctx: &egui::Context, host: &Host, pal: &Palette) -> bool {
        let mut open = true;
        egui::Window::new(self.label)
            .collapsible(false)
            .resizable(false)
            .fixed_size(vec2(520.0, 360.0))
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(RichText::new(self.id).font(theme::mono(13.0)).color(pal.blue));
                ui.add_space(4.0);
                ui.label("Paramètres (JSON)");
                ui.add(TextEdit::multiline(&mut self.params).font(theme::mono(13.0)).desired_rows(4).desired_width(f32::INFINITY));
                if ui.button("Exécuter").clicked() {
                    self.result = Some(match serde_json::from_str::<Value>(&self.params) {
                        Err(e) => Err(format!("JSON invalide : {e}")),
                        Ok(p) => host.run(self.id, &p).map(|v| serde_json::to_string_pretty(&v).unwrap_or_default()).map_err(|e| e.to_string()),
                    });
                }
                if let Some(r) = &self.result {
                    ui.add_space(6.0);
                    let (text, color) = match r {
                        Ok(t) => (t.as_str(), pal.text),
                        Err(e) => (e.as_str(), pal.bad),
                    };
                    ScrollArea::vertical().max_height(170.0).show(ui, |ui| {
                        ui.label(RichText::new(text).font(theme::mono(12.5)).color(color));
                    });
                }
            });
        open
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_builtin_extensions_load() {
        let (host, err) = load();
        assert_eq!(err, None);
        let ids: Vec<&str> = host.extensions().iter().map(|e| e.0).collect();
        for id in ["machine", "c2d", "cam", "dao", "cao", "mkc"] {
            assert!(ids.contains(&id), "{ids:?}");
        }
        let tree = menu_tree(&host);
        assert!(tree.keys().any(|k| k.starts_with("Machine")), "{:?}", tree.keys());
    }
}
