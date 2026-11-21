//! Onglet Pilotage : cartes d'axes, déplacement manuel | broche et arrêt logiciel | vue 3D / 2D /
//! caméra, rangée de commandes Grbl, programme et console.

use eframe::egui::{self, Align2, Color32, CornerRadius, Key, Rect, Response, RichText, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, Ui, UiBuilder, pos2, vec2};

use super::equip::Reference;
use super::vue::{self, Scene, View};
use super::{Machines, Request, Run};
use crate::icons::{self, Icon};
use crate::prefs::Units;
use crate::theme::{self, Palette};
use crate::widgets::{self, Kind};

/// Gros bouton à icône et libellé, avec une petite ligne d'explication éventuelle.
#[allow(clippy::too_many_arguments)]
fn big(ui: &mut Ui, pal: &Palette, size: egui::Vec2, icon: Option<Icon>, text: &str, sub: &str, kind: Kind, enabled: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, text));
    let (bg, fg, line) = match kind {
        Kind::Primary => (pal.blue, pal.on_blue, pal.blue),
        Kind::Component => (pal.orange, pal.on_orange, pal.orange),
        Kind::Danger => (pal.bad, Color32::WHITE, pal.bad),
        Kind::Plain => (pal.panel, pal.text, pal.card_line),
    };
    let hovered = enabled && resp.hovered();
    let bg = if !enabled { theme::alpha(bg, 0.35) } else if hovered && kind == Kind::Plain { pal.blue_soft } else if hovered { bg.gamma_multiply(0.9) } else { bg };
    let fg = if enabled { fg } else { theme::alpha(if kind == Kind::Plain { pal.weak } else { fg }, 0.6) };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(10), bg);
    p.rect_stroke(rect, CornerRadius::same(10), Stroke::new(1.0, if hovered && kind == Kind::Plain { pal.blue } else { line }), StrokeKind::Inside);
    let has_sub = !sub.is_empty();
    let mut cy = rect.center().y - if has_sub { 7.0 } else { 0.0 };
    if let Some(i) = icon {
        let s = 20.0;
        let iy = if text.is_empty() { cy - s / 2.0 } else { cy - s - 1.0 };
        icons::paint(p, Rect::from_min_size(pos2(rect.center().x - s / 2.0, iy), vec2(s, s)), i, fg);
        if !text.is_empty() {
            cy += 10.0;
        }
    }
    if !text.is_empty() {
        p.text(pos2(rect.center().x, cy), Align2::CENTER_CENTER, text, theme::bold(13.5), fg);
    }
    if has_sub {
        p.text(pos2(rect.center().x, rect.bottom() - 11.0), Align2::CENTER_CENTER, sub, theme::regular(10.5), theme::alpha(fg, 0.75));
    }
    resp
}

fn badge_color(pal: &Palette, r: Reference) -> Color32 {
    match r {
        Reference::None => pal.warn,
        Reference::TableZero => pal.blue,
        Reference::Homed => pal.ok,
    }
}

fn step_label(s: f64) -> String {
    let t = format!("{s:.3}");
    t.trim_end_matches('0').trim_end_matches('.').replace('.', ",")
}

impl Machines {
    pub(super) fn pilotage_ui(&mut self, ui: &mut Ui, pal: &Palette, units: Units, req: &mut Vec<Request>) {
        self.keyboard_jog(ui);
        ScrollArea::vertical().id_salt("mach-pilotage").auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 12.0;
            self.axes_row(ui, pal, units);
            if let Some((alarm, text)) = self.banner() {
                let color = if alarm { pal.bad } else { pal.warn };
                egui::Frame::new().fill(theme::alpha(color, 0.14)).stroke(Stroke::new(1.0, color)).corner_radius(CornerRadius::same(10)).inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                        icons::paint(ui.painter(), r, if alarm { Icon::Error } else { Icon::Warn }, color);
                        ui.add(egui::Label::new(RichText::new(text).size(13.5).strong().color(pal.text)).wrap());
                    });
                });
            }
            self.middle_row(ui, pal);
            self.actions_row(ui, pal, req);
            self.program_card(ui, pal, req);
            if self.console_open {
                self.console_card(ui, pal);
            }
        });
        self.step_window(ui.ctx(), pal);
    }

    /// Flèches pour X et Y, Page préc. / Page suiv. pour Z, hors saisie de texte.
    fn keyboard_jog(&mut self, ui: &Ui) {
        if ui.ctx().egui_wants_keyboard_input() || !self.connected() || self.confirm.is_some() {
            return;
        }
        let s = self.step;
        let keys = [(Key::ArrowUp, [0.0, s, 0.0]), (Key::ArrowDown, [0.0, -s, 0.0]), (Key::ArrowLeft, [-s, 0.0, 0.0]), (Key::ArrowRight, [s, 0.0, 0.0]), (Key::PageUp, [0.0, 0.0, s]), (Key::PageDown, [0.0, 0.0, -s])];
        for (k, d) in keys {
            if ui.input(|i| i.key_pressed(k)) {
                self.jog(d, false);
            }
        }
    }

    fn axes_row(&mut self, ui: &mut Ui, pal: &Palette, units: Units) {
        let on = self.connected() && self.run != Run::Running;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            if big(ui, pal, vec2(84.0, 112.0), Some(Icon::Zero), "Zéro XYZ", "G10 L20", Kind::Component, on).on_hover_text("La position actuelle devient le zéro pièce (X0 Y0 Z0)").clicked() {
                self.command("machine.zero", &mut Vec::new());
            }
            let w = ((ui.available_width() - 24.0) / 3.0).max(160.0);
            let (wp, mp) = (&self.status.wpos, &self.status.mpos);
            let values = [("X", wp.x, mp.x), ("Y", wp.y, mp.y), ("Z", wp.z, mp.z)];
            for (i, (axis, wv, mv)) in values.into_iter().enumerate() {
                let (rect, _) = ui.allocate_exact_size(vec2(w, 112.0), Sense::hover());
                let p = ui.painter();
                p.rect_filled(rect, CornerRadius::same(12), pal.card);
                p.rect_stroke(rect, CornerRadius::same(12), Stroke::new(1.0, pal.card_line), StrokeKind::Inside);
                p.text(rect.left_top() + vec2(16.0, 14.0), Align2::LEFT_TOP, axis, theme::bold(22.0), pal.orange);
                // badge de référence
                let lbl = self.reference.label();
                let c = badge_color(pal, self.reference);
                let g = p.layout_no_wrap(lbl.to_owned(), theme::bold(11.0), c);
                let br = Rect::from_min_size(pos2(rect.left() + 44.0, rect.top() + 17.0), g.size() + vec2(14.0, 6.0));
                p.rect_filled(br, CornerRadius::same(255), theme::alpha(c, 0.16));
                p.galley(br.min + vec2(7.0, 3.0), g, c);
                p.text(rect.left_top() + vec2(16.0, 48.0), Align2::LEFT_TOP, "PIÈCE", theme::bold(10.5), pal.weak);
                let value = if self.connected() { units.fmt(wv) } else { "-".to_owned() };
                p.text(rect.left_top() + vec2(16.0, 62.0), Align2::LEFT_TOP, value, theme::mono(22.0), pal.text);
                let machine = if !self.connected() || self.reference == Reference::None { "position inconnue".to_owned() } else { units.fmt(mv) };
                p.text(rect.left_bottom() + vec2(16.0, -10.0), Align2::LEFT_BOTTOM, format!("MACHINE  {machine}"), theme::regular(11.0), pal.weak);
                let zr = Rect::from_min_size(pos2(rect.right() - 84.0, rect.top() + 12.0), vec2(72.0, 30.0));
                let id = ["machine.zero.x", "machine.zero.y", "machine.zero.z"][i];
                let mut child = ui.new_child(UiBuilder::new().max_rect(zr));
                if widgets::button(&mut child, pal, None, &format!("Zéro {axis}"), Kind::Plain, on).clicked() {
                    self.command(id, &mut Vec::new());
                }
            }
        });
    }

    fn middle_row(&mut self, ui: &mut Ui, pal: &Palette) {
        let h = 410.0;
        let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
        let jog_w = 300.0_f32.min(row.width() * 0.34);
        let spin_w = 250.0_f32.min(row.width() * 0.28);
        let jog = Rect::from_min_size(row.min, vec2(jog_w, h));
        let spin = Rect::from_min_size(pos2(jog.right() + 12.0, row.top()), vec2(spin_w, h));
        let view = Rect::from_min_max(pos2(spin.right() + 12.0, row.top()), row.max);
        let mut j = ui.new_child(UiBuilder::new().max_rect(jog));
        widgets::card(&mut j, pal, Some("Déplacement manuel"), |ui| self.jog_ui(ui, pal));
        let mut s = ui.new_child(UiBuilder::new().max_rect(spin));
        s.spacing_mut().item_spacing.y = 12.0;
        self.spindle_ui(&mut s, pal);
        self.stop_ui(&mut s, pal);
        self.view_ui(ui, pal, view);
    }

    fn jog_ui(&mut self, ui: &mut Ui, pal: &Palette) {
        let on = self.connected() && self.run != Run::Running;
        let r = self.rights();
        ui.horizontal(|ui| {
            ui.label(RichText::new("PAS").font(theme::bold(11.5)).color(pal.text));
            ui.label(RichText::new("clic droit sur un pas pour le modifier").size(11.5).color(pal.weak));
        });
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let w = ((ui.available_width() - 18.0) / 4.0).max(40.0);
            for i in 0..self.steps.len() {
                let s = self.steps[i];
                let allowed = s <= r.max_step_mm;
                let kind = if (self.step - s).abs() < 1e-9 { Kind::Primary } else { Kind::Plain };
                let resp = big(ui, pal, vec2(w, 34.0), None, &format!("{} mm", step_label(s)), "", kind, allowed);
                if resp.clicked() {
                    self.step = s;
                }
                if allowed && resp.secondary_clicked() && r.edit_fiche {
                    self.step_edit = Some((i, s));
                }
            }
        });
        ui.add_space(6.0);
        let mut jog: Option<([f64; 3], bool)> = None;
        let s = self.step;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            let b = vec2(62.0, 62.0);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.add_space(b.x + 6.0);
                    if big(ui, pal, b, Some(Icon::Up), "Y+", "", Kind::Plain, on).on_hover_text("Y+ (flèche haut)").clicked() {
                        jog = Some(([0.0, s, 0.0], false));
                    }
                });
                ui.horizontal(|ui| {
                    if big(ui, pal, b, Some(Icon::Left), "X-", "", Kind::Plain, on).on_hover_text("X- (flèche gauche)").clicked() {
                        jog = Some(([-s, 0.0, 0.0], false));
                    }
                    if big(ui, pal, b, Some(Icon::Home), "Origine", "pièce", Kind::Component, on).on_hover_text("Retour au zéro pièce en X et Y ($J=G90 X0 Y0)").clicked() {
                        jog = Some(([0.0; 3], true));
                    }
                    if big(ui, pal, b, Some(Icon::Right), "X+", "", Kind::Plain, on).on_hover_text("X+ (flèche droite)").clicked() {
                        jog = Some(([s, 0.0, 0.0], false));
                    }
                });
                ui.horizontal(|ui| {
                    ui.add_space(b.x + 6.0);
                    if big(ui, pal, b, Some(Icon::Down), "Y-", "", Kind::Plain, on).on_hover_text("Y- (flèche bas)").clicked() {
                        jog = Some(([0.0, -s, 0.0], false));
                    }
                });
            });
            ui.add_space(6.0);
            ui.vertical(|ui| {
                let z = vec2(62.0, 96.0);
                if big(ui, pal, z, Some(Icon::Up), "Z+", "MONTER", Kind::Plain, on).on_hover_text("Z+ (Page préc.)").clicked() {
                    jog = Some(([0.0, 0.0, s], false));
                }
                if big(ui, pal, z, Some(Icon::Down), "Z-", "DESCENDRE", Kind::Plain, on).on_hover_text("Z- (Page suiv.)").clicked() {
                    jog = Some(([0.0, 0.0, -s], false));
                }
            });
        });
        if let Some((d, abs)) = jog {
            self.jog(d, abs);
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            widgets::caption(ui, pal, "Vitesse");
            let max = r.max_jog_feed;
            self.feed = self.feed.min(max);
            ui.spacing_mut().slider_width = (ui.available_width() - 120.0).max(60.0);
            ui.add(egui::Slider::new(&mut self.feed, 50.0..=max).step_by(50.0).suffix(" mm/min"));
        });
        if self.reference == Reference::None && !self.emulator && self.connected() {
            ui.label(RichText::new("Sans zéro valable : libre sous 1 mm, au-delà une ou deux validations.").size(11.5).color(pal.warn));
        }
    }

    fn spindle_ui(&mut self, ui: &mut Ui, pal: &Palette) {
        let r = self.rights();
        let on = self.connected() && self.run != Run::Running;
        let rpm_max = self.rpm_max();
        widgets::card(ui, pal, Some("Broche"), |ui| {
            let turning = self.connected() && self.status.spindle > 0.0;
            let (txt, c) = if turning { (format!("EN MARCHE  {:.0} tr/min", self.status.spindle), pal.ok) } else { ("ARRÊTÉE".to_owned(), pal.weak) };
            ui.label(RichText::new(txt).font(theme::bold(14.0)).color(c));
            ui.horizontal(|ui| {
                let w = (ui.available_width() - 8.0) / 2.0;
                if big(ui, pal, vec2(w, 44.0), Some(Icon::Play), "Marche", "", Kind::Component, on && r.spindle).on_hover_text("M3 : mise en marche (confirmation sur une vraie machine)").clicked() {
                    self.command("machine.broche.marche", &mut Vec::new());
                }
                if big(ui, pal, vec2(w, 44.0), Some(Icon::Stop), "Arrêt", "", Kind::Plain, self.connected()).on_hover_text("M5 : arrêt de la broche").clicked() {
                    self.command("machine.broche.arret", &mut Vec::new());
                }
            });
            self.rpm = self.rpm.clamp(1_000.0, rpm_max.max(1_000.0));
            ui.spacing_mut().slider_width = (ui.available_width() - 110.0).max(60.0);
            ui.add_enabled(r.spindle, egui::Slider::new(&mut self.rpm, 1_000.0..=rpm_max.max(1_000.0)).step_by(500.0).suffix(" tr/min"));
            let note = if r.spindle { format!("{} tr/min max.", super::thousands(rpm_max)) } else { "Broche réservée au prof.".to_owned() };
            ui.label(RichText::new(note).size(11.5).color(pal.weak));
        });
    }

    /// Arrêt logiciel, collé sous la broche : toujours disponible une fois connecté.
    fn stop_ui(&mut self, ui: &mut Ui, pal: &Palette) {
        let connected = self.connected();
        widgets::card(ui, pal, None, |ui| {
            let note = if self.equipment.estop { "Arrêt logiciel - pour couper vraiment : arrêt d'urgence" } else { "Arrêt logiciel - pour couper vraiment : débrancher l'alimentation" };
            ui.add(egui::Label::new(RichText::new(note).size(11.5).color(pal.weak)).wrap());
            ui.horizontal(|ui| {
                let side = vec2(52.0, 52.0);
                let w = (ui.available_width() - 2.0 * side.x - 12.0).max(80.0);
                if big(ui, pal, side, Some(Icon::Pause), "", "Pause", Kind::Plain, connected).on_hover_text("! : suspendre (feed hold)").clicked() {
                    if self.run == Run::Running {
                        self.command("machine.pause", &mut Vec::new());
                    } else if let Some(c) = &self.ctrl {
                        c.hold();
                    }
                }
                if big(ui, pal, vec2(w, 52.0), None, "ARRÊT", "", Kind::Danger, connected).on_hover_text("Pause puis reset de la carte, broche coupée").clicked() {
                    self.command("machine.arret", &mut Vec::new());
                }
                if big(ui, pal, side, Some(Icon::Play), "", "Reprendre", Kind::Plain, connected).on_hover_text("~ : reprendre").clicked() {
                    self.command("machine.reprise", &mut Vec::new());
                }
            });
        });
    }

    fn view_ui(&mut self, ui: &mut Ui, pal: &Palette, rect: Rect) {
        let mut v = ui.new_child(UiBuilder::new().max_rect(rect));
        widgets::card(&mut v, pal, None, |ui| {
            ui.horizontal(|ui| {
                for (view, label) in [(View::Iso, "3D"), (View::Top, "2D dessus"), (View::Camera, "Caméra")] {
                    let enabled = view != View::Camera;
                    let kind = if self.view == view { Kind::Primary } else { Kind::Plain };
                    let r = widgets::button(ui, pal, None, label, kind, enabled);
                    let r = if enabled { r } else { r.on_disabled_hover_text("Aucune caméra détectée (à venir)") };
                    if r.clicked() {
                        self.view = view;
                    }
                }
            });
            let (area, _) = ui.allocate_exact_size(vec2(ui.available_width(), (ui.available_height() - 4.0).max(200.0)), Sense::hover());
            let st = &self.status;
            let scene = Scene {
                travel: self.travel(),
                spoilboard_mm: self.equipment.spoilboard_mm,
                mpos: [st.mpos.x, st.mpos.y, st.mpos.z],
                wco: [st.wco.x, st.wco.y, st.wco.z],
                reference: self.reference,
                connected: self.connected(),
            };
            vue::paint(ui, pal, area, self.view, &scene);
        });
    }

    /// Rangée de commandes Grbl, six boutons de même largeur.
    fn actions_row(&mut self, ui: &mut Ui, pal: &Palette, req: &mut Vec<Request>) {
        let r = self.rights();
        let on = self.connected() && self.run != Run::Running;
        let e = self.equipment.clone();
        let home_sub = if e.can_home() { "$H" } else { "fins de course à venir" };
        let probe_sub = if e.probe { "assistant à venir" } else { "sonde à venir" };
        let console = if self.console_open { "Replier" } else { "Console" };
        let items: [(&str, Icon, &str, &str, bool); 6] = [
            ("machine.reset", Icon::Undo, "Réinitialiser", "Ctrl-X", on && r.unlock),
            ("machine.deverrouiller", Icon::Unlock, "Déverrouiller", "$X", on && r.unlock),
            ("machine.origine", Icon::Home, "Origine machine", home_sub, on && e.can_home()),
            ("machine.palper", Icon::Down, "Palper Z", probe_sub, false),
            ("machine.lire", Icon::Info, "Lire position", "?", self.connected()),
            ("console", Icon::File, console, "Grbl", true),
        ];
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let w = (ui.available_width() - 5.0 * 8.0) / 6.0;
            for (id, icon, text, sub, enabled) in items {
                if big(ui, pal, vec2(w, 58.0), Some(icon), text, sub, Kind::Plain, enabled).clicked() {
                    if id == "console" {
                        self.console_open = !self.console_open;
                    } else {
                        self.command(id, req);
                    }
                }
            }
        });
    }

    fn program_card(&mut self, ui: &mut Ui, pal: &Palette, req: &mut Vec<Request>) {
        let r = self.rights();
        widgets::card(ui, pal, Some("Programme"), |ui| {
            ui.horizontal(|ui| {
                let can_load = self.run != Run::Running;
                if widgets::button(ui, pal, Some(Icon::Open), "Ouvrir un .nc...", Kind::Plain, can_load).clicked() {
                    self.command("machine.ouvrir", req);
                }
                match &self.program {
                    Some(p) => {
                        let name = p.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        ui.label(RichText::new(name).font(theme::mono(13.5)));
                        ui.label(RichText::new(format!("{} lignes", p.text.lines.len())).color(pal.weak));
                    }
                    None => {
                        ui.label(RichText::new("aucun programme chargé").color(pal.weak));
                    }
                }
                let connected = self.connected();
                let running = self.run == Run::Running;
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::button(ui, pal, Some(Icon::Play), "Démarrer", Kind::Component, connected && !running && self.program.is_some() && r.run_program).clicked() {
                        self.command("machine.demarrer", req);
                    }
                    if !r.run_program {
                        ui.label(RichText::new("lancement par le prof").size(12.0).color(pal.weak));
                    }
                });
            });
            let pr = self.progress;
            let frac = if pr.total == 0 { 0.0 } else { pr.acked as f32 / pr.total as f32 };
            let text = match self.run {
                Run::Idle => format!("prêt  -  {} ligne(s)", pr.total),
                Run::Running if self.paused => format!("en pause  -  {} / {}", pr.acked, pr.total),
                Run::Running => format!("{:.1} %  -  {} / {}", pr.percent(), pr.acked, pr.total),
                Run::Finished => format!("terminé  -  {} erreur(s)", pr.errors),
                Run::Aborted => format!("arrêté à la ligne {}", pr.acked),
            };
            widgets::progress(ui, pal, frac, &text);
        });
    }

    fn console_card(&mut self, ui: &mut Ui, pal: &Palette) {
        let input = self.rights().console_input;
        widgets::card(ui, pal, Some("Console Grbl"), |ui| {
            ScrollArea::vertical().id_salt("console").max_height(220.0).stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
                for l in &self.log {
                    ui.label(RichText::new(l).font(theme::mono(12.5)));
                }
            });
            if !input {
                ui.label(RichText::new("Console en lecture seule en mode élève.").size(12.0).color(pal.weak));
                return;
            }
            ui.horizontal(|ui| {
                let on = self.connected();
                let r = ui.add_enabled(on, TextEdit::singleline(&mut self.input).hint_text("Commande Grbl (ex. ?, $X, M5, G10 L20 P1 X0), Entrée pour envoyer").desired_width(ui.available_width() - 110.0));
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                let click = widgets::button(ui, pal, None, "Envoyer", Kind::Plain, on).clicked();
                if (enter || click) && !self.input.trim().is_empty() {
                    let line = self.input.trim().to_owned();
                    if let Some(c) = &self.ctrl {
                        c.send(line.clone());
                    }
                    self.log(format!("> {line}"));
                    self.input.clear();
                    r.request_focus();
                }
            });
        });
    }

    /// Fenêtre de modification d'un pas (clic droit).
    fn step_window(&mut self, ctx: &egui::Context, pal: &Palette) {
        let Some((i, mut v)) = self.step_edit else { return };
        let max = self.rights().max_step_mm.min(100.0);
        let mut done = None;
        egui::Window::new("Modifier le pas").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
            ui.add(egui::DragValue::new(&mut v).range(0.01..=max).speed(0.05).suffix(" mm"));
            ui.horizontal(|ui| {
                if widgets::button(ui, pal, Some(Icon::Ok), "Valider", Kind::Primary, true).clicked() {
                    done = Some(true);
                }
                if widgets::button(ui, pal, None, "Annuler", Kind::Plain, true).clicked() {
                    done = Some(false);
                }
            });
        });
        match done {
            Some(true) => {
                if let Some(s) = self.steps.get_mut(i) {
                    if (self.step - *s).abs() < 1e-9 {
                        self.step = v;
                    }
                    *s = v;
                }
                self.step_edit = None;
            }
            Some(false) => self.step_edit = None,
            None => self.step_edit = Some((i, v)),
        }
    }
}
