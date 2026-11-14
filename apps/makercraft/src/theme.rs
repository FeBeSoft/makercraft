//! Couleurs et réglages visuels de MakerCraft.
//!
//! Thème sombre par défaut, dans l'esprit de GravéoCraft (fond presque noir, panneaux à filet
//! fin, titres de section en petites capitales). Deux couleurs portent l'identité : le **bleu**
//! pour l'esprit du design (dessin, sélection, actions principales) et l'**orange** pour les
//! composants (outils, trajectoires, machine). Le thème clair reste disponible dans les options.

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow, Stroke, TextStyle, vec2};

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    /// Fond de la fenêtre.
    pub window: Color32,
    /// Barres (haut, état).
    pub bar: Color32,
    pub bar_line: Color32,
    /// Fond secondaire (navigation des options, zones en retrait).
    pub panel: Color32,
    /// Cartes et fenêtres.
    pub card: Color32,
    pub card_line: Color32,
    /// Filet plus marqué (contours de champs, fenêtres flottantes).
    pub line2: Color32,
    /// Survol.
    pub hover: Color32,
    /// Fond des champs de saisie.
    pub field: Color32,
    /// Fond de la vue 2D.
    pub canvas: Color32,
    pub grid_minor: Color32,
    pub grid_major: Color32,
    pub text: Color32,
    pub weak: Color32,
    /// Texte le plus discret (titres de section, éléments indisponibles).
    pub dim: Color32,
    /// Bleu MakerCraft : design, sélection, actions principales.
    pub blue: Color32,
    pub blue_soft: Color32,
    pub on_blue: Color32,
    /// Orange MakerCraft : composants, outils, trajectoires, machine.
    pub orange: Color32,
    pub orange_soft: Color32,
    pub on_orange: Color32,
    /// Teintes d'accent des ateliers Laser et PCB.
    pub red: Color32,
    pub green: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub bad: Color32,
    /// Brut (matière) dans la vue 2D.
    pub stock_fill: Color32,
    pub stock_line: Color32,
    /// Dessin dans la vue 2D.
    pub drawing: Color32,
    pub rapid: Color32,
}

pub const DARK: Palette = Palette {
    dark: true,
    window: Color32::from_rgb(0x0B, 0x0D, 0x10),
    bar: Color32::from_rgb(0x0F, 0x11, 0x15),
    bar_line: Color32::from_rgb(0x22, 0x26, 0x2E),
    panel: Color32::from_rgb(0x17, 0x1A, 0x20),
    card: Color32::from_rgb(0x13, 0x16, 0x1B),
    card_line: Color32::from_rgb(0x22, 0x26, 0x2E),
    line2: Color32::from_rgb(0x2C, 0x31, 0x3A),
    hover: Color32::from_rgb(0x1C, 0x20, 0x28),
    field: Color32::from_rgb(0x0D, 0x0F, 0x13),
    canvas: Color32::from_rgb(0x0D, 0x0F, 0x13),
    grid_minor: Color32::from_rgb(0x16, 0x19, 0x1F),
    grid_major: Color32::from_rgb(0x23, 0x28, 0x31),
    text: Color32::from_rgb(0xE7, 0xE9, 0xEC),
    weak: Color32::from_rgb(0x8B, 0x91, 0x9B),
    dim: Color32::from_rgb(0x5D, 0x63, 0x6D),
    blue: Color32::from_rgb(0x5B, 0x9B, 0xFF),
    blue_soft: Color32::from_rgb(0x16, 0x22, 0x37),
    on_blue: Color32::from_rgb(0x06, 0x10, 0x22),
    orange: Color32::from_rgb(0xFF, 0x9F, 0x43),
    orange_soft: Color32::from_rgb(0x2C, 0x20, 0x13),
    on_orange: Color32::from_rgb(0x1A, 0x10, 0x00),
    red: Color32::from_rgb(0xFF, 0x7A, 0x6E),
    green: Color32::from_rgb(0x5B, 0xD6, 0x9A),
    ok: Color32::from_rgb(0x5B, 0xD6, 0x9A),
    warn: Color32::from_rgb(0xF5, 0xC4, 0x51),
    bad: Color32::from_rgb(0xFF, 0x7A, 0x6E),
    stock_fill: Color32::from_rgb(0x21, 0x1C, 0x15),
    stock_line: Color32::from_rgb(0x7A, 0x64, 0x46),
    drawing: Color32::from_rgb(0x6A, 0xA8, 0xFF),
    rapid: Color32::from_rgb(0x4A, 0x50, 0x5B),
};

pub const LIGHT: Palette = Palette {
    dark: false,
    window: Color32::from_rgb(0xF4, 0xF5, 0xF7),
    bar: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    bar_line: Color32::from_rgb(0xE3, 0xE5, 0xE9),
    panel: Color32::from_rgb(0xFA, 0xFB, 0xFC),
    card: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    card_line: Color32::from_rgb(0xE3, 0xE5, 0xE9),
    line2: Color32::from_rgb(0xD5, 0xD8, 0xDE),
    hover: Color32::from_rgb(0xEE, 0xF0, 0xF3),
    field: Color32::from_rgb(0xF4, 0xF5, 0xF7),
    canvas: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    grid_minor: Color32::from_rgb(0xEF, 0xF1, 0xF4),
    grid_major: Color32::from_rgb(0xDC, 0xE0, 0xE7),
    text: Color32::from_rgb(0x16, 0x18, 0x1D),
    weak: Color32::from_rgb(0x6B, 0x71, 0x7C),
    dim: Color32::from_rgb(0x9A, 0xA0, 0xAA),
    blue: Color32::from_rgb(0x2F, 0x6F, 0xDB),
    blue_soft: Color32::from_rgb(0xE6, 0xEE, 0xFB),
    on_blue: Color32::WHITE,
    orange: Color32::from_rgb(0xE8, 0x7A, 0x14),
    orange_soft: Color32::from_rgb(0xFC, 0xEF, 0xE1),
    on_orange: Color32::WHITE,
    red: Color32::from_rgb(0xD8, 0x44, 0x3A),
    green: Color32::from_rgb(0x14, 0x99, 0x5A),
    ok: Color32::from_rgb(0x14, 0x99, 0x5A),
    warn: Color32::from_rgb(0xB7, 0x86, 0x0B),
    bad: Color32::from_rgb(0xD8, 0x44, 0x3A),
    stock_fill: Color32::from_rgb(0xF6, 0xEE, 0xDF),
    stock_line: Color32::from_rgb(0xB9, 0x9E, 0x74),
    drawing: Color32::from_rgb(0x2F, 0x6F, 0xDB),
    rapid: Color32::from_rgb(0xA8, 0xB0, 0xBD),
};

/// Applique la palette et l'espacement aéré au contexte egui.
pub fn apply(ctx: &egui::Context, pal: &Palette) {
    let (theme, mut v) = if pal.dark { (egui::Theme::Dark, egui::Visuals::dark()) } else { (egui::Theme::Light, egui::Visuals::light()) };
    v.panel_fill = pal.window;
    v.window_fill = pal.card;
    v.window_stroke = Stroke::new(1.0, pal.line2);
    v.window_corner_radius = CornerRadius::same(10);
    v.window_shadow = Shadow { offset: [0, 14], blur: 40, spread: 0, color: Color32::from_black_alpha(if pal.dark { 150 } else { 45 }) };
    v.popup_shadow = Shadow { offset: [0, 10], blur: 28, spread: 0, color: Color32::from_black_alpha(if pal.dark { 140 } else { 40 }) };
    v.menu_corner_radius = CornerRadius::same(8);
    v.override_text_color = Some(pal.text);
    v.selection.bg_fill = alpha(pal.blue, 0.35);
    v.selection.stroke = Stroke::new(1.0, pal.blue);
    v.hyperlink_color = pal.blue;
    v.extreme_bg_color = pal.field;
    v.faint_bg_color = pal.panel;
    v.code_bg_color = pal.field;
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = CornerRadius::same(7);
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, pal.card_line);
    v.widgets.noninteractive.bg_fill = pal.card;
    v.widgets.inactive.bg_fill = pal.panel;
    v.widgets.inactive.weak_bg_fill = pal.panel;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, pal.line2);
    v.widgets.inactive.fg_stroke = Stroke::new(1.2, pal.text);
    v.widgets.hovered.bg_fill = pal.hover;
    v.widgets.hovered.weak_bg_fill = pal.hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, pal.blue);
    v.widgets.active.bg_fill = pal.blue;
    v.widgets.active.weak_bg_fill = pal.blue_soft;
    v.widgets.open.weak_bg_fill = pal.hover;
    ctx.set_theme(theme);
    ctx.set_visuals_of(theme, v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = vec2(10.0, 9.0);
        s.spacing.button_padding = vec2(12.0, 6.0);
        s.spacing.menu_margin = Margin::same(8);
        s.spacing.window_margin = Margin::same(0);
        s.spacing.interact_size.y = 28.0;
        s.text_styles = [
            (TextStyle::Heading, FontId::new(20.0, FontFamily::Name("bold".into()))),
            (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
        ]
        .into();
    });
}

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

pub fn regular(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// Couleur avec transparence (0..1).
pub fn alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a.clamp(0.0, 1.0) * 255.0) as u8)
}

/// Mélange de deux couleurs (0 = `a`, 1 = `b`).
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// Polices : Segoe UI (semi-gras pour les titres) et Consolas de Windows si présentes, sinon
/// celles d'egui. La famille « bold » existe toujours (sinon egui s'arrêterait au premier titre).
pub fn setup_fonts(ctx: &egui::Context) {
    let mut defs = FontDefinitions::default();
    let fonts_dir = std::env::var_os("WINDIR").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from).join("Fonts");
    let add = |defs: &mut FontDefinitions, key: &str, file: &str| -> bool {
        match std::fs::read(fonts_dir.join(file)) {
            Ok(bytes) => {
                defs.font_data.insert(key.to_owned(), Arc::new(FontData::from_owned(bytes)));
                true
            }
            Err(_) => false,
        }
    };
    let prop = FontFamily::Proportional;
    let has_ui = add(&mut defs, "ui", "segoeui.ttf");
    let has_semi = add(&mut defs, "ui-semibold", "seguisb.ttf");
    let has_bold = add(&mut defs, "ui-bold", "segoeuib.ttf");
    let has_mono = add(&mut defs, "mono", "consola.ttf");
    if has_ui {
        defs.families.entry(prop.clone()).or_default().insert(0, "ui".into());
    }
    let mut bold_list = defs.families.get(&prop).cloned().unwrap_or_default();
    if has_semi {
        bold_list.insert(0, "ui-semibold".into());
    } else if has_bold {
        bold_list.insert(0, "ui-bold".into());
    }
    defs.families.insert(FontFamily::Name("bold".into()), bold_list);
    if has_mono {
        defs.families.entry(FontFamily::Monospace).or_default().insert(0, "mono".into());
    }
    ctx.set_fonts(defs);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_goes_from_a_to_b() {
        let (a, b) = (Color32::from_rgb(0, 0, 0), Color32::from_rgb(200, 100, 50));
        assert_eq!(mix(a, b, 0.0), a);
        assert_eq!(mix(a, b, 1.0), b);
        assert_eq!(mix(a, b, 0.5), Color32::from_rgb(100, 50, 25));
    }
}
