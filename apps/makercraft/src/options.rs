//! Fenêtre « Options », sur le principe de GravéoCraft : catégories regroupées à gauche, onglets
//! en haut quand une catégorie a plusieurs pages, une ligne par réglage (libellé et explication
//! à gauche, réglage à droite), boutons Réinitialiser / Annuler / Appliquer.
//!
//! Chaque extension chargée a sa page : elle décrit ses commandes, MakerCraft les affiche.
//! L'« À propos » fait partie des options (groupe Aide).

use std::path::{Path, PathBuf};

use eframe::egui::{self, Align2, Color32, CornerRadius, Rect, RichText, ScrollArea, Sense, Stroke, TextureHandle, Ui, UiBuilder, pos2, vec2};
use makercraft_ext_api::{CommandSpec, Host};

use crate::about;
use crate::icons::{self, Icon};
use crate::prefs::{Mode, Prefs, ThemeChoice, Units};
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
            Self::About => &["Le logiciel", "L'Auteur", "Droits d'auteur", "RGPD", "Sources", "Mises à jour", "Contact"],
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
    /// Photo de l'auteur (fichier commun FeBe), chargée à la première ouverture de l'onglet.
    author_tex: Option<TextureHandle>,
    /// Préférences à l'ouverture, pour savoir si « Fermer » doit enregistrer quelque chose.
    orig: Prefs,
}

impl Default for Options {
    fn default() -> Self {
        Self { open: false, page: Page::General, tab: 0, draft: Prefs::default(), author_tex: None, orig: Prefs::default() }
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
        self.orig = prefs.clone();
        self.page = page;
        self.tab = 0;
        self.open = true;
    }

    pub fn ui(&mut self, ctx: &egui::Context, pal: &Palette, host: &Host, logo: Option<&TextureHandle>) -> Outcome {
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
                if let Some(o) = self.body(&mut bui, pal, host, logo) {
                    out = o;
                }
                // pied
                p.hline(foot.x_range(), foot.top() + 0.5, Stroke::new(1.0, pal.card_line));
                let mut fui = ui.new_child(
                    UiBuilder::new().max_rect(foot.shrink2(vec2(18.0, 11.0))).layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                if self.page == Page::About {
                    // pages en lecture seule : un seul bouton ; les réglages modifiés ailleurs
                    // avant d'arriver ici sont enregistrés
                    fui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::button(ui, pal, None, "Fermer", Kind::Primary, true).clicked() {
                            if self.draft != self.orig {
                                out = Outcome::Apply(self.draft.clone());
                            }
                            close = true;
                        }
                    });
                } else {
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
                }
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

    fn body(&mut self, ui: &mut Ui, pal: &Palette, host: &Host, logo: Option<&TextureHandle>) -> Option<Outcome> {
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
        if self.page == Page::About && self.tab == 1 && self.author_tex.is_none()
            && let Ok(img) = image::load_from_memory(about::AUTHOR_JPG)
        {
            let img = img.to_rgba8();
            let size = [img.width() as usize, img.height() as usize];
            self.author_tex = Some(ui.ctx().load_texture("auteur", egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()), egui::TextureOptions::LINEAR));
        }
        let author = self.author_tex.clone();
        if self.page == Page::About && matches!(self.tab, 1..=3) {
            // en-tête fixe, seul le texte défile
            match self.tab {
                1 => about_author(ui, pal, author.as_ref()),
                2 => about_rights(ui, pal),
                _ => about_rgpd(ui, pal),
            }
            return None;
        }
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
                (Page::About, 0) => about_software(ui, pal, host, logo),
                (Page::About, 4) => about_sources(ui, pal),
                (Page::About, 5) => about_updates(ui, pal),
                (Page::About, _) => about_contact(ui, pal),
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
        widgets::option_row(
            ui,
            pal,
            "Mode d'utilisation",
            "Maker : tout est permis. Prof et élève : pour la classe ; en mode élève, la fiche machine est en lecture seule, les réglages, la broche, le déverrouillage et la console sont réservés. Le mot de passe du prof viendra ensuite.",
            |ui| {
                let label = |m: Mode| match m {
                    Mode::Maker => "Maker",
                    Mode::Prof => "Prof",
                    Mode::Eleve => "Élève",
                };
                egui::ComboBox::from_id_salt("mode").width(180.0).selected_text(label(d.mode)).show_ui(ui, |ui| {
                    for m in [Mode::Maker, Mode::Prof, Mode::Eleve] {
                        ui.selectable_value(&mut d.mode, m, label(m));
                    }
                });
            },
        );
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

/// Texte long dans un cadre en retrait, avec défilement si besoin.
fn text_box(ui: &mut Ui, pal: &Palette, text: &str) {
    egui::Frame::new()
        .fill(pal.panel)
        .stroke(Stroke::new(1.0, pal.card_line))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add(egui::Label::new(RichText::new(text).size(13.5).color(pal.text)).wrap());
        });
}

/// Comme `text_box`, avec liens intégrés `[libellé](adresse)` (bio commune).
fn text_box_lie(ui: &mut Ui, pal: &Palette, text: &str) {
    egui::Frame::new()
        .fill(pal.panel)
        .stroke(Stroke::new(1.0, pal.card_line))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let style = crate::texte_lie::Style { taille: Some(13.5), couleur: Some(pal.text), couleur_lien: Some(pal.blue), interligne: None };
            crate::texte_lie::afficher(ui, text, style);
        });
}

/// Adresse sans « https:// » ni barre finale, pour l'affichage.
fn short(url: &str) -> &str {
    url.trim_start_matches("https://").trim_start_matches("www.").trim_end_matches('/')
}

/// Petit bouton à icône ouvrant une adresse (adresse en info-bulle).
fn link_icon(ui: &mut Ui, pal: &Palette, icon: Icon, what: &str, url: &str) {
    let (r, resp) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Link, true, what));
    let c = if resp.hovered() { pal.blue } else { pal.weak };
    if resp.hovered() {
        ui.painter().rect_filled(r, CornerRadius::same(6), pal.hover);
    }
    icons::paint(ui.painter(), r.shrink(6.0), icon, c);
    let shown = url.trim_start_matches("mailto:").split('?').next().unwrap_or(url).to_owned();
    if resp.on_hover_text(format!("{what} : {}", short(&shown))).clicked() {
        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
    }
}

/// Onglet « Le logiciel » : icône | fiche (contacts et sites en petites icônes), puis la présentation.
fn about_software(ui: &mut Ui, pal: &Palette, host: &Host, logo: Option<&TextureHandle>) {
    ui.add_space(4.0);
    ui.horizontal_top(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(96.0, 96.0), Sense::hover());
        match logo {
            Some(t) => {
                ui.painter().image(t.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            None => icons::paint(ui.painter(), r, Icon::Machines, pal.orange),
        }
        ui.add_space(14.0);
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("MakerCraft").font(theme::bold(20.0)));
                widgets::pill(ui, &about::version_label(), pal.blue_soft, pal.blue);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    // ordre inversé (de droite à gauche) : contact, site, Facebook, Patreon, GitHub
                    let gh = if about::GITHUB_REPO.is_empty() { about::GITHUB_ACCOUNT } else { about::GITHUB_REPO };
                    link_icon(ui, pal, Icon::Code, "GitHub", gh);
                    if let Some(u) = about::lien("Patreon") {
                        link_icon(ui, pal, Icon::Heart, "Patreon", u);
                    }
                    if let Some(u) = about::lien("Facebook") {
                        link_icon(ui, pal, Icon::People, "Facebook", u);
                    }
                    link_icon(ui, pal, Icon::Globe, "Site", about::SITE);
                    link_icon(ui, pal, Icon::Mail, "Contact", &format!("mailto:{}?subject=MakerCraft", about::CONTACT));
                });
            });
            ui.add_space(4.0);
            let tip = format!("Déclaration RGPD FeBe : version {}", about::rgpd_version());
            let lib = about::main_library();
            egui::Grid::new("fiche-logiciel").num_columns(2).spacing(vec2(16.0, 6.0)).show(ui, |ui| {
                for (k, v, t) in [("Auteur", about::AUTHOR, None), ("Droits", about::COPYRIGHT, None), ("Licence", about::LICENSE, None), ("RGPD", about::rgpd_reference(), Some(tip.as_str())), ("Bibliothèque", lib.as_str(), None)] {
                    ui.label(RichText::new(k).size(13.0).color(pal.weak));
                    let r = ui.label(RichText::new(v).size(13.5));
                    if let Some(t) = t {
                        r.on_hover_text(t);
                    }
                    ui.end_row();
                }
            });
        });
    });
    ui.add_space(10.0);
    ui.label(RichText::new(about::TAGLINE).strong());
    text_box(ui, pal, about::PRESENTATION);
    let exts: Vec<&str> = host.extensions().iter().map(|e| e.0).collect();
    ui.label(RichText::new(format!("Extensions chargées : {}", exts.join(", "))).size(12.5).color(pal.weak));
    ui.add_space(6.0);
    if widgets::button(ui, pal, None, "Copier les informations de version", Kind::Plain, true).clicked() {
        ui.ctx().copy_text(about::version_info());
    }
}

/// Onglet « L'Auteur » : la photo reste en place, la bio défile à côté.
fn about_author(ui: &mut Ui, pal: &Palette, photo: Option<&TextureHandle>) {
    ui.horizontal_top(|ui| {
        if let Some(t) = photo {
            let s = t.size_vec2();
            let w = 150.0;
            let (r, _) = ui.allocate_exact_size(vec2(w, w * s.y / s.x.max(1.0)), Sense::hover());
            ui.painter().image(t.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            ui.add_space(12.0);
        }
        ui.vertical(|ui| {
            ui.label(RichText::new(about::AUTHOR).font(theme::bold(19.0)));
            ScrollArea::vertical().id_salt("a-propos-bio").auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width(ui.available_width() - 6.0);
                text_box_lie(ui, pal, &format!("{}\n\n{}", about::BIO.trim(), about::ORIGIN));
            });
        });
    });
}

/// Onglet « RGPD » : règlement et version de la déclaration sur une ligne fixe, le texte défile.
fn about_rgpd(ui: &mut Ui, pal: &Palette) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Règlement").size(13.0).color(pal.weak));
        ui.label(RichText::new(about::rgpd_reference()).size(13.5));
        ui.add_space(14.0);
        ui.label(RichText::new("Déclaration FeBe").size(13.0).color(pal.weak));
        ui.label(RichText::new(about::rgpd_version()).size(13.5));
    });
    ui.add_space(6.0);
    ScrollArea::vertical().id_salt("a-propos-rgpd").auto_shrink([false, false]).show(ui, |ui| {
        ui.set_width(ui.available_width() - 6.0);
        text_box(ui, pal, &about::rgpd());
    });
}

fn about_sources(ui: &mut Ui, pal: &Palette) {
    for (name, lib, what, url) in about::SOURCES {
        let title = if lib.is_empty() { (*name).to_owned() } else { format!("{name} {}", about::version_of(lib)) };
        widgets::option_row(ui, pal, &title, what, |ui| {
            ui.hyperlink_to(RichText::new(short(url)).font(theme::mono(12.0)).color(pal.blue), *url);
        });
    }
    ui.add_space(8.0);
    ui.label(RichText::new("Remerciements").font(theme::bold(15.0)));
    text_box(ui, pal, about::THANKS);
}

/// Onglet « Droits d'auteur » : nom et droits fixes en haut, le texte défile.
fn about_rights(ui: &mut Ui, pal: &Palette) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("MakerCraft").font(theme::bold(16.0)));
        ui.label(RichText::new(about::COPYRIGHT).size(14.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.hyperlink_to(RichText::new("gnu.org/licenses/gpl-3.0").size(12.5).color(pal.blue), "https://www.gnu.org/licenses/gpl-3.0.html");
            ui.label(RichText::new("Texte intégral : fichier LICENSE ·").size(12.5).color(pal.weak));
        });
    });
    ui.add_space(6.0);
    ScrollArea::vertical().id_salt("a-propos-droits").auto_shrink([false, false]).show(ui, |ui| {
        ui.set_width(ui.available_width() - 6.0);
        text_box(ui, pal, about::rights_body());
    });
}

/// Badge à deux tons, à la manière des badges des dépôts GitHub : nom | version.
fn version_badge(ui: &mut Ui, pal: &Palette, name: &str, version: &str) {
    let font = theme::bold(11.5);
    let gn = ui.painter().layout_no_wrap(name.to_owned(), font.clone(), Color32::WHITE);
    let gv = ui.painter().layout_no_wrap(version.to_owned(), font, Color32::WHITE);
    let (wn, wv) = (gn.size().x + 14.0, gv.size().x + 14.0);
    let h = 20.0;
    let (r, _) = ui.allocate_exact_size(vec2(wn + wv, h), Sense::hover());
    let left = Rect::from_min_size(r.min, vec2(wn, h));
    let right = Rect::from_min_max(pos2(left.right(), r.top()), r.max);
    let p = ui.painter();
    p.rect_filled(left, CornerRadius { nw: 4, sw: 4, ne: 0, se: 0 }, Color32::from_gray(85));
    p.rect_filled(right, CornerRadius { nw: 0, sw: 0, ne: 4, se: 4 }, pal.blue);
    p.galley(pos2(left.left() + 7.0, r.center().y - gn.size().y / 2.0), gn, Color32::WHITE);
    p.galley(pos2(right.left() + 7.0, r.center().y - gv.size().y / 2.0), gv, Color32::WHITE);
}

fn about_updates(ui: &mut Ui, pal: &Palette) {
    widgets::option_row(ui, pal, "MakerCraft", &about::version_label(), |_| {});
    widgets::option_row(ui, pal, "Vérification des mises à jour", "Pas encore disponible : MakerCraft ne fait aucune connexion réseau. Quand elle viendra, elle lira seulement version.json sur makercraft.fe-be.fr et pourra être désactivée.", |_| {});
    ui.add_space(8.0);
    ui.label(RichText::new("Versions des sources utilisées").font(theme::bold(14.0)));
    ui.label(RichText::new("Lues dans Cargo.lock à la compilation.").size(12.0).color(pal.weak));
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        for (name, v) in about::versions() {
            version_badge(ui, pal, name, v);
        }
    });
}

fn about_contact(ui: &mut Ui, pal: &Palette) {
    ui.add_space(4.0);
    text_box(ui, pal, "Pour une idée, un atelier à ajouter ou un problème, écris à l'adresse de MakerCraft en joignant les informations de version. Les autres contacts FeBe sont sur le site de MakerCraft.");
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if widgets::button(ui, pal, None, about::CONTACT, Kind::Primary, true).clicked() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(format!("mailto:{}?subject=MakerCraft", about::CONTACT)));
        }
        if widgets::button(ui, pal, None, "Copier les informations de version", Kind::Plain, true).clicked() {
            ui.ctx().copy_text(about::version_info());
        }
    });
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
