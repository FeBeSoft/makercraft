//! Vue de la machine dans l'onglet Pilotage : table, martyr, zéro pièce et position de l'outil,
//! en 3D (projection isométrique) ou en 2D (vue de dessus). Dessin au Painter, sans moteur 3D.

use eframe::egui::{Align2, Color32, CornerRadius, Pos2, Rect, Shape, Stroke, StrokeKind, Ui, pos2, vec2};

use super::equip::Reference;
use crate::theme::{self, Palette};

/// Ce que la vue montre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Iso,
    Top,
    Camera,
}

/// Données affichées.
pub struct Scene {
    /// Courses X, Y, Z (mm).
    pub travel: [f64; 3],
    pub spoilboard_mm: f64,
    /// Position machine de l'outil (mm).
    pub mpos: [f64; 3],
    /// Zéro pièce en coordonnées machine (mm).
    pub wco: [f64; 3],
    pub reference: Reference,
    pub connected: bool,
}

/// Position de l'outil sur la table (origine au coin avant gauche), bornée aux courses.
fn on_table(s: &Scene, p: [f64; 3]) -> [f64; 3] {
    let z = if s.reference == Reference::Homed { s.travel[2] + p[2] } else { s.spoilboard_mm + p[2] };
    [p[0].clamp(0.0, s.travel[0]), p[1].clamp(0.0, s.travel[1]), z.clamp(0.0, s.travel[2])]
}

pub fn paint(ui: &mut Ui, pal: &Palette, rect: Rect, view: View, s: &Scene) {
    let p = ui.painter_at(rect);
    p.rect_filled(rect, CornerRadius::same(10), pal.canvas);
    match view {
        View::Top => top(&p, pal, rect, s),
        View::Iso => iso(&p, pal, rect, s),
        View::Camera => {
            p.text(rect.center(), Align2::CENTER_CENTER, "Aucune caméra détectée", theme::regular(14.0), pal.weak);
        }
    }
    let note = if !s.connected {
        Some("machine déconnectée")
    } else if s.reference == Reference::None {
        Some("position machine non référencée : la vue est indicative")
    } else {
        None
    };
    if let (Some(n), true) = (note, view != View::Camera) {
        p.text(rect.left_bottom() + vec2(12.0, -10.0), Align2::LEFT_BOTTOM, n, theme::regular(12.0), pal.weak);
    }
}

fn top(p: &eframe::egui::Painter, pal: &Palette, rect: Rect, s: &Scene) {
    let area = rect.shrink(28.0);
    let k = (area.width() as f64 / s.travel[0]).min(area.height() as f64 / s.travel[1]) as f32;
    let size = vec2(s.travel[0] as f32 * k, s.travel[1] as f32 * k);
    let table = Rect::from_center_size(area.center(), size);
    let at = |x: f64, y: f64| pos2(table.left() + x as f32 * k, table.bottom() - y as f32 * k);
    p.rect_filled(table, CornerRadius::same(2), pal.stock_fill);
    // rainures de la table (espacement indicatif)
    let mut y = 45.0;
    while y < s.travel[1] {
        p.line_segment([at(0.0, y), at(s.travel[0], y)], Stroke::new(1.0, pal.grid_minor));
        y += 45.0;
    }
    p.rect_stroke(table, CornerRadius::same(2), Stroke::new(1.5, pal.stock_line), StrokeKind::Inside);
    p.text(table.right_bottom() + vec2(-4.0, 14.0), Align2::RIGHT_CENTER, "X", theme::bold(12.0), pal.weak);
    p.text(table.left_top() + vec2(-14.0, 4.0), Align2::CENTER_TOP, "Y", theme::bold(12.0), pal.weak);
    let w = on_table(s, s.wco);
    cross(p, at(w[0], w[1]), 9.0, pal.orange);
    let t = on_table(s, s.mpos);
    let c = at(t[0], t[1]);
    p.circle_filled(c, 6.0, pal.blue);
    p.circle_stroke(c, 10.0, Stroke::new(1.5, theme::alpha(pal.blue, 0.6)));
}

fn iso(p: &eframe::egui::Painter, pal: &Palette, rect: Rect, s: &Scene) {
    let (c30, s30) = (0.866_f64, 0.5_f64);
    let raw = |x: f64, y: f64, z: f64| ((x - y) * c30, -(x + y) * s30 - z);
    // cadrage : boîte des courses
    let corners = [(0.0, 0.0, 0.0), (s.travel[0], 0.0, 0.0), (0.0, s.travel[1], 0.0), (s.travel[0], s.travel[1], 0.0), (0.0, 0.0, s.travel[2]), (s.travel[0], s.travel[1], s.travel[2])];
    let pts: Vec<(f64, f64)> = corners.iter().map(|&(x, y, z)| raw(x, y, z)).collect();
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for (x, y) in &pts {
        x0 = x0.min(*x);
        x1 = x1.max(*x);
        y0 = y0.min(*y);
        y1 = y1.max(*y);
    }
    let area = rect.shrink(30.0);
    let k = (area.width() as f64 / (x1 - x0)).min(area.height() as f64 / (y1 - y0));
    let ox = area.center().x as f64 - (x0 + x1) / 2.0 * k;
    let oy = area.center().y as f64 - (y0 + y1) / 2.0 * k;
    let at = |x: f64, y: f64, z: f64| -> Pos2 {
        let (u, v) = raw(x, y, z);
        pos2((ox + u * k) as f32, (oy + v * k) as f32)
    };
    let (tx, ty) = (s.travel[0], s.travel[1]);
    let face = |z: f64| vec![at(0.0, 0.0, z), at(tx, 0.0, z), at(tx, ty, z), at(0.0, ty, z)];
    // table puis martyr
    p.add(Shape::convex_polygon(face(0.0), theme::alpha(pal.stock_line, 0.35), Stroke::new(1.0, pal.stock_line)));
    let m = s.spoilboard_mm;
    p.add(Shape::convex_polygon(vec![at(0.0, 0.0, 0.0), at(tx, 0.0, 0.0), at(tx, 0.0, m), at(0.0, 0.0, m)], theme::alpha(pal.stock_fill, 0.9), Stroke::new(1.0, pal.stock_line)));
    p.add(Shape::convex_polygon(vec![at(tx, 0.0, 0.0), at(tx, ty, 0.0), at(tx, ty, m), at(tx, 0.0, m)], theme::alpha(pal.stock_fill, 0.7), Stroke::new(1.0, pal.stock_line)));
    p.add(Shape::convex_polygon(face(m), pal.stock_fill, Stroke::new(1.2, pal.stock_line)));
    p.text(at(tx, 0.0, 0.0) + vec2(10.0, 4.0), Align2::LEFT_CENTER, "X", theme::bold(12.0), pal.weak);
    p.text(at(0.0, ty, 0.0) + vec2(-10.0, 4.0), Align2::RIGHT_CENTER, "Y", theme::bold(12.0), pal.weak);
    // zéro pièce
    let w = on_table(s, s.wco);
    cross(p, at(w[0], w[1], m), 9.0, pal.orange);
    // outil : fût de broche et pointe
    let t = on_table(s, s.mpos);
    let tip = at(t[0], t[1], t[2].max(m));
    let top_pt = at(t[0], t[1], t[2].max(m) + 40.0);
    p.line_segment([at(t[0], t[1], m), tip], Stroke::new(1.0, theme::alpha(pal.blue, 0.45)));
    p.line_segment([top_pt, tip], Stroke::new(7.0, theme::alpha(pal.blue, 0.85)));
    p.circle_filled(tip, 4.0, pal.blue);
}

fn cross(p: &eframe::egui::Painter, c: Pos2, r: f32, color: Color32) {
    let st = Stroke::new(2.0, color);
    p.line_segment([c - vec2(r, 0.0), c + vec2(r, 0.0)], st);
    p.line_segment([c - vec2(0.0, r), c + vec2(0.0, r)], st);
    p.circle_stroke(c, r * 0.55, Stroke::new(1.5, color));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene(r: Reference, mpos: [f64; 3]) -> Scene {
        Scene { travel: [300.0, 180.0, 45.0], spoilboard_mm: 8.0, mpos, wco: [0.0; 3], reference: r, connected: true }
    }

    #[test]
    fn tool_stays_on_the_table() {
        let s = scene(Reference::None, [-20.0, 500.0, 3.0]);
        assert_eq!(on_table(&s, s.mpos), [0.0, 180.0, 11.0]);
        // référencé par $H : Z machine négatif depuis le haut de la course
        let s = scene(Reference::Homed, [10.0, 10.0, -5.0]);
        assert_eq!(on_table(&s, s.mpos), [10.0, 10.0, 40.0]);
    }
}
