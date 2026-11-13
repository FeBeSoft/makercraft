//! Couleurs et réglages visuels de MakerCraft.
//!
//! Deux couleurs portent l'identité : le **bleu** pour l'esprit du design (dessin, sélection,
//! actions principales) et l'**orange** pour les composants (outils, trajectoires, machine).
//! Mise en page aérée : marges et espacements généreux.

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke, TextStyle, vec2};

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    /// Fond de la fenêtre.
    pub window: Color32,
    /// Barres (menus, ateliers, état).
    pub bar: Color32,
    pub bar_line: Color32,
    /// Panneaux latéraux et cartes.
    pub panel: Color32,
    pub card: Color32,
    pub card_line: Color32,
    /// Fond de la vue 2D.
    pub canvas: Color32,
    pub grid_minor: Color32,
    pub grid_major: Color32,
    pub text: Color32,
    pub weak: Color32,
    /// Bleu MakerCraft : design, sélection, actions principales.
    pub blue: Color32,
    pub blue_soft: Color32,
    pub on_blue: Color32,
    /// Orange MakerCraft : composants, outils, trajectoires, machine.
    pub orange: Color32,
    pub orange_soft: Color32,
    pub on_orange: Color32,
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

pub const LIGHT: Palette = Palette {
    dark: false,
    window: Color32::from_rgb(0xF4, 0xF6, 0xFA),
    bar: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    bar_line: Color32::from_rgb(0xDC, 0xE2, 0xEC),
    panel: Color32::from_rgb(0xFA, 0xFB, 0xFD),
    card: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    card_line: Color32::from_rgb(0xDF, 0xE5, 0xEE),
    canvas: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    grid_minor: Color32::from_rgb(0xEE, 0xF1, 0xF6),
    grid_major: Color32::from_rgb(0xD9, 0xDF, 0xEA),
    text: Color32::from_rgb(0x1B, 0x23, 0x33),
    weak: Color32::from_rgb(0x5E, 0x6A, 0x7E),
    blue: Color32::from_rgb(0x25, 0x63, 0xEB),
    blue_soft: Color32::from_rgb(0xE3, 0xEC, 0xFD),
    on_blue: Color32::WHITE,
    orange: Color32::from_rgb(0xE8, 0x7A, 0x14),
    orange_soft: Color32::from_rgb(0xFD, 0xEE, 0xDF),
    on_orange: Color32::WHITE,
    ok: Color32::from_rgb(0x1F, 0x9D, 0x55),
    warn: Color32::from_rgb(0xC9, 0x8A, 0x04),
    bad: Color32::from_rgb(0xD1, 0x34, 0x38),
    stock_fill: Color32::from_rgb(0xF6, 0xEE, 0xDF),
    stock_line: Color32::from_rgb(0xB9, 0x9E, 0x74),
    drawing: Color32::from_rgb(0x25, 0x63, 0xEB),
    rapid: Color32::from_rgb(0xA8, 0xB0, 0xBD),
};

pub const DARK: Palette = Palette {
    dark: true,
    window: Color32::from_rgb(0x12, 0x16, 0x1F),
    bar: Color32::from_rgb(0x19, 0x1F, 0x2B),
    bar_line: Color32::from_rgb(0x2A, 0x33, 0x44),
    panel: Color32::from_rgb(0x16, 0x1B, 0x26),
    card: Color32::from_rgb(0x1D, 0x24, 0x32),
    card_line: Color32::from_rgb(0x2E, 0x38, 0x4B),
    canvas: Color32::from_rgb(0x0F, 0x13, 0x1B),
    grid_minor: Color32::from_rgb(0x1A, 0x20, 0x2C),
    grid_major: Color32::from_rgb(0x27, 0x30, 0x41),
    text: Color32::from_rgb(0xE6, 0xEA, 0xF2),
    weak: Color32::from_rgb(0x95, 0xA0, 0xB4),
    blue: Color32::from_rgb(0x4F, 0x8B, 0xFF),
    blue_soft: Color32::from_rgb(0x1C, 0x2C, 0x4D),
    on_blue: Color32::WHITE,
    orange: Color32::from_rgb(0xFF, 0x9A, 0x3C),
    orange_soft: Color32::from_rgb(0x3B, 0x2A, 0x1A),
    on_orange: Color32::from_rgb(0x1A, 0x10, 0x05),
    ok: Color32::from_rgb(0x3F, 0xC2, 0x7A),
    warn: Color32::from_rgb(0xE8, 0xB3, 0x3A),
    bad: Color32::from_rgb(0xF0, 0x5D, 0x5E),
    stock_fill: Color32::from_rgb(0x2A, 0x24, 0x1B),
    stock_line: Color32::from_rgb(0x8C, 0x74, 0x52),
    drawing: Color32::from_rgb(0x6F, 0xA1, 0xFF),
    rapid: Color32::from_rgb(0x55, 0x5F, 0x70),
};

/// Applique la palette et l'espacement aéré au contexte egui.
pub fn apply(ctx: &egui::Context, pal: &Palette) {
    let (theme, mut v) = if pal.dark { (egui::Theme::Dark, egui::Visuals::dark()) } else { (egui::Theme::Light, egui::Visuals::light()) };
    v.panel_fill = pal.window;
    v.window_fill = pal.card;
    v.window_stroke = Stroke::new(1.0, pal.card_line);
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(8);
    v.override_text_color = Some(pal.text);
    v.selection.bg_fill = pal.blue;
    v.selection.stroke = Stroke::new(1.0, pal.on_blue);
    v.hyperlink_color = pal.blue;
    v.extreme_bg_color = pal.canvas;
    v.faint_bg_color = pal.panel;
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = CornerRadius::same(7);
    }
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, pal.blue);
    // cases à cocher et boutons radio bien visibles sur fond clair
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, pal.weak);
    v.widgets.inactive.bg_fill = pal.card;
    v.widgets.active.bg_fill = pal.blue;
    ctx.set_theme(theme);
    ctx.set_visuals_of(theme, v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = vec2(10.0, 9.0);
        s.spacing.button_padding = vec2(12.0, 6.0);
        s.spacing.menu_margin = Margin::same(8);
        s.spacing.window_margin = Margin::same(18);
        s.spacing.interact_size.y = 28.0;
        s.text_styles = [
            (TextStyle::Heading, FontId::new(21.0, FontFamily::Name("bold".into()))),
            (TextStyle::Body, FontId::new(14.5, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(14.5, FontFamily::Proportional)),
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

/// Polices : Segoe UI et Consolas de Windows si présentes, sinon celles d'egui.
/// La famille « bold » existe toujours (sinon egui s'arrêterait au premier texte en gras).
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
    let has_bold = add(&mut defs, "ui-bold", "segoeuib.ttf");
    let has_mono = add(&mut defs, "mono", "consola.ttf");
    if has_ui {
        defs.families.entry(prop.clone()).or_default().insert(0, "ui".into());
    }
    let mut bold_list = defs.families.get(&prop).cloned().unwrap_or_default();
    if has_bold {
        bold_list.insert(0, "ui-bold".into());
    }
    defs.families.insert(FontFamily::Name("bold".into()), bold_list);
    if has_mono {
        defs.families.entry(FontFamily::Monospace).or_default().insert(0, "mono".into());
    }
    ctx.set_fonts(defs);
}
