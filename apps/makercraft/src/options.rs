//! Fenêtre « Options », sur le principe de GravéoCraft : catégories regroupées à gauche, onglets
//! en haut quand une catégorie a plusieurs pages, une ligne par réglage (libellé et explication
//! à gauche, réglage à droite), boutons Réinitialiser / Annuler / Appliquer.
//!
//! Chaque extension chargée a sa page : elle décrit ses commandes, MakerCraft les affiche.
//! L'« À propos » fait partie des options (groupe Aide).

use std::path::{Path, PathBuf};

use eframe::egui::{self, Align2, Color32, CornerRadius, Rect, RichText, ScrollArea, Sense, Stroke, TextureHandle, Ui, UiBuilder, pos2, vec2};
use makercraft_ext_api::{CommandSpec, Host};

use crate::icons::{self, Icon};
use crate::prefs::{Prefs, ThemeChoice, Units};
use crate::theme::{self, Palette};
use crate::widgets::{self, Kind};

const W: f32 = 900.0;
const H: f32 = 600.0;
const NAV_W: f32 = 236.0;
const HEAD_H: f32 = 44.0;
const FOOT_H: f32 = 58.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    General,
    Files,
    /// Page d'une extension (rang dans `Host::extensions`).
    Extension(usize),
    About,
}

impl Page {
    fn tabs(self) -> &'static [&'static str] {
        match self {
            Self::General => &["Interface", "Démarrage"],
            Self::Files => &["Dossiers", "Récents"],
            Self::Extension(_) => &["Commandes"],
            Self::About => &["Logiciel", "Sources", "Licence"],
        }
    }
}

/// Ce que l'application doit faire après l'affichage.
pub enum Outcome {
    None,
    /// Enregistrer et appliquer ces préférences.
    Apply(Prefs),
    /// Ouvrir la fenêtre d'exécution d'une commande d'extension.
    Run(CommandSpec),
}

pub struct Options {
    pub open: bool,
    page: Page,
    tab: usize,
    /// Préférences en cours de modification (appliquées seulement par « Appliquer »).
    pub draft: Prefs,
}

impl Default for Options {
    fn default() -> Self {
        Self { open: false, page: Page::General, tab: 0, draft: Prefs::default() }
    }
}

fn open_folder(dir: &Path) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(dir).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(dir).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
}

impl Options {
    /// Ouvre la fenêtre sur une page, avec une copie des préférences actuelles.
    pub fn show_page(&mut self, prefs: &Prefs, page: Page) {
        self.draft = prefs.clone();
        self.page = page;
        self.tab = 0;
        self.open = true;
    }

    pub fn ui(&mut self, ctx: &egui::Context, pal: &Palette, host: &Host, about_img: Option<&TextureHandle>) -> Outcome {
        let mut out = Outcome::None;
        let mut close = false;
        egui::Window::new("Options")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_size(vec2(W, H))
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                let full = Rect::from_min_size(ui.max_rect().min, vec2(W, H));
                ui.allocate_rect(full, Sense::hover());
                let head = Rect::from_min_size(full.min, vec2(W, HEAD_H));
                let foot = Rect::from_min_max(pos2(full.left(), full.bottom() - FOOT_H), full.max);
                let nav = Rect::from_min_max(pos2(full.left(), head.bottom()), pos2(full.left() + NAV_W, foot.top()));
                let body = Rect::from_min_max(pos2(nav.right(), head.bottom()), pos2(full.right(), foot.top()));
                // bandeau du haut
                let p = ui.painter().clone();
                p.rect_filled(head, CornerRadius { nw: 10, ne: 10, sw: 0, se: 0 }, pal.bar);
                p.hline(head.x_range(), head.bottom() - 0.5, Stroke::new(1.0, pal.card_line));
                icons::paint(&p, Rect::from_min_size(pos2(head.left() + 16.0, head.center().y - 8.0), vec2(16.0, 16.0)), Icon::Machines, pal.weak);
                p.text(pos2(head.left() + 42.0, head.center().y), Align2::LEFT_CENTER, "Options", theme::bold(14.5), pal.text);
                let xr = Rect::from_center_size(pos2(head.right() - 24.0, head.center().y), vec2(30.0, 26.0));
                let x = ui.interact(xr, ui.scope_id().with("fermer"), Sense::click());
                x.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Fermer"));
                if x.hovered() {
                    p.rect_filled(xr, CornerRadius::same(6), Color32::from_rgb(0xC4, 0x2B, 0x1C));
                }
                let c = xr.center();
                let xs = Stroke::new(1.4, if x.hovered() { Color32::WHITE } else { pal.weak });
                p.line_segment([c + vec2(-5.0, -5.0), c + vec2(5.0, 5.0)], xs);
                p.line_segment([c + vec2(5.0, -5.0), c + vec2(-5.0, 5.0)], xs);
                if x.clicked() {
                    close = true;
                }
                // navigation
                p.rect_filled(nav, CornerRadius::ZERO, pal.panel);
                p.vline(nav.right() - 0.5, nav.y_range(), Stroke::new(1.0, pal.card_line));
                let mut nui = ui.new_child(UiBuilder::new().max_rect(nav.shrink2(vec2(8.0, 10.0))));
                self.nav(&mut nui, pal, host);
                // contenu
                let mut bui = ui.new_child(UiBuilder::new().max_rect(body.shrink2(vec2(24.0, 14.0))));
                if let Some(o) = self.body(&mut bui, pal, host, about_img) {
                    out = o;
                }
                // pied
                p.hline(foot.x_range(), foot.top() + 0.5, Stroke::new(1.0, pal.card_line));
                let mut fui = ui.new_child(
                    UiBuilder::new().max_rect(foot.shrink2(vec2(18.0, 11.0))).layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                if widgets::button(&mut fui, pal, None, "Réinitialiser", Kind::Plain, self.page == Page::General)
                    .on_hover_text("Remet les réglages de cette page à leur valeur d'origine")
                    .clicked()
                {
                    let d = Prefs::default();
                    self.draft.theme = d.theme;
                    self.draft.units = d.units;
                    self.draft.remember_atelier = d.remember_atelier;
                }
                fui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::button(ui, pal, None, "Appliquer", Kind::Primary, true).clicked() {
                        out = Outcome::Apply(self.draft.clone());
                        close = true;
                    }
                    if widgets::button(ui, pal, None, "Annuler", Kind::Plain, true).clicked() {
                        close = true;
                    }
                });
            });
        if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.open = false;
        }
        out
    }

    fn nav(&mut self, ui: &mut Ui, pal: &Palette, host: &Host) {
        ui.spacing_mut().item_spacing.y = 2.0;
        let group = |ui: &mut Ui, t: &str| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(RichText::new(t).font(theme::bold(10.5)).color(pal.dim).extra_letter_spacing(0.9));
            });
            ui.add_space(2.0);
        };
        let mut pick = None;
        group(ui, "APPLICATION");
        for (page, icon, label) in [(Page::General, Icon::Machines, "Général"), (Page::Files, Icon::Open, "Fichiers et dossiers")] {
            if nav_item(ui, pal, icon, label, None, self.page == page) {
                pick = Some(page);
            }
        }
        group(ui, "EXTENSIONS");
        for (i, (id, label)) in host.extensions().iter().enumerate() {
            let short = label.split(" (").next().unwrap_or(label);
            if nav_item(ui, pal, Icon::Plug, short, Some(id), self.page == Page::Extension(i)) {
                pick = Some(Page::Extension(i));
            }
        }
        group(ui, "AIDE");
        if nav_item(ui, pal, Icon::Info, "À propos", None, self.page == Page::About) {
            pick = Some(Page::About);
        }
        if let Some(p) = pick {
            self.page = p;
            self.tab = 0;
        }
    }

    fn body(&mut self, ui: &mut Ui, pal: &Palette, host: &Host, about_img: Option<&TextureHandle>) -> Option<Outcome> {
        let (title, desc) = match self.page {
            Page::General => ("Général", "Apparence et démarrage de MakerCraft."),
            Page::Files => ("Fichiers et dossiers", "Emplacements utilisés par MakerCraft et fichiers récents."),
            Page::Extension(i) => (host.extensions().get(i).map_or("Extension", |e| e.1), "Commandes de l'extension, utilisables aussi en ligne de commande et par script."),
            Page::About => ("À propos", ""),
        };
        ui.label(RichText::new(title).font(theme::bold(19.0)).color(pal.text));
        if !desc.is_empty() {
            ui.label(RichText::new(desc).size(13.0).color(pal.weak));
        }
        if let Page::Extension(i) = self.page
            && let Some((id, _)) = host.extensions().get(i)
        {
            banner(ui, pal, &format!("Page fournie par l'extension « {id} » : elle décrit ses commandes, MakerCraft les affiche."));
        }
        // onglets
        let tabs = self.page.tabs();
        if tabs.len() > 1 {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (k, t) in tabs.iter().enumerate() {
                    if tab(ui, pal, t, self.tab == k) {
                        self.tab = k;
                    }
                }
            });
            let r = ui.available_rect_before_wrap();
            ui.painter().hline(r.x_range(), r.top() - 1.0, Stroke::new(1.0, pal.card_line));
        }
        ui.add_space(6.0);
        let mut out = None;
        ScrollArea::vertical().id_salt(("options", self.page_key(), self.tab)).auto_shrink([false, false]).show(ui, |ui| {
            ui.set_width(ui.available_width() - 6.0);
            match (self.page, self.tab) {
                (Page::General, 0) => self.general_interface(ui, pal),
                (Page::General, _) => {
                    let d = &mut self.draft;
                    widgets::option_row(ui, pal, "Rouvrir le dernier atelier", "Sinon MakerCraft démarre dans l'atelier CNC - DAO / CAO.", |ui| {
                        widgets::switch(ui, pal, &mut d.remember_atelier, "Rouvrir le dernier atelier");
                    });
                }
                (Page::Files, 0) => self.files(ui, pal),
                (Page::Files, _) => self.recents(ui, pal),
                (Page::Extension(i), _) => out = extension(ui, pal, host, i),
                (Page::About, 0) => about_software(ui, pal, host, about_img),
                (Page::About, 1) => about_sources(ui, pal),
                (Page::About, _) => about_license(ui, pal),
            }
        });
        out
    }

    fn page_key(&self) -> usize {
        match self.page {
            Page::General => 0,
            Page::Files => 1,
            Page::About => 2,
            Page::Extension(i) => 10 + i,
        }
    }

    fn general_interface(&mut self, ui: &mut Ui, pal: &Palette) {
        let d = &mut self.draft;
        widgets::option_row(ui, pal, "Thème", "Sombre par défaut. Appliqué avec « Appliquer ».", |ui| {
            egui::ComboBox::from_id_salt("theme").width(180.0).selected_text(if d.theme == ThemeChoice::Dark { "Sombre" } else { "Clair" }).show_ui(ui, |ui| {
                ui.selectable_value(&mut d.theme, ThemeChoice::Dark, "Sombre");
                ui.selectable_value(&mut d.theme, ThemeChoice::Light, "Clair");
            });
        });
        widgets::option_row(ui, pal, "Unités d'affichage", "Cotes, positions et grille. Les fichiers gardent leurs unités.", |ui| {
            egui::ComboBox::from_id_salt("unites").width(180.0).selected_text(if d.units == Units::Mm { "Millimètres" } else { "Pouces" }).show_ui(ui, |ui| {
                ui.selectable_value(&mut d.units, Units::Mm, "Millimètres");
                ui.selectable_value(&mut d.units, Units::Inch, "Pouces");
            });
        });
    }

    fn files(&mut self, ui: &mut Ui, pal: &Palette) {
        let dir = Prefs::dir();
        let path_help = |p: &Path| p.display().to_string();
        widgets::option_row(ui, pal, "Réglages de MakerCraft", &path_help(&dir.join("preferences.json")), |ui| {
            if widgets::button(ui, pal, Some(Icon::Open), "Ouvrir le dossier", Kind::Plain, true).clicked() {
                let _ = std::fs::create_dir_all(&dir);
                open_folder(&dir);
            }
        });
        let dirs: Vec<PathBuf> = self.draft.recent_dirs.clone();
        if dirs.is_empty() {
            widgets::option_row(ui, pal, "Dossiers récents", "Aucun pour l'instant : ils s'ajoutent à l'ouverture et à l'export.", |_| {});
        }
        for (k, d) in dirs.iter().enumerate() {
            let label = if k == 0 { "Dossier récent (proposé à l'ouverture)" } else { "Dossier récent" };
            widgets::option_row(ui, pal, label, &path_help(d), |ui| {
                if widgets::button(ui, pal, Some(Icon::Open), "Ouvrir", Kind::Plain, d.is_dir()).clicked() {
                    open_folder(d);
                }
            });
        }
    }

    fn recents(&mut self, ui: &mut Ui, pal: &Palette) {
        let n = self.draft.recents.len();
        let d = &mut self.draft;
        widgets::option_row(ui, pal, "Fichiers récents", &format!("{n} fichier(s) dans Fichier > Fichiers récents."), |ui| {
            if widgets::button(ui, pal, None, "Vider les listes", Kind::Plain, n > 0 || !d.recent_dirs.is_empty()).clicked() {
                d.recents.clear();
                d.recent_dirs.clear();
            }
        });
        for f in &self.draft.recents {
            let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            widgets::option_row(ui, pal, &name, &f.display().to_string(), |_| {});
        }
    }
}

fn nav_item(ui: &mut Ui, pal: &Palette, icon: Icon, label: &str, tag: Option<&str>, on: bool) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, on, label));
    let p = ui.painter();
    if on {
        p.rect_filled(r, CornerRadius::same(7), pal.blue_soft);
    } else if resp.hovered() {
        p.rect_filled(r, CornerRadius::same(7), pal.hover);
    }
    let fg = if on { pal.blue } else { pal.text };
    icons::paint(p, Rect::from_min_size(pos2(r.left() + 10.0, r.center().y - 8.0), vec2(16.0, 16.0)), icon, if on { pal.blue } else { pal.weak });
    let max = r.width() - 44.0 - if tag.is_some() { 60.0 } else { 0.0 };
    let mut job = egui::text::LayoutJob::simple_singleline(label.to_owned(), if on { theme::bold(13.5) } else { theme::regular(13.5) }, fg);
    job.wrap.max_width = max;
    job.wrap.max_rows = 1;
    p.galley(pos2(r.left() + 36.0, r.center().y - 9.0), p.layout_job(job), fg);
    if let Some(t) = tag {
        let g = p.layout_no_wrap(t.to_owned(), theme::mono(10.5), pal.weak);
        let tr = Rect::from_min_size(pos2(r.right() - 8.0 - g.size().x - 10.0, r.center().y - g.size().y / 2.0 - 1.0), g.size() + vec2(10.0, 2.0));
        p.rect_filled(tr, CornerRadius::same(4), pal.hover);
        p.galley(tr.min + vec2(5.0, 1.0), g, pal.weak);
    }
    resp.clicked()
}

fn tab(ui: &mut Ui, pal: &Palette, t: &str, on: bool) -> bool {
    let font = if on { theme::bold(13.5) } else { theme::regular(13.5) };
    let g = ui.painter().layout_no_wrap(t.to_owned(), font, if on { pal.text } else { pal.weak });
    let (r, resp) = ui.allocate_exact_size(vec2(g.size().x + 26.0, 34.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, on, t));
    let p = ui.painter();
    if resp.hovered() && !on {
        p.rect_filled(r.shrink2(vec2(0.0, 3.0)), CornerRadius::same(6), pal.hover);
    }
    p.galley(pos2(r.left() + 13.0, r.center().y - g.size().y / 2.0), g, pal.text);
    if on {
        p.rect_filled(Rect::from_min_max(pos2(r.left() + 4.0, r.bottom() - 2.0), pos2(r.right() - 4.0, r.bottom())), CornerRadius::same(1), pal.blue);
    }
    resp.clicked()
}

fn banner(ui: &mut Ui, pal: &Palette, text: &str) {
    ui.add_space(6.0);
    egui::Frame::new().fill(pal.blue_soft).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::symmetric(12, 9)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
            icons::paint(ui.painter(), r, Icon::Plug, pal.blue);
            ui.add(egui::Label::new(RichText::new(text).size(12.5).color(pal.text)).wrap());
        });
    });
}

fn extension(ui: &mut Ui, pal: &Palette, host: &Host, i: usize) -> Option<Outcome> {
    let id = host.extensions().get(i)?.0;
    let prefix = format!("{id}.");
    let mut out = None;
    for c in host.commands().iter().filter(|c| c.id.starts_with(&prefix)) {
        let help = format!("{}   ·   menu {}", c.id, c.menu.replace('/', " > "));
        widgets::option_row(ui, pal, c.label, &help, |ui| {
            if widgets::button(ui, pal, Some(Icon::Play), "Exécuter...", Kind::Plain, true).clicked() {
                out = Some(Outcome::Run(c.clone()));
            }
        });
    }
    out
}

fn about_software(ui: &mut Ui, pal: &Palette, host: &Host, img: Option<&TextureHandle>) {
    if let Some(t) = img {
        let w = ui.available_width();
        let s = t.size_vec2();
        let h = (w * s.y / s.x.max(1.0)).min(190.0);
        let (r, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
        // recadrage centré de l'illustration sur toute la largeur
        let vis = (h / (w * s.y / s.x.max(1.0))).min(1.0);
        let uv = Rect::from_min_max(pos2(0.0, (1.0 - vis) / 2.0), pos2(1.0, (1.0 + vis) / 2.0));
        ui.painter().image(t.id(), r, uv, Color32::WHITE);
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("MakerCraft").font(theme::bold(20.0)));
        widgets::pill(ui, concat!("v", env!("CARGO_PKG_VERSION")), pal.blue_soft, pal.blue);
    });
    ui.label(RichText::new("Moteur 2D/3D à ateliers pour le fablab et l'enseignement de la Technologie : DAO, CAO, FAO et pilotage de machines.").color(pal.weak));
    ui.add_space(4.0);
    widgets::option_row(ui, pal, "Auteur", "Emmanuel Firre (FeBe) - fe-be.fr", |_| {});
    widgets::option_row(ui, pal, "Licence", "GPL-3.0-or-later", |_| {});
    let exts: Vec<String> = host.extensions().iter().map(|(id, l)| format!("{id} ({l})")).collect();
    widgets::option_row(ui, pal, "Extensions chargées", &exts.join(", "), |_| {});
}

fn about_sources(ui: &mut Ui, pal: &Palette) {
    for (name, what, url) in [
        ("CadCraft 0.20.1", "MIT/Apache, utilisé tel quel : lecture DXF (cadcraft-dxf).", "github.com/storytold/cadcraft"),
        ("SolidCraft 0.41.0", "MIT/Apache, utilisé tel quel : noyau B-rep, STEP (solidcraft-kernel).", "github.com/tangency-labs/solidcraft"),
        ("LaserGRBL 7.14.1", "GPLv3, porté : pilotage Grbl, émulateur.", "github.com/arkypita/LaserGRBL"),
        ("Candle", "GPLv3, porté : G-code, carte de hauteur.", "github.com/Denvi/Candle"),
        ("Universal G-Code Sender 2.1.27", "GPLv3, porté : architecture du contrôleur.", "github.com/winder/Universal-G-Code-Sender"),
        ("egui / eframe 0.37", "MIT/Apache : interface graphique.", "github.com/emilk/egui"),
    ] {
        widgets::option_row(ui, pal, name, what, |ui| {
            ui.label(RichText::new(url).font(theme::mono(12.0)).color(pal.blue));
        });
    }
}

fn about_license(ui: &mut Ui, pal: &Palette) {
    ui.label(
        RichText::new(
            "MakerCraft est un logiciel libre : vous pouvez le redistribuer et le modifier selon les termes de la licence publique générale GNU (GPL) version 3 ou toute version ultérieure, publiée par la Free Software Foundation.\n\nIl est distribué dans l'espoir d'être utile, mais SANS AUCUNE GARANTIE, sans même la garantie implicite d'une valeur marchande ou d'une adéquation à un usage particulier.\n\nLes composants MIT/Apache intégrés (CadCraft, SolidCraft, egui) gardent leur licence. Projet indépendant, non affilié à ArtCraft ni aux auteurs de SolidCraft.",
        )
        .size(13.5)
        .color(pal.text),
    );
    ui.add_space(8.0);
    ui.label(RichText::new("Texte intégral : fichier LICENSE, ou gnu.org/licenses/gpl-3.0").size(12.5).color(pal.weak));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_page_has_tabs() {
        for p in [Page::General, Page::Files, Page::Extension(0), Page::About] {
            assert!(!p.tabs().is_empty());
        }
    }

    #[test]
    fn opening_copies_the_preferences() {
        let mut o = Options::default();
        let p = Prefs { units: Units::Inch, ..Prefs::default() };
        o.show_page(&p, Page::About);
        assert!(o.open);
        assert_eq!(o.draft, p);
        assert_eq!(o.page, Page::About);
    }
}
