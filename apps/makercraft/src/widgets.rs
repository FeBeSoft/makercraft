//! Petits éléments d'interface communs : cartes, boutons à icône vectorielle, pastilles.

use eframe::egui::{self, Align2, Color32, CornerRadius, Margin, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};

use crate::icons::{self, Icon};
use crate::theme::{self, Palette};

/// Carte : fond relevé, coins arrondis, marges généreuses.
pub fn card<R>(ui: &mut Ui, pal: &Palette, title: Option<&str>, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(pal.card)
        .stroke(Stroke::new(1.0, pal.card_line))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if let Some(t) = title {
                ui.label(RichText::new(t).font(theme::bold(15.0)).color(pal.text));
                ui.add_space(2.0);
            }
            add(ui)
        })
        .inner
}

/// Style d'un bouton.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Action principale (bleu).
    Primary,
    /// Action sur un composant / la machine (orange).
    Component,
    /// Action secondaire (neutre).
    Plain,
    /// Danger (rouge) : arrêt.
    Danger,
}

/// Bouton avec icône vectorielle et texte (texte vide : bouton carré).
pub fn button(ui: &mut Ui, pal: &Palette, icon: Option<Icon>, text: &str, kind: Kind, enabled: bool) -> Response {
    let font = theme::bold(14.0);
    let tw = if text.is_empty() { 0.0 } else { ui.painter().layout_no_wrap(text.to_owned(), font.clone(), pal.text).size().x };
    let iw = if icon.is_some() { 18.0 } else { 0.0 };
    let gap = if icon.is_some() && !text.is_empty() { 8.0 } else { 0.0 };
    let w = if text.is_empty() { 36.0 } else { tw + iw + gap + 28.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 36.0), if enabled { Sense::click() } else { Sense::hover() });
    let label = if text.is_empty() { None } else { Some(text) };
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, label.unwrap_or("")));
    let (bg, fg, line) = match kind {
        Kind::Primary => (pal.blue, pal.on_blue, pal.blue),
        Kind::Component => (pal.orange, pal.on_orange, pal.orange),
        Kind::Danger => (pal.bad, Color32::WHITE, pal.bad),
        Kind::Plain => (pal.card, pal.text, pal.card_line),
    };
    let hovered = enabled && resp.hovered();
    let bg = if !enabled {
        theme::alpha(bg, 0.35)
    } else if hovered && kind == Kind::Plain {
        pal.blue_soft
    } else if hovered {
        bg.gamma_multiply(0.9)
    } else {
        bg
    };
    let fg = if enabled {
        fg
    } else if kind == Kind::Plain {
        theme::alpha(pal.weak, 0.55)
    } else {
        theme::alpha(fg, 0.75)
    };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(8), bg);
    let line = if hovered && kind == Kind::Plain { pal.blue } else { line };
    p.rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.0, if enabled { line } else { theme::alpha(line, 0.35) }), StrokeKind::Inside);
    let mut x = if text.is_empty() { rect.center().x - iw / 2.0 } else { rect.left() + 14.0 };
    if let Some(i) = icon {
        icons::paint(p, Rect::from_min_size(pos2(x, rect.center().y - 9.0), vec2(18.0, 18.0)), i, fg);
        x += iw + gap;
    }
    if !text.is_empty() {
        p.text(pos2(x, rect.center().y), Align2::LEFT_CENTER, text, font, fg);
    }
    resp
}

/// Onglet d'atelier (icône + nom), surligné quand actif.
pub fn tab(ui: &mut Ui, pal: &Palette, icon: Icon, text: &str, active: bool, soon: bool) -> Response {
    let font = theme::bold(14.5);
    let tw = ui.painter().layout_no_wrap(text.to_owned(), font.clone(), pal.text).size().x;
    let (rect, resp) = ui.allocate_exact_size(vec2(tw + 52.0, 40.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, active, text));
    let p = ui.painter();
    let (bg, fg) = if active {
        (pal.blue, pal.on_blue)
    } else if resp.hovered() {
        (pal.blue_soft, pal.text)
    } else {
        (Color32::TRANSPARENT, if soon { pal.weak } else { pal.text })
    };
    p.rect_filled(rect, CornerRadius::same(10), bg);
    icons::paint(p, Rect::from_min_size(pos2(rect.left() + 12.0, rect.center().y - 10.0), vec2(20.0, 20.0)), icon, fg);
    p.text(pos2(rect.left() + 40.0, rect.center().y), Align2::LEFT_CENTER, text, font, fg);
    resp
}

/// Segment d'un sélecteur (sous-atelier) ; `soon` : affiché atténué.
pub fn segment(ui: &mut Ui, pal: &Palette, text: &str, active: bool, soon: bool) -> Response {
    let font = theme::regular(13.5);
    let tw = ui.painter().layout_no_wrap(text.to_owned(), font.clone(), pal.text).size().x;
    let (rect, resp) = ui.allocate_exact_size(vec2(tw + 26.0, 32.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, active, text));
    let p = ui.painter();
    if active {
        p.rect_filled(rect, CornerRadius::same(8), pal.card);
        p.rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.5, pal.blue), StrokeKind::Inside);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(8), pal.blue_soft);
    }
    let fg = if active {
        pal.blue
    } else if soon {
        pal.weak
    } else {
        pal.text
    };
    p.text(rect.center(), Align2::CENTER_CENTER, text, if active { theme::bold(13.5) } else { font }, fg);
    resp
}

/// Pastille colorée avec texte (ex. état machine, outil).
pub fn pill(ui: &mut Ui, text: &str, bg: Color32, fg: Color32) -> Response {
    let font = theme::bold(12.5);
    let g = ui.painter().layout_no_wrap(text.to_owned(), font, fg);
    let (rect, resp) = ui.allocate_exact_size(g.size() + vec2(18.0, 8.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(255), bg);
    ui.painter().galley(rect.min + vec2(9.0, 4.0), g, fg);
    resp
}

/// Barre de progression orange avec texte centré (`frac` de 0 à 1).
pub fn progress(ui: &mut Ui, pal: &Palette, frac: f32, text: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(8), pal.orange_soft);
    let f = frac.clamp(0.0, 1.0);
    let w = if f > 0.0 { (rect.width() * f).max(16.0) } else { 0.0 };
    let done = Rect::from_min_size(rect.min, vec2(w, rect.height()));
    if w > 0.0 {
        p.rect_filled(done, CornerRadius::same(8), pal.orange);
    }
    // texte lisible des deux côtés de la limite : clair sur l'orange, foncé sur le fond
    let rest = Rect::from_min_max(pos2(done.right(), rect.top()), rect.max);
    let font = theme::bold(13.0);
    p.with_clip_rect(rest).text(rect.center(), Align2::CENTER_CENTER, text, font.clone(), pal.text);
    p.with_clip_rect(done).text(rect.center(), Align2::CENTER_CENTER, text, font, pal.on_orange);
}

/// Ligne « icône d'état + texte » (vérifications).
pub fn status_line(ui: &mut Ui, pal: &Palette, icon: Icon, color: Color32, text: &str) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let (r, _) = ui.allocate_exact_size(vec2(16.0, 18.0), Sense::hover());
        icons::paint(ui.painter(), Rect::from_center_size(r.center(), Vec2::splat(15.0)), icon, color);
        ui.add(egui::Label::new(RichText::new(text).size(13.5).color(pal.text)).wrap());
    });
}

/// Petite étiquette atténuée au-dessus d'une valeur.
pub fn caption(ui: &mut Ui, pal: &Palette, text: &str) {
    ui.label(RichText::new(text).size(12.0).color(pal.weak));
}
