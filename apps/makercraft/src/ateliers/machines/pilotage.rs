//! Onglet Pilotage, sur le modèle de l'image de référence du 16/11 : cartes d'axes en couleur
//! (pièce et table), bandeau d'alarme ou de changement d'outil, déplacement manuel | broche et
//! arrêt logiciel | vue 3D / 2D / caméra, rangée de commandes Grbl, programme et console.

use eframe::egui::{self, Align2, Color32, CornerRadius, Key, Rect, Response, RichText, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, Ui, UiBuilder, pos2, vec2};
use makercraft_ext_machine::MachineState;

use super::equip::Reference;
use super::fiche::badge;
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

/// Contrôle segmenté (pas, vue) : renvoie l'index cliqué.
fn segmented(ui: &mut Ui, pal: &Palette, labels: &[String], current: Option<usize>, enabled: &[bool], height: f32) -> (Option<usize>, Option<usize>) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, height), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(8), pal.panel);
    ui.painter().rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.0, pal.card_line), StrokeKind::Inside);
    let n = labels.len().max(1) as f32;
    let cw = rect.width() / n;
    let (mut clicked, mut secondary) = (None, None);
    for (i, l) in labels.iter().enumerate() {
        let r = Rect::from_min_size(pos2(rect.left() + i as f32 * cw, rect.top()), vec2(cw, height));
        let on = enabled.get(i).copied().unwrap_or(true);
        let resp = ui.interact(r, ui.make_persistent_id(("seg", l, i)), if on { Sense::click() } else { Sense::hover() });
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, on, current == Some(i), l));
        let sel = current == Some(i);
        if sel {
            ui.painter().rect_filled(r.shrink(2.0), CornerRadius::same(6), theme::mix(pal.blue, pal.panel, 0.25));
        } else if on && resp.hovered() {
            ui.painter().rect_filled(r.shrink(2.0), CornerRadius::same(6), pal.hover);
        }
        if i > 0 && !sel && current != Some(i - 1) {
            ui.painter().vline(r.left(), r.y_range().shrink(8.0), Stroke::new(1.0, pal.card_line));
        }
        let c = if !on { pal.dim } else if sel { pal.on_blue } else { pal.text };
        ui.painter().text(r.center(), Align2::CENTER_CENTER, l, theme::bold(13.5), c);
        if resp.clicked() {
            clicked = Some(i);
        }
        if resp.secondary_clicked() {
            secondary = Some(i);
        }
    }
    (clicked, secondary)
}

fn step_label(s: f64) -> String {
    let t = format!("{s:.3}");
    t.trim_end_matches('0').trim_end_matches('.').replace('.', ",")
}

/// Valeur d'axe : 3 décimales et virgule en mm, 4 décimales en pouces.
fn axis_value(units: Units, mm: f64) -> (String, &'static str) {
    match units {
        Units::Mm => (format!("{mm:.3}").replace('.', ","), "mm"),
        Units::Inch => (format!("{:.4}", mm / 25.4).replace('.', ","), "in"),
    }
}

impl Machines {
    pub(super) fn pilotage_ui(&mut self, ui: &mut Ui, pal: &Palette, units: Units, req: &mut Vec<Request>) {
        self.keyboard_jog(ui);
        ScrollArea::vertical().id_salt("mach-pilotage").auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 12.0;
            self.axes_row(ui, pal, units);
            self.banner_ui(ui, pal);
            self.middle_row(ui, pal);
            self.actions_row(ui, pal, req);
            self.program_card(ui, pal, req);
            if self.console_open {
                self.console_card(ui, pal);
            }
        });
        self.step_window(ui.ctx(), pal);
        self.z_redo_window(ui.ctx(), pal);
    }

    /// Flèches pour X et Y, Page préc. / Page suiv. pour Z, hors saisie de texte.
    fn keyboard_jog(&mut self, ui: &Ui) {
        if ui.ctx().egui_wants_keyboard_input() || !self.connected() || self.confirm.is_some() || self.z_redo.is_some() {
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
        let connected = self.connected();
        let table = self.table_pos();
        let badge_text = match self.reference {
            Reference::None => "non référencé",
            Reference::TableZero => "zéro table (manuel)",
            Reference::Homed => "référencé ($H)",
        };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            if big(ui, pal, vec2(110.0, 96.0), None, "Zéro XYZ", "G10 L20", Kind::Component, on).on_hover_text("La position actuelle devient le zéro pièce (X0 Y0 Z0)").clicked() {
                self.command("machine.zero", &mut Vec::new());
            }
            let w = ((ui.available_width() - 24.0) / 3.0).max(180.0);
            let (wp, mp) = (&self.status.wpos, &self.status.mpos);
            let values = [("X", wp.x, mp.x, pal.red), ("Y", wp.y, mp.y, pal.green), ("Z", wp.z, mp.z, pal.blue)];
            for (i, (axis, wv, _mv, color)) in values.into_iter().enumerate() {
                let (rect, _) = ui.allocate_exact_size(vec2(w, 96.0), Sense::hover());
                let p = ui.painter();
                p.rect_filled(rect, CornerRadius::same(12), pal.card);
                p.rect_stroke(rect, CornerRadius::same(12), Stroke::new(1.0, pal.card_line), StrokeKind::Inside);
                // bandeau de l'axe
                let band = Rect::from_min_size(rect.min, vec2(rect.width(), 34.0));
                p.rect_filled(band, CornerRadius { nw: 12, ne: 12, sw: 0, se: 0 }, color);
                p.text(band.left_center() + vec2(14.0, 0.0), Align2::LEFT_CENTER, axis, theme::bold(22.0), Color32::WHITE);
                let g = p.layout_no_wrap(badge_text.to_owned(), theme::bold(11.5), Color32::WHITE);
                let br = Rect::from_min_size(pos2(band.right() - g.size().x - 26.0, band.center().y - g.size().y / 2.0 - 4.0), g.size() + vec2(16.0, 8.0));
                p.rect_filled(br, CornerRadius::same(6), Color32::from_black_alpha(110));
                p.galley(br.min + vec2(8.0, 4.0), g, Color32::WHITE);
                // valeurs
                let x0 = rect.left() + 14.0;
                p.text(pos2(x0, band.bottom() + 18.0), Align2::LEFT_CENTER, "PIÈCE", theme::bold(11.0), pal.weak);
                p.text(pos2(x0, band.bottom() + 42.0), Align2::LEFT_CENTER, "TABLE", theme::bold(11.0), pal.weak);
                let vx = rect.left() + w * 0.62;
                let (v, u) = if connected { axis_value(units, wv) } else { ("-".to_owned(), "") };
                p.text(pos2(vx, band.bottom() + 18.0), Align2::RIGHT_CENTER, format!("{v} {u}"), theme::bold(21.0), pal.text);
                let t = match (connected, table) {
                    (true, Some(t)) => axis_value(units, t[i]).0,
                    (true, None) => "inconnue".to_owned(),
                    (false, _) => "-".to_owned(),
                };
                p.text(pos2(vx, band.bottom() + 42.0), Align2::RIGHT_CENTER, t, theme::regular(13.5), pal.weak);
                let zr = Rect::from_min_size(pos2(rect.right() - 84.0, band.bottom() + 14.0), vec2(72.0, 34.0));
                let id = ["machine.zero.x", "machine.zero.y", "machine.zero.z"][i];
                let resp = ui.interact(zr, ui.make_persistent_id(id), if on { Sense::click() } else { Sense::hover() });
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, on, format!("Zéro {axis}")));
                let c = if on { color } else { theme::alpha(color, 0.35) };
                let c = if on && resp.hovered() { c.gamma_multiply(1.2) } else { c };
                ui.painter().rect_filled(zr, CornerRadius::same(8), c);
                ui.painter().text(zr.center(), Align2::CENTER_CENTER, format!("Zéro {axis}"), theme::bold(13.0), Color32::WHITE);
                if resp.clicked() {
                    self.command(id, &mut Vec::new());
                }
            }
        });
    }

    /// Bandeau : alarme, ou changement d'outil avec « Refaire le Z ».
    fn banner_ui(&mut self, ui: &mut Ui, pal: &Palette) {
        let tool_change = self.pause_note.as_ref().is_some_and(|n| n.to_ascii_lowercase().contains("outil"));
        let (color, icon, text) = if let Some((alarm, text)) = self.banner().filter(|(alarm, _)| *alarm || !tool_change) {
            if alarm { (pal.bad, Icon::Error, text) } else { (pal.warn, Icon::Warn, text) }
        } else if tool_change {
            let note = self.pause_note.clone().unwrap_or_default();
            (pal.orange, Icon::Warn, format!("Outil changé : refaire le Z (XY conservés)  -  {note}"))
        } else {
            return;
        };
        egui::Frame::new().fill(theme::alpha(color, 0.85)).corner_radius(CornerRadius::same(10)).inner_margin(egui::Margin::symmetric(14, 8)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                icons::paint(ui.painter(), r, icon, Color32::BLACK);
                ui.add(egui::Label::new(RichText::new(text).size(14.5).strong().color(Color32::BLACK)).wrap());
                if tool_change {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let idle = self.status.state == MachineState::Idle;
                        let r = widgets::button(ui, pal, Some(Icon::Down), "Refaire le Z", Kind::Plain, idle);
                        let r = if idle { r } else { r.on_disabled_hover_text("Grbl refuse les déplacements pendant une pause M0 : possible quand la machine est au repos (un fichier par outil)") };
                        if r.clicked() {
                            self.z_redo = Some((0.1, 0.10));
                        }
                    });
                }
            });
        });
    }

    fn middle_row(&mut self, ui: &mut Ui, pal: &Palette) {
        let h = 400.0;
        let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
        let jog_w = 340.0_f32.min(row.width() * 0.32);
        let spin_w = 270.0_f32.min(row.width() * 0.25);
        let jog = Rect::from_min_size(row.min, vec2(jog_w, h));
        let spin = Rect::from_min_size(pos2(jog.right() + 12.0, row.top()), vec2(spin_w, h));
        let view = Rect::from_min_max(pos2(spin.right() + 12.0, row.top()), row.max);
        let mut j = ui.new_child(UiBuilder::new().max_rect(jog));
        widgets::card(&mut j, pal, None, |ui| self.jog_ui(ui, pal));
        let mut s = ui.new_child(UiBuilder::new().max_rect(spin));
        s.spacing_mut().item_spacing.y = 12.0;
        self.spindle_ui(&mut s, pal);
        self.stop_ui(&mut s, pal);
        self.view_ui(ui, pal, view);
    }

    fn jog_ui(&mut self, ui: &mut Ui, pal: &Palette) {
        let on = self.connected() && self.run != Run::Running;
        let r = self.rights();
        super::fiche::heading(ui, pal, "Déplacement manuel");
        let labels: Vec<String> = self.steps.iter().map(|s| format!("{} mm", step_label(*s))).collect();
        let allowed: Vec<bool> = self.steps.iter().map(|s| *s <= r.max_step_mm).collect();
        let current = self.steps.iter().position(|s| (self.step - s).abs() < 1e-9);
        let (click, sec) = segmented(ui, pal, &labels, current, &allowed, 34.0);
        if let Some(i) = click {
            self.step = self.steps[i];
        }
        if let Some(i) = sec
            && r.edit_fiche
        {
            self.step_edit = Some((i, self.steps[i]));
        }
        ui.label(RichText::new("PAS  clic droit sur un pas pour le modifier").size(11.5).color(pal.weak));
        ui.add_space(4.0);
        let mut jog: Option<([f64; 3], bool)> = None;
        let s = self.step;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            let b = vec2(64.0, 54.0);
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
                    if big(ui, pal, b, None, "ORIGINE", "PIÈCE", Kind::Primary, on).on_hover_text("Retour au zéro pièce en X et Y ($J=G90 X0 Y0)").clicked() {
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
            ui.add_space(4.0);
            ui.vertical(|ui| {
                let z = vec2((ui.available_width()).clamp(80.0, 130.0), 84.0);
                if big(ui, pal, z, None, "Z+", "MONTER", Kind::Plain, on).on_hover_text("Z+ (Page préc.)").clicked() {
                    jog = Some(([0.0, 0.0, s], false));
                }
                if big(ui, pal, z, None, "Z-", "DESCENDRE", Kind::Plain, on).on_hover_text("Z- (Page suiv.)").clicked() {
                    jog = Some(([0.0, 0.0, -s], false));
                }
            });
        });
        if let Some((d, abs)) = jog {
            self.jog(d, abs);
        }
        ui.add_space(4.0);
        let max = r.max_jog_feed;
        self.feed = self.feed.clamp(50.0, max);
        ui.horizontal(|ui| {
            ui.label(RichText::new("VITESSE").font(theme::bold(12.0)).color(pal.text));
            ui.spacing_mut().slider_width = (ui.available_width() - 8.0).max(60.0);
            ui.add(egui::Slider::new(&mut self.feed, 50.0..=max).step_by(50.0).show_value(false));
        });
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(format!("Vitesse : {} mm/min", super::thousands(self.feed))).size(13.0));
        });
        if self.reference == Reference::None && !self.emulator && self.connected() {
            ui.label(RichText::new("Sans zéro valable : libre sous 1 mm, au-delà une ou deux validations.").size(11.5).color(pal.warn));
        }
    }

    fn spindle_ui(&mut self, ui: &mut Ui, pal: &Palette) {
        let r = self.rights();
        let on = self.connected() && self.run != Run::Running;
        let rpm_max = self.rpm_max().max(1_000.0);
        widgets::card(ui, pal, None, |ui| {
            let turning = self.connected() && self.status.spindle > 0.0;
            ui.horizontal(|ui| {
                ui.label(RichText::new("BROCHE").font(theme::bold(19.0)).color(pal.orange));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (t, bg, fg) = if turning { ("EN MARCHE", pal.ok, Color32::BLACK) } else { ("ARRÊTÉE", pal.line2, pal.text) };
                    badge(ui, t, bg, fg, false);
                });
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let txt = if turning { format!("{} tr/min", super::thousands(self.status.spindle)) } else { "arrêtée".to_owned() };
                ui.label(RichText::new(txt).size(15.0).color(if turning { pal.ok } else { pal.weak }));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if big(ui, pal, vec2(46.0, 40.0), Some(Icon::Stop), "", "", Kind::Plain, self.connected()).on_hover_text("M5 : arrêt de la broche").clicked() {
                        self.command("machine.broche.arret", &mut Vec::new());
                    }
                    if big(ui, pal, vec2(46.0, 40.0), Some(Icon::Play), "", "", Kind::Plain, on && r.spindle).on_hover_text("M3 : mise en marche (confirmation sur une vraie machine)").clicked() {
                        self.command("machine.broche.marche", &mut Vec::new());
                    }
                });
            });
            self.rpm = self.rpm.clamp(1_000.0, rpm_max);
            ui.spacing_mut().slider_width = ui.available_width();
            ui.add_enabled(r.spindle, egui::Slider::new(&mut self.rpm, 1_000.0..=rpm_max).step_by(500.0).show_value(false));
            ui.vertical_centered(|ui| {
                let t = if r.spindle { format!("Vitesse : {} tr/min", super::thousands(self.rpm)) } else { "Broche réservée au prof".to_owned() };
                ui.label(RichText::new(t).size(13.0));
            });
        });
    }

    /// Arrêt logiciel, collé sous la broche : toujours disponible une fois connecté.
    fn stop_ui(&mut self, ui: &mut Ui, pal: &Palette) {
        let connected = self.connected();
        widgets::card(ui, pal, None, |ui| {
            ui.vertical_centered(|ui| {
                ui.horizontal(|ui| {
                    let tw = 90.0;
                    ui.add_space(((ui.available_width() - tw) / 2.0).max(0.0));
                    let (r, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                    icons::paint(ui.painter(), r, Icon::Warn, pal.orange);
                    ui.label(RichText::new("Arrêt").font(theme::bold(17.0)).color(pal.orange));
                });
                let note = if self.equipment.estop { "Arrêt logiciel - pour couper vraiment, arrêt d'urgence" } else { "Arrêt logiciel - pour couper vraiment, débrancher l'alimentation" };
                ui.add(egui::Label::new(RichText::new(note).size(12.0).color(pal.weak)).wrap());
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let side = vec2(62.0, 56.0);
                let gap = ((ui.available_width() - 2.0 * side.x - 70.0) / 2.0 - 8.0).max(0.0);
                if big(ui, pal, side, Some(Icon::Pause), "", "Pause", Kind::Plain, connected).on_hover_text("! : suspendre (feed hold)").clicked() {
                    if self.run == Run::Running {
                        self.command("machine.pause", &mut Vec::new());
                    } else if let Some(c) = &self.ctrl {
                        c.hold();
                    }
                }
                ui.add_space(gap);
                let (r, resp) = ui.allocate_exact_size(vec2(70.0, 70.0), if connected { Sense::click() } else { Sense::hover() });
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, connected, "ARRÊT"));
                let c = if connected { pal.bad } else { theme::alpha(pal.bad, 0.35) };
                let c = if connected && resp.hovered() { c.gamma_multiply(0.85) } else { c };
                ui.painter().circle_filled(r.center(), 34.0, c);
                ui.painter().circle_stroke(r.center(), 34.0, Stroke::new(2.0, theme::alpha(Color32::WHITE, 0.25)));
                ui.painter().text(r.center(), Align2::CENTER_CENTER, "ARRÊT", theme::bold(14.0), Color32::WHITE);
                if resp.on_hover_text("Pause puis reset de la carte, broche coupée").clicked() {
                    self.command("machine.arret", &mut Vec::new());
                }
                ui.add_space(gap);
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
                ui.label(RichText::new("Vue").font(theme::bold(19.0)).color(pal.orange));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.allocate_ui(vec2(220.0, 34.0), |ui| {
                        let views = [View::Iso, View::Top, View::Camera];
                        let labels = ["3D".to_owned(), "2D".to_owned(), "Caméra".to_owned()];
                        let current = views.iter().position(|v| *v == self.view);
                        let (click, _) = segmented(ui, pal, &labels, current, &[true, true, self.equipment.camera], 32.0);
                        if let Some(i) = click {
                            self.view = views[i];
                        }
                    });
                });
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
        let console = if self.console_open { "Replier la console" } else { "Console" };
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
                    if running && widgets::button(ui, pal, Some(Icon::Stop), "Arrêt", Kind::Danger, true).on_hover_text("Arrêt logiciel : pause puis reset de la carte").clicked() {
                        self.command("machine.arret", req);
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

    /// « Refaire le Z » après un changement d'outil : descendre à la cale par pas de 0,1 puis
    /// 0,01 mm, puis le dessus de la pièce devient Z0 (épaisseur de la cale déduite).
    fn z_redo_window(&mut self, ctx: &egui::Context, pal: &Palette) {
        let Some((mut step, mut cale)) = self.z_redo else { return };
        let mut close = false;
        let mut jog = None;
        let mut validate = false;
        egui::Window::new("Refaire le Z").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
            ui.label("1. Glisser la cale sous l'outil (X et Y ne bougent pas).");
            ui.label("2. Descendre par pas de 0,1 mm puis 0,01 mm.");
            ui.label("3. S'arrêter quand la cale frotte légèrement, puis valider.");
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Cale utilisée :");
                egui::ComboBox::from_id_salt("cale").selected_text(format!("{} mm", step_label(cale))).show_ui(ui, |ui| {
                    ui.selectable_value(&mut cale, 0.10, "Papier 80-90 g/m² (0,10 mm)");
                    ui.selectable_value(&mut cale, 0.08, "Papier fin (0,08 mm)");
                });
                ui.add(egui::DragValue::new(&mut cale).range(0.0..=1.0).speed(0.01).suffix(" mm")).on_hover_text("Lame du jeu de cales : épaisseur");
            });
            ui.horizontal(|ui| {
                for s in [0.1, 0.01] {
                    let k = if (step - s).abs() < 1e-9 { Kind::Primary } else { Kind::Plain };
                    if widgets::button(ui, pal, None, &format!("{} mm", step_label(s)), k, true).clicked() {
                        step = s;
                    }
                }
                if widgets::button(ui, pal, Some(Icon::Up), "Z+", Kind::Plain, true).clicked() {
                    jog = Some(step);
                }
                if widgets::button(ui, pal, Some(Icon::Down), "Z-", Kind::Plain, true).clicked() {
                    jog = Some(-step);
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                validate = widgets::button(ui, pal, Some(Icon::Ok), "Valider le Z", Kind::Component, true).clicked();
                close = widgets::button(ui, pal, None, "Annuler", Kind::Plain, true).clicked();
            });
        });
        if let Some(dz) = jog {
            self.jog([0.0, 0.0, dz], false);
        }
        if validate {
            if let Some(c) = &self.ctrl {
                c.send(format!("G10 L20 P1 Z{cale:.3}"));
            }
            self.log(format!("Z refait : dessus de la pièce = Z0 (cale de {} mm déduite)", step_label(cale)));
            close = true;
        }
        self.z_redo = if close { None } else { Some((step, cale)) };
    }
}
