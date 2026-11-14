//! Fenêtre principale : barre du haut (sélecteur d'ateliers à gauche, menus, état de la
//! machine, options), contenu de l'atelier actif, barre d'état ; fenêtres « Options » (qui
//! contient l'À propos) et exécution d'une commande d'extension.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use eframe::egui::{
    self, Align2, ColorImage, CornerRadius, Key, KeyboardShortcut, Modifiers, Rect, RichText, Sense, Stroke, TextureHandle, TextureOptions, Ui, UiBuilder, pos2,
    vec2,
};
use makercraft_ext_api::{CommandSpec, Host};

use crate::ateliers::{self, Atelier, Family, cnc, machines};
use crate::hote::{self, CommandDialog};
use crate::icons::{self, Icon};
use crate::options::{self, Options, Page};
use crate::prefs::{Prefs, ThemeChoice};
use crate::switcher::Switcher;
use crate::theme::{self, Palette};

pub const ICON_PNG: &[u8] = include_bytes!("../../../assets/branding/makercraft-256.png");
const ABOUT_JPG: &[u8] = include_bytes!("../../../assets/branding/makercraft-about.jpg");

const TOP_H: f32 = 58.0;
const STATUS_H: f32 = 30.0;
/// Largeur réservée au sélecteur d'ateliers.
const SWITCH_W: f32 = 300.0;

pub struct App {
    prefs: Prefs,
    applied_theme: Option<ThemeChoice>,
    host: Host,
    host_error: Option<String>,
    menus: BTreeMap<String, Vec<CommandSpec>>,
    atelier: Atelier,
    switcher: Switcher,
    cnc: cnc::Cnc,
    machines: machines::Machines,
    options: Options,
    command: Option<CommandDialog>,
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
    Options(Page),
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
        let atelier = if prefs.remember_atelier { Atelier::from_id(&prefs.last_atelier).filter(|a| a.available()) } else { None };
        let mut app = Self {
            prefs,
            applied_theme: None,
            host,
            host_error,
            menus,
            atelier: atelier.unwrap_or(Atelier::CncDao),
            switcher: Switcher::default(),
            cnc: cnc::Cnc::default(),
            machines,
            options: Options::default(),
            command: None,
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

    fn set_atelier(&mut self, at: Atelier) {
        self.atelier = at;
        if at == Atelier::Machines && self.machines.port.is_empty() {
            self.machines.refresh_ports();
        }
        if self.prefs.last_atelier != at.id() {
            self.prefs.last_atelier = at.id().to_owned();
            self.save_prefs();
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
            Action::Atelier(at) => self.set_atelier(at),
            Action::Options(page) => self.options.show_page(&self.prefs, page),
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
            let top = Rect::from_min_size(full.min, vec2(full.width(), TOP_H));
            let status = Rect::from_min_max(pos2(full.left(), full.bottom() - STATUS_H), full.max);
            let body = Rect::from_min_max(pos2(full.left() + 18.0, top.bottom() + 16.0), pos2(full.right() - 18.0, status.top() - 14.0));
            self.top_bar(ui, top, pal, &mut actions);
            self.status_bar(ui, status, pal);
            let mut b = ui.new_child(UiBuilder::new().max_rect(body));
            self.body(&mut b, pal);
        });

        if self.options.open {
            match self.options.ui(&ctx, pal, &self.host, self.about_img.as_ref()) {
                options::Outcome::Apply(p) => {
                    self.prefs = p;
                    self.save_prefs();
                    self.status = "Options enregistrées".to_owned();
                }
                options::Outcome::Run(spec) => actions.push(Action::Command(spec)),
                options::Outcome::None => {}
            }
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
    fn top_bar(&mut self, ui: &mut Ui, r: Rect, pal: &Palette, actions: &mut Vec<Action>) {
        let p = ui.painter().clone();
        p.rect_filled(r, CornerRadius::ZERO, pal.bar);
        p.hline(r.x_range(), r.bottom() - 0.5, Stroke::new(1.0, pal.bar_line));
        // sélecteur d'ateliers, tout à gauche
        let sw = Rect::from_min_size(pos2(r.left() + 10.0, r.top() + 6.0), vec2(SWITCH_W - 16.0, r.height() - 12.0));
        if let Some(at) = self.switcher.ui(ui, sw, pal, self.atelier) {
            actions.push(Action::Atelier(at));
        }
        p.vline(r.left() + SWITCH_W + 2.0, (r.top() + 14.0)..=(r.bottom() - 14.0), Stroke::new(1.0, pal.bar_line));
        // menus
        let mr = Rect::from_min_max(pos2(r.left() + SWITCH_W + 14.0, r.top() + 12.0), pos2(r.right() - 330.0, r.bottom() - 12.0));
        let mut m = ui.new_child(UiBuilder::new().max_rect(mr).layout(egui::Layout::left_to_right(egui::Align::Center)));
        self.menus(&mut m, pal, actions);
        // à droite : état de la machine, options
        let gear = Rect::from_min_size(pos2(r.right() - 16.0 - 108.0, r.center().y - 17.0), vec2(108.0, 34.0));
        let g = ui.interact(gear, ui.scope_id().with("options"), Sense::click());
        g.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Options"));
        p.rect_filled(gear, CornerRadius::same(8), if g.hovered() { pal.hover } else { pal.bar });
        p.rect_stroke(gear, CornerRadius::same(8), Stroke::new(1.0, pal.line2), egui::StrokeKind::Inside);
        icons::paint(&p, Rect::from_min_size(pos2(gear.left() + 12.0, gear.center().y - 8.0), vec2(16.0, 16.0)), Icon::Machines, pal.weak);
        p.text(pos2(gear.left() + 36.0, gear.center().y), Align2::LEFT_CENTER, "Options", theme::regular(14.0), pal.text);
        if g.clicked() {
            actions.push(Action::Options(Page::General));
        }
        let connected = self.machines.connected();
        let mc = if connected { pal.orange } else { pal.dim };
        let label = format!("Machine : {}", self.machines.state_label());
        let font = theme::regular(13.0);
        let tw = p.layout_no_wrap(label.clone(), font.clone(), pal.text).size().x;
        let pill = Rect::from_min_size(pos2(gear.left() - 14.0 - tw - 34.0, r.center().y - 15.0), vec2(tw + 34.0, 30.0));
        let pr = ui.interact(pill, ui.scope_id().with("etat-machine"), Sense::click());
        pr.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
        p.rect_filled(pill, CornerRadius::same(255), if pr.hovered() { pal.hover } else { pal.card });
        p.rect_stroke(pill, CornerRadius::same(255), Stroke::new(1.0, pal.card_line), egui::StrokeKind::Inside);
        p.circle_filled(pos2(pill.left() + 15.0, pill.center().y), 4.0, mc);
        p.text(pos2(pill.left() + 26.0, pill.center().y), Align2::LEFT_CENTER, label, font, if connected { pal.text } else { pal.weak });
        if pr.clicked() {
            actions.push(Action::Atelier(Atelier::Machines));
        }
    }

    fn menus(&self, m: &mut Ui, pal: &Palette, actions: &mut Vec<Action>) {
        m.spacing_mut().button_padding = vec2(10.0, 5.0);
        m.spacing_mut().item_spacing.x = 4.0;
        m.visuals_mut().widgets.inactive.bg_stroke = Stroke::NONE;
        m.visuals_mut().widgets.inactive.weak_bg_fill = pal.bar;
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
            if ui.button("Options...").clicked() {
                actions.push(Action::Options(Page::General));
                ui.close();
            }
            ui.separator();
            if ui.button("Quitter").clicked() {
                actions.push(Action::Quit);
                ui.close();
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
                actions.push(Action::Options(Page::About));
                ui.close();
            }
        });
    }

    fn status_bar(&self, ui: &mut Ui, r: Rect, pal: &Palette) {
        let p = ui.painter();
        p.rect_filled(r, CornerRadius::ZERO, pal.bar);
        p.hline(r.x_range(), r.top() + 0.5, Stroke::new(1.0, pal.bar_line));
        let f = theme::regular(12.5);
        let accent = self.atelier.family().accent(pal);
        p.circle_filled(pos2(r.left() + 18.0, r.center().y), 4.0, accent);
        let a = p.text(pos2(r.left() + 30.0, r.center().y), Align2::LEFT_CENTER, self.atelier.title(), theme::bold(12.5), pal.text);
        let mut x = a.right() + 18.0;
        if let Some(n) = self.cnc.file_name() {
            let t = p.text(pos2(x, r.center().y), Align2::LEFT_CENTER, n, theme::mono(12.0), pal.weak);
            x = t.right() + 18.0;
        }
        let right = p.text(pos2(r.right() - 16.0, r.center().y), Align2::RIGHT_CENTER, format!("Unités : {}", self.prefs.units.label()), f.clone(), pal.weak);
        let ver = p.text(pos2(right.left() - 22.0, r.center().y), Align2::RIGHT_CENTER, concat!("MakerCraft ", env!("CARGO_PKG_VERSION")), f.clone(), pal.dim);
        let max = ver.left() - 30.0 - x;
        if max > 40.0 {
            let mut job = egui::text::LayoutJob::simple_singleline(self.status.clone(), f, pal.text);
            job.wrap.max_width = max;
            job.wrap.max_rows = 1;
            let g = p.layout_job(job);
            p.galley(pos2(x, r.center().y - g.size().y / 2.0), g, pal.text);
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
}
