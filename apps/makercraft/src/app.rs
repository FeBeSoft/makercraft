//! Fenêtre principale : barre du haut (sélecteur d'ateliers à gauche, menus, état de la
//! machine, options), contenu de l'atelier actif, barre d'état ; fenêtres « Options » (qui
//! contient l'À propos) et exécution d'une commande d'extension.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use eframe::egui::{
    self, Align2, Color32, ColorImage, CornerRadius, Key, KeyboardShortcut, Modifiers, Rect, RichText, Sense, Stroke, TextureHandle, TextureOptions, Ui, UiBuilder, pos2,
    vec2,
};
use makercraft_ext_api::{CommandSpec, Host};

use crate::ateliers::{self, Atelier, Family, cnc, machines};
use crate::hote::{self, CommandDialog};
use crate::icons::{self, Icon};
use crate::options::{self, Options, Page};
use crate::prefs::{Prefs, ThemeChoice};
use crate::ribbon::{self, Ribbon, Tab};
use crate::switcher::Switcher;
use crate::theme::{self, Palette};

pub const ICON_PNG: &[u8] = include_bytes!("../../../assets/branding/makercraft-256.png");

/// Barre d'accès rapide (logo, menus, document, état de la machine, options).
const QUICK_H: f32 = 40.0;
const STATUS_H: f32 = 30.0;
/// Largeur du sélecteur d'ateliers, à gauche du ruban.
const SWITCH_W: f32 = 104.0;

pub struct App {
    prefs: Prefs,
    applied_theme: Option<ThemeChoice>,
    host: Host,
    host_error: Option<String>,
    menus: BTreeMap<String, Vec<CommandSpec>>,
    atelier: Atelier,
    switcher: Switcher,
    ribbon: Ribbon,
    logo: Option<TextureHandle>,
    cnc: cnc::Cnc,
    machines: machines::Machines,
    options: Options,
    /// Fenêtre « Mettre à jour ».
    updater: crate::update::Updater,
    /// Projet modifié : action en attente de la réponse « Enregistrer ? ».
    confirm: Option<Pending>,
    /// Fermeture accordée (après enregistrement ou « Ne pas enregistrer »).
    closing_ok: bool,
    /// Titre de la fenêtre Windows déjà envoyé.
    title: String,
    command: Option<CommandDialog>,
    textures_tried: bool,
    status: String,
    /// Images déjà affichées (les premières sont redemandées : la fenêtre apparaît après elles).
    frames: u32,
}

/// Ce qui attend la réponse à « Enregistrer les modifications ? ».
enum Pending {
    Quit,
    Open(PathBuf),
    OpenDialog,
}

enum Action {
    OpenDialog,
    Open(PathBuf),
    Atelier(Atelier),
    Options(Page),
    Command(CommandSpec),
    /// Commande du ruban de l'atelier actif.
    Ribbon(&'static str),
    /// Choix dans une liste déroulante du ruban : identifiant et rang choisi.
    RibbonChoice(&'static str, usize),
    Quit,
}

impl App {
    pub fn new(files: Vec<PathBuf>) -> Self {
        let prefs = Prefs::load();
        let (host, host_error) = hote::load();
        let menus = hote::menu_tree(&host);
        let mut machines = machines::Machines::from_prefs(&prefs);
        machines.refresh_ports();
        let atelier = if prefs.remember_atelier { Atelier::from_id(&prefs.last_atelier).filter(|a| a.available()) } else { None };
        let mut app = Self {
            prefs,
            applied_theme: None,
            host,
            host_error,
            menus,
            atelier: atelier.unwrap_or(Atelier::Cnc),
            switcher: Switcher::default(),
            ribbon: Ribbon::default(),
            logo: None,
            cnc: cnc::Cnc::default(),
            machines,
            options: Options::default(),
            updater: crate::update::Updater::default(),
            confirm: None,
            closing_ok: false,
            title: String::new(),
            command: None,
            textures_tried: false,
            status: "Prêt".to_owned(),
            frames: 0,
        };
        if app.prefs.check_updates {
            app.updater.check_quietly();
        }
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
        if at != self.atelier {
            self.ribbon.tab = 0;
        }
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
            self.atelier = Atelier::Cnc;
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
        if let Ok(img) = image::load_from_memory(ICON_PNG) {
            let rgba = img.resize(128, 128, image::imageops::FilterType::Lanczos3).to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            self.logo = Some(ctx.load_texture("logo", ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()), TextureOptions::LINEAR));
        }
    }

    /// Le projet CNC a des modifications non enregistrées que `p` ferait perdre.
    fn would_lose(&self, p: &Pending) -> bool {
        let cnc_file = |path: &Path| {
            let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            !matches!(ext.as_str(), "nc" | "gcode" | "ngc" | "tap")
        };
        self.cnc.dirty()
            && match p {
                Pending::Quit => true,
                Pending::Open(path) => cnc_file(path),
                Pending::OpenDialog => self.atelier.family() == Family::Cnc,
            }
    }

    /// Fait l'action, sans plus rien demander.
    fn proceed(&mut self, p: Pending, ctx: &egui::Context) {
        match p {
            Pending::Quit => {
                self.closing_ok = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Pending::Open(path) => self.open(&path),
            Pending::OpenDialog => self.open_dialog(),
        }
    }

    /// Demande d'abord s'il faut enregistrer, si l'action ferait perdre des modifications.
    fn guarded(&mut self, p: Pending, ctx: &egui::Context) {
        if self.would_lose(&p) {
            self.confirm = Some(p);
        } else {
            self.proceed(p, ctx);
        }
    }

    /// Fenêtre « Enregistrer les modifications ? ».
    fn confirm_window(&mut self, ctx: &egui::Context, pal: &Palette) {
        if self.confirm.is_none() {
            return;
        }
        let name = self.cnc.path.as_ref().and_then(|p| p.file_name()).map_or_else(|| "Sans titre.mkc".to_owned(), |n| n.to_string_lossy().into_owned());
        let mut choice = None;
        egui::Modal::new(egui::Id::unique("enregistrer-avant")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.label(RichText::new("Enregistrer les modifications ?").font(theme::bold(15.0)));
            ui.add_space(4.0);
            ui.label(RichText::new(format!("Le projet « {name} » a été modifié depuis le dernier enregistrement.")).color(pal.weak));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button("Enregistrer").clicked() {
                    choice = Some(0);
                }
                if ui.button("Ne pas enregistrer").clicked() {
                    choice = Some(1);
                }
                if ui.button("Annuler").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    choice = Some(2);
                }
            });
        });
        match choice {
            Some(0) => {
                self.ribbon_command("cnc.enregistrer");
                if let Some(p) = self.confirm.take()
                    && !self.cnc.dirty()
                {
                    self.proceed(p, ctx);
                }
            }
            Some(1) => {
                if let Some(p) = self.confirm.take() {
                    self.proceed(p, ctx);
                }
            }
            Some(_) => self.confirm = None,
            None => {}
        }
    }

    fn run(&mut self, a: Action, ctx: &egui::Context) {
        match a {
            Action::OpenDialog => self.guarded(Pending::OpenDialog, ctx),
            Action::Open(p) => self.guarded(Pending::Open(p), ctx),
            Action::Atelier(at) => self.set_atelier(at),
            Action::Options(page) => self.options.show_page(&self.prefs, page),
            Action::Command(spec) => self.command = Some(CommandDialog::new(&spec)),
            Action::Ribbon(id) => self.ribbon_command(id),
            Action::RibbonChoice(id, i) => {
                if id.starts_with("machine.") {
                    let mut req = Vec::new();
                    self.machines.choose(id, i, &self.prefs, &mut req);
                    self.machines_requests(req);
                }
            }
            Action::Quit => self.guarded(Pending::Quit, ctx),
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
        if self.atelier == Atelier::Cnc && ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::S))) {
            actions.push(Action::Ribbon("cnc.enregistrer"));
        }
        // Ctrl+Z / Ctrl+Y (ou Ctrl+Maj+Z) : annuler, rétablir, hors saisie de texte
        if self.atelier == Atelier::Cnc && !ctx.egui_wants_keyboard_input() {
            if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z)) || i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Y))) {
                actions.push(Action::Ribbon("cnc.retablir"));
            }
            if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Z))) {
                actions.push(Action::Ribbon("cnc.annuler"));
            }
        }

        egui::CentralPanel::default().frame(egui::Frame::new().fill(pal.window)).show(ui, |ui| {
            let full = ui.max_rect();
            let quick = Rect::from_min_size(full.min, vec2(full.width(), QUICK_H));
            let rib_h = if self.ribbon.collapsed { ribbon::TAB_ROW_H } else { ribbon::TAB_ROW_H + ribbon::BODY_H };
            let rib = Rect::from_min_size(quick.left_bottom(), vec2(full.width(), rib_h));
            let status = Rect::from_min_max(pos2(full.left(), full.bottom() - STATUS_H), full.max);
            let body = Rect::from_min_max(pos2(full.left() + 18.0, rib.bottom() + 16.0), pos2(full.right() - 18.0, status.top() - 14.0));
            self.quick_bar(ui, quick, pal, &mut actions);
            self.ribbon_area(ui, rib, pal, &mut actions);
            if self.status_bar(ui, status, pal) {
                actions.push(Action::Atelier(Atelier::Machines));
            }
            let mut b = ui.new_child(UiBuilder::new().max_rect(body));
            self.body(&mut b, pal);
        });

        // fermeture de la fenêtre (croix, Alt+F4) avec un projet modifié : demander d'abord
        if ctx.input(|i| i.viewport().close_requested()) && !self.closing_ok && self.would_lose(&Pending::Quit) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm = Some(Pending::Quit);
        }
        self.confirm_window(&ctx, pal);
        // barre de titre de Windows (et barre des tâches) : fichier, puis logiciel et version
        let title = match self.cnc.file_name().filter(|_| self.atelier.family() == Family::Cnc) {
            Some(f) => format!("{f} - MakerCraft {}", crate::about::version_label()),
            None => format!("MakerCraft {}", crate::about::version_label()),
        };
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
        if self.updater.ui(&ctx, pal, &mut self.prefs) {
            self.save_prefs();
        }
        if self.options.open {
            match self.options.ui(&ctx, pal, &self.host, self.logo.as_ref()) {
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
    /// Barre d'accès rapide : logo et nom, menu Fichier, Ouvrir / Enregistrer / Annuler /
    /// Rétablir, onglet du document ; à droite les menus Extensions et Aide, l'état de la
    /// machine et les options.
    fn quick_bar(&mut self, ui: &mut Ui, r: Rect, pal: &Palette, actions: &mut Vec<Action>) {
        let p = ui.painter().clone();
        p.rect_filled(r, CornerRadius::ZERO, pal.bar);
        p.hline(r.x_range(), r.bottom() - 0.5, Stroke::new(1.0, pal.bar_line));
        // nom, version et fichier sont dans la barre de titre de Windows : ici, les menus seulement
        // à gauche
        let left = Rect::from_min_max(pos2(r.left() + 10.0, r.top() + 5.0), pos2(r.center().x + 120.0, r.bottom() - 5.0));
        let mut m = ui.new_child(UiBuilder::new().max_rect(left).layout(egui::Layout::left_to_right(egui::Align::Center)));
        menu_style(&mut m, pal);
        self.file_menu(&mut m, pal, actions);
        m.add_space(6.0);
        if quick_button(&mut m, pal, Icon::Open, "Ouvrir (Ctrl+O)", true) {
            actions.push(Action::OpenDialog);
        }
        let cnc = self.atelier == Atelier::Cnc;
        for (icon, tip, id) in [(Icon::Save, "Enregistrer (Ctrl+S)", "cnc.enregistrer"), (Icon::Undo, "Annuler", "cnc.annuler"), (Icon::Redo, "Rétablir", "cnc.retablir")] {
            if quick_button(&mut m, pal, icon, if cnc { tip } else { "Dans l'atelier CNC" }, cnc) {
                actions.push(Action::Ribbon(id));
            }
        }
        if self.atelier.family() == Family::Cnc
            && let Some(n) = self.cnc.file_name()
        {
            m.add_space(12.0);
            doc_tab(&mut m, pal, &n, self.atelier.family().accent(pal));
        }
        // à droite (de droite à gauche) : options, état de la machine, menus
        let right = Rect::from_min_max(pos2(r.center().x + 130.0, r.top() + 5.0), pos2(r.right() - 12.0, r.bottom() - 5.0));
        let mut rr = ui.new_child(UiBuilder::new().max_rect(right).layout(egui::Layout::right_to_left(egui::Align::Center)));
        menu_style(&mut rr, pal);
        // tout à droite : Aide, puis Options à sa gauche
        self.help_menu(&mut rr, actions);
        rr.add_space(6.0);
        let (gear, g) = rr.allocate_exact_size(vec2(100.0, 30.0), Sense::click());
        g.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Options"));
        p.rect_filled(gear, CornerRadius::same(8), if g.hovered() { pal.hover } else { pal.bar });
        p.rect_stroke(gear, CornerRadius::same(8), Stroke::new(1.0, pal.line2), egui::StrokeKind::Inside);
        icons::paint(&p, Rect::from_min_size(pos2(gear.left() + 11.0, gear.center().y - 8.0), vec2(16.0, 16.0)), Icon::Machines, pal.weak);
        p.text(pos2(gear.left() + 34.0, gear.center().y), Align2::LEFT_CENTER, "Options", theme::regular(13.5), pal.text);
        if g.clicked() {
            actions.push(Action::Options(Page::General));
        }
        // à gauche d'Options : vérification des mises à jour, à la demande
        rr.add_space(8.0);
        let newer = self.updater.newer(&self.prefs).map(|r| if r.name.is_empty() { r.tag.clone() } else { r.name.clone() });
        let ulabel = "Mettre à jour";
        let uw = p.layout_no_wrap(ulabel.to_owned(), theme::regular(13.0), pal.text).size().x;
        let (ub, ur) = rr.allocate_exact_size(vec2(uw + 38.0, 28.0), Sense::click());
        ur.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, ulabel));
        p.rect_filled(ub, CornerRadius::same(8), if ur.hovered() { pal.hover } else { pal.bar });
        p.rect_stroke(ub, CornerRadius::same(8), Stroke::new(1.0, if newer.is_some() { pal.orange } else { pal.line2 }), egui::StrokeKind::Inside);
        icons::paint(&p, Rect::from_min_size(pos2(ub.left() + 10.0, ub.center().y - 8.0), vec2(16.0, 16.0)), Icon::Down, if newer.is_some() { pal.orange } else { pal.weak });
        p.text(pos2(ub.left() + 30.0, ub.center().y), Align2::LEFT_CENTER, ulabel, theme::regular(13.0), pal.text);
        let tip = match &newer {
            Some(n) => format!("Nouvelle version disponible : {n}. Cliquer pour voir les détails."),
            None => "Vérifier sur GitHub s'il existe une nouvelle version (connexion seulement à la demande)".to_owned(),
        };
        if ur.on_hover_text(tip).clicked() {
            self.updater.start();
        }
        if self.prefs.extensions_menu {
            rr.add_space(10.0);
            self.extensions_menu(&mut rr, actions);
        }
    }

    /// Ruban : grand sélecteur d'ateliers à gauche, onglets et groupes de l'atelier actif.
    fn ribbon_area(&mut self, ui: &mut Ui, r: Rect, pal: &Palette, actions: &mut Vec<Action>) {
        let p = ui.painter().clone();
        p.rect_filled(r, CornerRadius::ZERO, pal.bar);
        p.hline(r.x_range(), r.bottom() - 0.5, Stroke::new(1.0, pal.bar_line));
        let sw = Rect::from_min_size(pos2(r.left() + 6.0, r.top() + 4.0), vec2(SWITCH_W - 12.0, r.height() - 8.0));
        if let Some(at) = self.switcher.ui(ui, sw, pal, self.atelier) {
            actions.push(Action::Atelier(at));
        }
        p.vline(r.left() + SWITCH_W, (r.top() + 6.0)..=(r.bottom() - 6.0), Stroke::new(1.0, pal.bar_line));
        let tabs = self.ribbon_tabs();
        let collapsed = self.ribbon.collapsed;
        // atelier CNC : les onglets du ruban sont les étapes, de la machine à l'usinage
        if self.atelier == Atelier::Cnc {
            self.ribbon.tab = self.cnc.step.index();
        }
        let tr = Rect::from_min_max(pos2(r.left() + SWITCH_W + 6.0, r.top()), pos2(r.right() - 6.0, r.top() + ribbon::TAB_ROW_H));
        // atelier CNC : pastille de l'état de la machine après « Machine » (vert : connectée,
        // rouge : déconnectée) ; un clic mène à l'atelier Machines
        let tip = format!("Machine : {} ; cliquer pour ouvrir l'atelier Machines", self.machines.state_label());
        let dot = (self.atelier == Atelier::Cnc).then(|| (cnc::Step::Machine.index(), if self.machines.connected() { pal.ok } else { pal.bad }, tip.as_str()));
        if self.ribbon.tabs_ui(ui, tr, pal, &tabs, self.atelier.family().accent(pal), dot) {
            actions.push(Action::Atelier(Atelier::Machines));
        }
        if self.atelier == Atelier::Cnc {
            self.cnc.step = cnc::Step::from_index(self.ribbon.tab);
        }
        if !collapsed {
            p.hline(tr.left()..=r.right(), tr.bottom() - 0.5, Stroke::new(1.0, pal.bar_line));
            let br = Rect::from_min_max(pos2(tr.left(), tr.bottom()), pos2(r.right() - 6.0, r.bottom()));
            if let Some(id) = self.ribbon.body_ui(ui, br, pal, &tabs) {
                actions.push(Action::Ribbon(id));
            }
            if let Some((id, i)) = self.ribbon.picked.take() {
                actions.push(Action::RibbonChoice(id, i));
            }
        }
    }

    fn ribbon_tabs(&self) -> Vec<Tab> {
        match self.atelier {
            Atelier::Cnc => self.cnc.ribbon(),
            Atelier::Machines => self.machines.ribbon(),
            _ => vec![Tab { label: "Accueil", groups: Vec::new(), empty_note: "Cet atelier arrive dans une prochaine version : son ruban est vide pour l'instant." }],
        }
    }

    fn ribbon_command(&mut self, id: &str) {
        if id.starts_with("cnc.") {
            let mut req = Vec::new();
            self.cnc.command(id, &mut req);
            self.cnc_requests(req);
        } else if id.starts_with("machine.") {
            let mut req = Vec::new();
            self.machines.command(id, &mut req);
            self.machines_requests(req);
        }
    }

    fn cnc_requests(&mut self, req: Vec<cnc::Request>) {
        for r in req {
            match r {
                cnc::Request::OpenDialog => self.open_dialog(),
                cnc::Request::Status(s) => self.status = s,
                cnc::Request::Recent(p) => {
                    self.prefs.add_recent(&p);
                    self.save_prefs();
                }
                cnc::Request::SendProgram(p) => match self.machines.load_program(&p) {
                    Ok(msg) => {
                        self.set_atelier(Atelier::Machines);
                        self.status = format!("Programme ouvert dans l'atelier Machines : {msg}");
                    }
                    Err(e) => self.status = format!("Programme illisible : {e}"),
                },
            }
        }
    }

    fn machines_requests(&mut self, req: Vec<machines::Request>) {
        for r in req {
            match r {
                machines::Request::Status(s) => self.status = s,
                machines::Request::Port(port, baud) => {
                    self.prefs.port = port;
                    self.prefs.baud = baud;
                    self.save_prefs();
                }
                machines::Request::Equipment(id, e) => {
                    self.prefs.equipment.insert(id, e);
                    self.save_prefs();
                }
                machines::Request::Options => self.options.show_page(&self.prefs, Page::General),
            }
        }
    }

    fn file_menu(&self, m: &mut Ui, pal: &Palette, actions: &mut Vec<Action>) {
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
    }

    fn extensions_menu(&self, m: &mut Ui, actions: &mut Vec<Action>) {
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
    }

    fn help_menu(&self, m: &mut Ui, actions: &mut Vec<Action>) {
        m.menu_button("Aide", |ui| {
            if ui.button("À propos de MakerCraft").clicked() {
                actions.push(Action::Options(Page::About));
                ui.close();
            }
        });
    }

    /// Barre d'état ; renvoie vrai si on a cliqué sur l'état de la machine.
    fn status_bar(&self, ui: &mut Ui, r: Rect, pal: &Palette) -> bool {
        let p = ui.painter();
        p.rect_filled(r, CornerRadius::ZERO, pal.bar);
        p.hline(r.x_range(), r.top() + 0.5, Stroke::new(1.0, pal.bar_line));
        let f = theme::regular(12.5);
        let accent = self.atelier.family().accent(pal);
        p.circle_filled(pos2(r.left() + 18.0, r.center().y), 4.0, accent);
        let a = p.text(pos2(r.left() + 30.0, r.center().y), Align2::LEFT_CENTER, self.atelier.title(), theme::bold(12.5), pal.text);
        let mut x = a.right() + 18.0;
        if self.atelier == Atelier::Machines {
            let t = p.text(pos2(x, r.center().y), Align2::LEFT_CENTER, self.machines.status_summary(), f.clone(), pal.weak);
            x = t.right() + 18.0;
        }
        let right = p.text(pos2(r.right() - 16.0, r.center().y), Align2::RIGHT_CENTER, format!("Unités : {}", self.prefs.units.label()), f.clone(), pal.weak);
        // état de la machine avant les unités (le fichier et la version sont dans le titre)
        let connected = self.machines.connected();
        let ver = p.text(pos2(right.left() - 22.0, r.center().y), Align2::RIGHT_CENTER, format!("Machine : {}", self.machines.state_label()), f.clone(), if connected { pal.text } else { pal.weak });
        p.circle_filled(pos2(ver.left() - 9.0, r.center().y), 4.0, if connected { pal.ok } else { pal.bad });
        let ver = ver.expand2(vec2(14.0, 0.0));
        let zone = Rect::from_min_max(pos2(ver.left() - 6.0, r.top()), pos2(ver.right() - 8.0, r.bottom()));
        let m = ui.interact(zone, ui.scope_id().with("etat-machine"), Sense::click()).on_hover_text("Ouvrir l'atelier Machines").on_hover_cursor(egui::CursorIcon::PointingHand);
        m.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "État de la machine"));
        let go = m.clicked();
        let max = ver.left() - 30.0 - x;
        if max > 40.0 {
            let mut job = egui::text::LayoutJob::simple_singleline(self.status.clone(), f, pal.text);
            job.wrap.max_width = max;
            job.wrap.max_rows = 1;
            let g = p.layout_job(job);
            p.galley(pos2(x, r.center().y - g.size().y / 2.0), g, pal.text);
        }
        go
    }

    fn body(&mut self, ui: &mut Ui, pal: &Palette) {
        let units = self.prefs.units;
        match self.atelier {
            Atelier::Cnc => {
                let mut req = Vec::new();
                self.cnc.apply_prefs(&self.prefs);
                self.cnc.ui(ui, pal, units, &mut req);
                self.cnc_requests(req);
            }
            Atelier::Machines => {
                let mut req = Vec::new();
                self.machines.ui(ui, pal, units, &self.prefs, &mut req);
                self.machines_requests(req);
            }
            at => ateliers::soon_screen(ui, pal, at),
        }
    }
}

/// Menus de la barre d'accès rapide : sans cadre, fond de la barre.
fn menu_style(m: &mut Ui, pal: &Palette) {
    m.spacing_mut().button_padding = vec2(10.0, 5.0);
    m.spacing_mut().item_spacing.x = 4.0;
    m.visuals_mut().widgets.inactive.bg_stroke = Stroke::NONE;
    m.visuals_mut().widgets.inactive.weak_bg_fill = pal.bar;
}

/// Petit bouton à icône de la barre d'accès rapide ; renvoie vrai s'il a été cliqué.
fn quick_button(ui: &mut Ui, pal: &Palette, icon: Icon, tip: &str, enabled: bool) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(30.0, 28.0), if enabled { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, tip));
    let p = ui.painter();
    if enabled && resp.hovered() {
        p.rect_filled(r, CornerRadius::same(6), pal.hover);
    }
    icons::paint(p, Rect::from_center_size(r.center(), vec2(16.0, 16.0)), icon, if enabled { pal.text } else { theme::alpha(pal.dim, 0.7) });
    let resp = resp.on_hover_text(tip);
    enabled && resp.clicked()
}

/// Onglet du document ouvert.
fn doc_tab(ui: &mut Ui, pal: &Palette, name: &str, accent: Color32) {
    let shown: String = if name.chars().count() > 36 { format!("{}...", name.chars().take(34).collect::<String>()) } else { name.to_owned() };
    let font = theme::regular(13.0);
    let tw = ui.painter().layout_no_wrap(shown.clone(), font.clone(), pal.text).size().x;
    let (r, resp) = ui.allocate_exact_size(vec2(tw + 40.0, 28.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, CornerRadius::same(7), pal.card);
    p.rect_stroke(r, CornerRadius::same(7), Stroke::new(1.0, pal.card_line), egui::StrokeKind::Inside);
    p.rect_filled(Rect::from_min_max(pos2(r.left() + 8.0, r.top()), pos2(r.right() - 8.0, r.top() + 2.0)), CornerRadius::same(1), accent);
    icons::paint(p, Rect::from_min_size(pos2(r.left() + 10.0, r.center().y - 7.0), vec2(14.0, 14.0)), Icon::File, accent);
    p.text(pos2(r.left() + 30.0, r.center().y), Align2::LEFT_CENTER, shown, font, pal.text);
    if name.chars().count() > 36 {
        resp.on_hover_text(name);
    }
}
