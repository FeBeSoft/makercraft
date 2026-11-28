//! Panneau de droite de l'étape Dessin : Propriétés de la sélection (position, taille, rayon,
//! rotation, point de référence), vue 3D et Commentaires du contour. Les mêmes champs servent
//! à la boîte des cotes ouverte par Entrée, à la manière de Galaad.

use std::collections::BTreeMap;

use eframe::egui::{self, Align2, CornerRadius, DragValue, Rect, RichText, Sense, Stroke, StrokeKind, Ui, vec2};
use makercraft_dao::Sketch;
use makercraft_dao::sketch::{cgeom, doc};
use serde_json::json;

use super::select_frame::{BBox, Xform};
use crate::theme::{self, Palette};

type WVec = cgeom::Vec2;

/// Onglets du panneau de droite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SideTab {
    #[default]
    Props,
    View3d,
    Comments,
}

impl SideTab {
    pub const ALL: [SideTab; 3] = [Self::Props, Self::View3d, Self::Comments];

    pub fn label(self) -> &'static str {
        match self {
            Self::Props => "Propriétés",
            Self::View3d => "3D",
            Self::Comments => "Commentaires",
        }
    }
}

/// Champs de la sélection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    X,
    Y,
    W,
    H,
    Radius,
    Turn,
}

/// État des champs entre deux images.
#[derive(Debug, Clone, Copy)]
pub struct PropsState {
    /// Point de référence, 0 à 8 de gauche à droite et de haut en bas (6 : bas gauche).
    pub anchor: u8,
    /// Valeur en cours de saisie (pas encore appliquée).
    edit: Option<(Field, f64)>,
}

impl Default for PropsState {
    fn default() -> Self {
        Self { anchor: 6, edit: None }
    }
}

impl PropsState {
    /// Point de référence dans la boîte `b`.
    fn point(&self, b: BBox) -> WVec {
        let (col, row) = (f64::from(self.anchor % 3), f64::from(self.anchor / 3));
        WVec::new(b[0] + (b[2] - b[0]) * col / 2.0, b[3] - (b[3] - b[1]) * row / 2.0)
    }
}

/// Champs de la sélection `sel` (objets modifiables) de boîte `b`. Renvoie un message
/// d'erreur si une modification échoue.
pub fn editor(ui: &mut Ui, pal: &Palette, st: &mut PropsState, sketch: &mut Sketch, sel: &[doc::Handle], b: Option<BBox>) -> Option<String> {
    let Some(b) = b.filter(|_| !sel.is_empty()) else {
        ui.label(RichText::new("Aucune sélection. Cliquer un objet, ou glisser un rectangle dans la vue (vers la droite : objets entièrement dedans ; vers la gauche : objets touchés).").size(12.5).color(pal.weak));
        return None;
    };
    let mut layers: Vec<String> = sel.iter().filter_map(|h| sketch.layer_of(*h)).collect();
    layers.sort();
    layers.dedup();
    ui.label(RichText::new(format!("{} objet(s) sur {}", sel.len(), layers.join(", "))).font(theme::bold(13.0)));

    let mut msg = None;
    ui.horizontal(|ui| {
        anchor_grid(ui, pal, &mut st.anchor);
        ui.label(RichText::new("Point de référence des cotes X, Y et de la mise à l'échelle.").size(12.0).color(pal.weak));
    });
    let a = st.point(b);
    let (w, h) = (b[2] - b[0], b[3] - b[1]);
    let mut todo = None;

    if let Some(v) = field(ui, st, Field::X, "X", a.x, -1e6, " mm", true) {
        todo = Some(Xform::Move(WVec::new(v - a.x, 0.0)));
    }
    if let Some(v) = field(ui, st, Field::Y, "Y", a.y, -1e6, " mm", true) {
        todo = Some(Xform::Move(WVec::new(0.0, v - a.y)));
    }
    if let Some(v) = field(ui, st, Field::W, "Largeur", w, 1e-3, " mm", w > 1e-9) {
        todo = Some(Xform::Scale { base: a, factor: v / w });
    }
    if let Some(v) = field(ui, st, Field::H, "Hauteur", h, 1e-3, " mm", h > 1e-9) {
        todo = Some(Xform::Scale { base: a, factor: v / h });
    }
    if let [one] = sel
        && let Some(r) = sketch.radius_of(*one)
        && let Some(v) = field(ui, st, Field::Radius, "Rayon", r, 1e-3, " mm", true)
        && let Err(e) = sketch.session.execute("properties.set", &json!({ "handles": [one.hex()], "radius": v }))
    {
        msg = Some(e.to_string());
    }
    if let Some(v) = field(ui, st, Field::Turn, "Tourner de", 0.0, -360.0, "°", true) {
        todo = Some(Xform::Rotate { base: a, deg: v });
    }
    if let Some(x) = todo
        && let Err(e) = x.execute(sketch, sel)
    {
        msg = Some(e);
    }
    ui.label(RichText::new("Largeur et hauteur gardent les proportions. Entrée ou sortie du champ : appliqué (annulable).").size(12.0).color(pal.weak));
    msg
}

/// Un champ numérique ; renvoie la valeur quand elle est validée.
#[allow(clippy::too_many_arguments)]
fn field(ui: &mut Ui, st: &mut PropsState, f: Field, label: &str, value: f64, min: f64, suffix: &str, enabled: bool) -> Option<f64> {
    let mut v = match st.edit {
        Some((k, e)) if k == f => e,
        _ => value,
    };
    let mut commit = None;
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(90.0, 20.0), Sense::hover());
        ui.painter().text(r.left_center(), Align2::LEFT_CENTER, label, theme::regular(13.0), ui.visuals().weak_text_color());
        let dv = DragValue::new(&mut v).range(min..=1e6).speed(0.1).max_decimals(3).suffix(suffix).update_while_editing(false);
        let resp = ui.add_enabled(enabled, dv);
        if resp.changed() {
            st.edit = Some((f, v));
        }
        if resp.drag_stopped() || (resp.changed() && !resp.dragged()) {
            st.edit = None;
            if (v - value).abs() > 1e-9 {
                commit = Some(v);
            }
        }
    });
    commit
}

/// Grille 3 x 3 du point de référence.
fn anchor_grid(ui: &mut Ui, pal: &Palette, anchor: &mut u8) {
    let (r, resp) = ui.allocate_exact_size(vec2(48.0, 48.0), Sense::click());
    let p = ui.painter();
    p.rect_stroke(r.shrink(6.0), CornerRadius::ZERO, Stroke::new(1.0, pal.line2), StrokeKind::Middle);
    let cell = |i: u8| {
        let (c, l) = (f32::from(i % 3), f32::from(i / 3));
        r.min + vec2(6.0 + c * 18.0, 6.0 + l * 18.0)
    };
    for i in 0..9 {
        let at = cell(i);
        let on = i == *anchor;
        p.rect(Rect::from_center_size(at, vec2(8.0, 8.0)), CornerRadius::same(1), if on { pal.orange } else { pal.canvas }, Stroke::new(1.2, if on { pal.orange } else { pal.weak }), StrokeKind::Middle);
    }
    if resp.clicked()
        && let Some(pos) = resp.interact_pointer_pos()
        && let Some(i) = (0..9).min_by(|a, b| cell(*a).distance(pos).total_cmp(&cell(*b).distance(pos)))
    {
        *anchor = i;
    }
    resp.on_hover_text("Point de référence");
}

/// Commentaires libres du contour `layer`, enregistrés dans le projet.
pub fn comments(ui: &mut Ui, pal: &Palette, all: &mut BTreeMap<String, String>, layer: &str) {
    ui.label(RichText::new(format!("Contour {layer}")).font(theme::bold(13.0)));
    let mut text = all.get(layer).cloned().unwrap_or_default();
    let resp = ui.add(egui::TextEdit::multiline(&mut text).desired_rows(10).desired_width(f32::INFINITY).hint_text("Notes sur ce contour : intention, cotes à respecter, outil prévu..."));
    if resp.changed() {
        if text.trim().is_empty() {
            all.remove(layer);
        } else {
            all.insert(layer.to_owned(), text);
        }
    }
    ui.label(RichText::new("Contour courant ou celui de l'objet choisi. Les commentaires s'enregistrent dans le projet .mkc.").size(12.0).color(pal.weak));
    let others: Vec<&String> = all.keys().filter(|k| k.as_str() != layer).collect();
    if !others.is_empty() {
        ui.label(RichText::new(format!("Autres contours commentés : {}", others.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "))).size(12.0).color(pal.weak));
    }
}
