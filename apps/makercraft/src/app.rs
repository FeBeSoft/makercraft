//! Fenêtre principale : barre de menus, barre des ateliers, contenu de l'atelier actif, barre
//! d'état ; fenêtres « À propos », « Préférences » et exécution d'une commande d'extension.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use eframe::egui::{
    self, Align2, Color32, ColorImage, CornerRadius, Key, KeyboardShortcut, Modifiers, Rect, RichText, Stroke, TextureHandle, TextureOptions, Ui, UiBuilder,
    pos2, vec2,
};
use makercraft_ext_api::{CommandSpec, Host};

use crate::ateliers::{self, Atelier, Family, cnc, machines};
use crate::hote::{self, CommandDialog};
use crate::icons::{self, Icon};
use crate::prefs::{Prefs, ThemeChoice, Units};
use crate::theme::{self, Palette};
use crate::widgets;

pub const ICON_PNG: &[u8] = include_bytes!("../../../assets/branding/makercraft-256.png");
const ABOUT_JPG: &[u8] = include_bytes!("../../../assets/branding/makercraft-about.jpg");

const MENU_H: f32 = 34.0;
const ATELIER_H: f32 = 60.0;
const STATUS_H: f32 = 32.0;

pub struct App {
    prefs: Prefs,
    applied_theme: Option<ThemeChoice>,
    host: Host,
    host_error: Option<String>,
    menus: BTreeMap<String, Vec<CommandSpec>>,
    atelier: Atelier,
    cnc: cnc::Cnc,
    machines: machines::Machines,
    show_about: bool,
    show_prefs: bool,
    command: Option<CommandDialog>,
    logo: Option<TextureHandle>,
    about_img: Option<TextureHandle>,
    textures_tried: bool,
    status: String,
    /// Images déjà affichées (les premières sont redemandées : la fenêtre apparaît après elles).
    frames: u32,
}

enum Action {
    OpenDialog,
    Open(PathBuf),
    Atelier(Atelier),
    About,
    Prefs,
    Command(CommandSpec),
    Quit,
}

impl App {
    pub fn new(files: Vec<PathBuf>) -> Self {
        let prefs = Prefs::load();
        let (host, host_error) = hote::load();
        let menus = hote::menu_tree(&host);
        let mut machines = machines::Machines::with_port(prefs.port.clone(), prefs.baud);
        machines.refresh_ports();
        let mut app = Self {
            prefs,
            applied_theme: None,
            host,
            host_error,
            menus,
            atelier: Atelier::CncDao,
            cnc: cnc::Cnc::default(),
            machines,
            show_about: false,
            show_prefs: false,
            command: None,
            logo: None,
            about_img: None,
            textures_tried: false,
            status: "Prêt".to_owned(),
            frames: 0,
        };
        if let Some(e) = &app.host_error {
            app.status = format!("Extensions : {e}");
        }
        for f in files.into_iter().take(1) {
            app.open(&f);
        }
        app
    }

    fn pal(&self) -> &'static Palette {
        match self.prefs.theme {
            ThemeChoice::Light => &theme::LIGHT,
            ThemeChoice::Dark => &theme::DARK,
        }
    }

    fn save_prefs(&mut self) {
        if let Err(e) = self.prefs.save() {
            self.status = format!("Préférences non enregistrées : {e}");
        }
    }

    fn open(&mut self, path: &Path) {
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let r = if matches!(ext.as_str(), "nc" | "gcode" | "ngc" | "tap") {
            self.atelier = Atelier::Machines;
            self.machines.load_program(path)
        } else {
            if self.atelier.family() != Family::Cnc {
                self.atelier = Atelier::CncDao;
            }
            self.cnc.open(path)
        };
        match r {
            Ok(msg) => {
                self.status = msg;
                self.prefs.add_recent(path);
                self.save_prefs();
            }
            Err(e) => self.status = format!("Ouverture impossible : {e}"),
        }
    }

    fn open_dialog(&mut self) {
        let mut d = rfd::FileDialog::new().set_title("Ouvrir");
        for (name, exts) in cnc::open_filters() {
            d = d.add_filter(name, &exts);
        }
        d = d.add_filter("Programme G-code", &["nc", "gcode", "ngc", "tap"]);
        if let Some(dir) = self.prefs.start_dir() {
            d = d.set_directory(dir);
        }
        if let Some(p) = d.pick_file() {
            self.open(&p);
        }
    }

    fn load_textures(&mut self, ctx: &egui::Context) {
        if self.textures_tried {
            return;
        }
        self.textures_tried = true;
        if let Ok(icon) = eframe::icon_data::from_png_bytes(ICON_PNG) {
            let img = ColorImage::from_rgba_unmultiplied([icon.width as usize, icon.height as usize], &icon.rgba);
            self.logo = Some(ctx.load_texture("logo", img, TextureOptions::LINEAR));
        }
        if let Ok(img) = image::load_from_memory(ABOUT_JPG) {
            let rgba = img.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            self.about_img = Some(ctx.load_texture("a-propos", ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()), TextureOptions::LINEAR));
        }
    }

    fn run(&mut self, a: Action, ctx: &egui::Context) {
        match a {
            Action::OpenDialog => self.open_dialog(),
            Action::Open(p) => self.open(&p),
            Action::Atelier(at) => {
                self.atelier = at;
                if at == Atelier::Machines && self.machines.port.is_empty() {
                    self.machines.refresh_ports();
                }
            }
            Action::About => self.show_about = true,
            Action::Prefs => self.show_prefs = true,
            Action::Command(spec) => self.command = Some(CommandDialog::new(&spec)),
            Action::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.applied_theme != Some(self.prefs.theme) {
            self.applied_theme = Some(self.prefs.theme);
            theme::apply(&ctx, self.pal());
        }
        self.load_textures(&ctx);
        if self.frames < 10 {
            self.frames += 1;
            ctx.request_repaint();
        }
        self.machines.poll(&ctx);
        let pal = self.pal();
        let mut actions: Vec<Action> = Vec::new();
        // fichiers déposés dans la fenêtre
        let dropped: Vec<PathBuf> =
            ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        actions.extend(dropped.into_iter().take(1).map(Action::Open));
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::O))) {
            actions.push(Action::OpenDialog);
        }

        egui::CentralPanel::default().frame(egui::Frame::new().fill(pal.window)).show(ui, |ui| {
            let full = ui.max_rect();
            let menu = Rect::from_min_size(full.min, vec2(full.width(), MENU_H));
            let bar = Rect::from_min_size(pos2(full.left(), menu.bottom()), vec2(full.width(), ATELIER_H));
            let status = Rect::from_min_max(pos2(full.left(), full.bottom() - STATUS_H), full.max);
            let body = Rect::from_min_max(pos2(full.left() + 20.0, bar.bottom() + 18.0), pos2(full.right() - 20.0, status.top() - 16.0));
            self.menu_bar(ui, menu, pal, &mut actions);
            self.atelier_bar(ui, bar, pal, &mut actions);
            self.status_bar(ui, status, pal);
            let mut b = ui.new_child(UiBuilder::new().max_rect(body));
            self.body(&mut b, pal);
        });

        if self.show_about {
            self.about_window(&ctx, pal);
        }
        if self.show_prefs {
            self.prefs_window(&ctx, pal);
        }
        if let Some(d) = &mut self.command
            && !d.show(&ctx, &self.host, pal)
        {
            self.command = None;
        }
        if !actions.is_empty() && !ctx.will_discard() {
            for a in actions {
                self.run(a, &ctx);
            }
            ctx.request_repaint();
        }
    }
}

impl App {
    fn menu_bar(&self, ui: &mut Ui, r: Rect, pal: &Palette, actions: &mut Vec<Action>) {
        ui.painter().rect_filled(r, CornerRadius::ZERO, pal.bar);
        let mut m = ui.new_child(UiBuilder::new().max_rect(r.shrink2(vec2(10.0, 3.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
        m.spacing_mut().button_padding = vec2(10.0, 4.0);
        m.visuals_mut().widgets.inactive.bg_stroke = Stroke::NONE;
        m.menu_button("Fichier", |ui| {
            if ui.add(egui::Button::new("Ouvrir...").shortcut_text("Ctrl+O")).clicked() {
                actions.push(Action::OpenDialog);
                ui.close();
            }
            ui.menu_button("Fichiers récents", |ui| {
                if self.prefs.recents.is_empty() {
                    ui.label(RichText::new("aucun").color(pal.weak));
                }
                for f in &self.prefs.recents {
                    if ui.button(f.display().to_string()).clicked() {
                        actions.push(Action::Open(f.clone()));
                        ui.close();
                    }
                }
            });
            ui.separator();
            if ui.button("Préférences...").clicked() {
                actions.push(Action::Prefs);
                ui.close();
            }
            ui.separator();
            if ui.button("Quitter").clicked() {
                actions.push(Action::Quit);
                ui.close();
            }
        });
        m.menu_button("Atelier", |ui| {
            ui.menu_button("CNC", |ui| {
                for at in Atelier::CNC {
                    let label = if at.available() { at.label().to_owned() } else { format!("{} (à venir)", at.label()) };
                    if ui.radio(self.atelier == at, label).clicked() {
                        actions.push(Action::Atelier(at));
                        ui.close();
                    }
                }
            });
            for at in [Atelier::Laser, Atelier::Pcb, Atelier::Machines] {
                let label = if at.available() { at.label().to_owned() } else { format!("{} (à venir)", at.label()) };
                if ui.radio(self.atelier == at, label).clicked() {
                    actions.push(Action::Atelier(at));
                    ui.close();
                }
            }
        });
        m.menu_button("Extensions", |ui| {
            let mut groups: BTreeMap<&str, Vec<(&str, &CommandSpec)>> = BTreeMap::new();
            for (path, cmds) in &self.menus {
                let (top, rest) = path.split_once('/').unwrap_or((path.as_str(), ""));
                for c in cmds {
                    groups.entry(top).or_default().push((rest, c));
                }
            }
            for (top, cmds) in groups {
                ui.menu_button(top, |ui| {
                    for (sub, c) in cmds {
                        let label = if sub.is_empty() { c.label.to_owned() } else { format!("{sub} : {}", c.label) };
                        if ui.button(label).on_hover_text(c.id).clicked() {
                            actions.push(Action::Command(c.clone()));
                            ui.close();
                        }
                    }
                });
            }
        });
        m.menu_button("Aide", |ui| {
            if ui.button("À propos de MakerCraft").clicked() {
                actions.push(Action::About);
                ui.close();
            }
        });
    }

    fn atelier_bar(&self, ui: &mut Ui, r: Rect, pal: &Palette, actions: &mut Vec<Action>) {
        let p = ui.painter();
        p.rect_filled(r, CornerRadius::ZERO, pal.bar);
        p.hline(r.x_range(), r.top() + 0.5, Stroke::new(1.0, pal.bar_line));
        p.hline(r.x_range(), r.bottom() - 0.5, Stroke::new(1.0, pal.bar_line));
        let logo = Rect::from_min_size(pos2(r.left() + 20.0, r.center().y - 17.0), vec2(34.0, 34.0));
        if let Some(t) = &self.logo {
            p.image(t.id(), logo, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        let title = p.text(pos2(logo.right() + 12.0, r.center().y), Align2::LEFT_CENTER, "MakerCraft", theme::bold(19.0), pal.text);
        let mut tabs = ui.new_child(UiBuilder::new().max_rect(Rect::from_min_max(pos2(title.right() + 36.0, r.top() + 10.0), pos2(r.right() - 20.0, r.bottom() - 10.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
        tabs.spacing_mut().item_spacing.x = 6.0;
        for f in Family::ALL {
            let active = self.atelier.family() == f;
            let soon = !f.default_atelier().available();
            let resp = widgets::tab(&mut tabs, pal, f.icon(), f.label(), active, soon);
            if resp.clicked() && !active {
                actions.push(Action::Atelier(f.default_atelier()));
            }
            if soon {
                resp.on_hover_text("Atelier à venir");
            }
        }
        if self.atelier.family() == Family::Cnc {
            tabs.add_space(24.0);
            let (sep, _) = tabs.allocate_exact_size(vec2(1.0, 28.0), egui::Sense::hover());
            tabs.painter().rect_filled(sep, CornerRadius::ZERO, pal.bar_line);
            tabs.add_space(16.0);
            for at in Atelier::CNC {
                if widgets::segment(&mut tabs, pal, at.label(), self.atelier == at, !at.available()).clicked() {
                    actions.push(Action::Atelier(at));
                }
            }
        }
    }

    fn status_bar(&self, ui: &mut Ui, r: Rect, pal: &Palette) {
        let p = ui.painter();
        p.rect_filled(r, CornerRadius::ZERO, pal.bar);
        p.hline(r.x_range(), r.top() + 0.5, Stroke::new(1.0, pal.bar_line));
        let f = theme::regular(12.5);
        icons::paint(p, Rect::from_min_size(pos2(r.left() + 16.0, r.center().y - 8.0), vec2(16.0, 16.0)), self.atelier.family().icon(), pal.blue);
        let a = p.text(pos2(r.left() + 40.0, r.center().y), Align2::LEFT_CENTER, self.atelier.title(), theme::bold(12.5), pal.text);
        let mut x = a.right() + 18.0;
        if let Some(n) = self.cnc.file_name() {
            let t = p.text(pos2(x, r.center().y), Align2::LEFT_CENTER, n, theme::mono(12.0), pal.weak);
            x = t.right() + 18.0;
        }
        // à droite : unités, machine
        let mc = if self.machines.connected() { pal.orange } else { pal.weak };
        let right = p.text(pos2(r.right() - 16.0, r.center().y), Align2::RIGHT_CENTER, format!("Unités : {}", self.prefs.units.label()), f.clone(), pal.weak);
        let m = p.text(pos2(right.left() - 22.0, r.center().y), Align2::RIGHT_CENTER, format!("Machine : {}", self.machines.state_label()), f.clone(), mc);
        p.circle_filled(pos2(m.left() - 10.0, r.center().y), 4.0, mc);
        let max = m.left() - 30.0 - x;
        if max > 40.0 {
            let mut s = self.status.clone();
            while s.chars().count() > 4 && p.layout_no_wrap(s.clone(), f.clone(), pal.text).size().x > max {
                let n = s.chars().count();
                s = s.chars().take(n - 4).collect::<String>() + "...";
                s = s.replacen("......", "...", 1);
            }
            p.text(pos2(x, r.center().y), Align2::LEFT_CENTER, s, f, pal.text);
        }
    }

    fn body(&mut self, ui: &mut Ui, pal: &Palette) {
        let units = self.prefs.units;
        match self.atelier {
            Atelier::CncDao | Atelier::CncFao => {
                let mut req = Vec::new();
                self.cnc.ui(ui, pal, units, self.atelier, &mut req);
                for r in req {
                    match r {
                        cnc::Request::OpenDialog => self.open_dialog(),
                        cnc::Request::RecentDir(d) => {
                            self.prefs.add_recent_dir(&d);
                            self.save_prefs();
                        }
                        cnc::Request::Status(s) => self.status = s,
                    }
                }
            }
            Atelier::Machines => {
                let mut req = Vec::new();
                self.machines.ui(ui, pal, units, &mut req);
                for r in req {
                    match r {
                        machines::Request::Status(s) => self.status = s,
                        machines::Request::Port(port, baud) => {
                            self.prefs.port = port;
                            self.prefs.baud = baud;
                            self.save_prefs();
                        }
                    }
                }
            }
            at => ateliers::soon_screen(ui, pal, at),
        }
    }

    fn about_window(&mut self, ctx: &egui::Context, pal: &Palette) {
        let mut open = true;
        egui::Window::new("À propos de MakerCraft").collapsible(false).resizable(false).fixed_size(vec2(560.0, 620.0)).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).open(&mut open).show(ctx, |ui| {
            if let Some(t) = &self.about_img {
                let w = ui.available_width();
                let s = t.size_vec2();
                let h = (w * s.y / s.x.max(1.0)).min(240.0);
                let w = h * s.x / s.y.max(1.0);
                let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), egui::Sense::hover());
                let r = Rect::from_center_size(r.center(), vec2(w, h));
                ui.painter().image(t.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            ui.add_space(6.0);
            ui.label(RichText::new(format!("MakerCraft {}", env!("CARGO_PKG_VERSION"))).font(theme::bold(20.0)));
            ui.label("Moteur 2D/3D à extensions pour le fablab et l'enseignement de la Technologie : DAO, CAO, FAO et pilotage de machines.");
            ui.label(RichText::new("Licence GPL-3.0-or-later. Projet indépendant, non affilié à ArtCraft ni aux auteurs de SolidCraft.").size(12.5).color(pal.weak));
            ui.add_space(4.0);
            widgets::caption(ui, pal, "Extensions chargées");
            for (id, label) in self.host.extensions() {
                ui.horizontal(|ui| {
                    widgets::pill(ui, id, pal.blue_soft, pal.blue);
                    ui.label(*label);
                });
            }
            if let Some(e) = &self.host_error {
                widgets::status_line(ui, pal, Icon::Error, pal.bad, e);
            }
            ui.add_space(4.0);
            widgets::caption(ui, pal, "Bâti sur");
            ui.label(RichText::new("CadCraft 0.20.1 et SolidCraft 0.41.0 (MIT/Apache, utilisés tels quels) ; portages de LaserGRBL 7.14.1, Candle et Universal G-Code Sender 2.1.27 (GPLv3).").size(12.5));
        });
        if !open {
            self.show_about = false;
        }
    }

    fn prefs_window(&mut self, ctx: &egui::Context, pal: &Palette) {
        let mut open = true;
        let before = self.prefs.clone();
        let mut clear = false;
        egui::Window::new("Préférences").collapsible(false).resizable(false).fixed_size(vec2(440.0, 340.0)).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).open(&mut open).show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 10.0;
            widgets::caption(ui, pal, "Unités d'affichage");
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.prefs.units, Units::Mm, "Millimètres");
                ui.radio_value(&mut self.prefs.units, Units::Inch, "Pouces");
            });
            widgets::caption(ui, pal, "Thème");
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.prefs.theme, ThemeChoice::Light, "Clair");
                ui.radio_value(&mut self.prefs.theme, ThemeChoice::Dark, "Sombre");
            });
            widgets::caption(ui, pal, "Dossiers récents");
            egui::ScrollArea::vertical().max_height(110.0).show(ui, |ui| {
                if self.prefs.recent_dirs.is_empty() {
                    ui.label(RichText::new("aucun").color(pal.weak));
                }
                for d in &self.prefs.recent_dirs {
                    ui.label(RichText::new(d.display().to_string()).font(theme::mono(12.5)));
                }
            });
            clear = widgets::button(ui, pal, None, "Vider les listes de récents", widgets::Kind::Plain, !self.prefs.recent_dirs.is_empty() || !self.prefs.recents.is_empty()).clicked();
        });
        if clear {
            self.prefs.recents.clear();
            self.prefs.recent_dirs.clear();
        }
        if self.prefs != before {
            self.save_prefs();
        }
        if !open {
            self.show_prefs = false;
        }
    }
}
