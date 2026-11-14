//! Icônes vectorielles dessinées au Painter (pas d'emoji, pas d'image) : nettes à toutes les
//! tailles et dans les deux thèmes. Chaque icône s'inscrit dans le carré `r` donné.

use eframe::egui::{Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2, pos2, vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    /// Fraise (atelier CNC).
    Cnc,
    /// Faisceau (atelier Laser).
    Laser,
    /// Puce (atelier PCB).
    Pcb,
    /// Engrenage (atelier Machines).
    Machines,
    Open,
    Export,
    Plug,
    Unplug,
    Play,
    Pause,
    Stop,
    Up,
    Down,
    Left,
    Right,
    Home,
    Fit,
    Info,
    Ok,
    Warn,
    Error,
    /// Sablier (atelier à venir).
    Soon,
    Unlock,
    Zero,
    Save,
    Undo,
    Redo,
    /// Trajectoires d'usinage.
    Paths,
    /// Déplacements rapides (tirets).
    Rapids,
    /// Un seul fichier.
    File,
    /// Image (aperçu SVG).
    Image,
    /// Chevron vers le haut (replier le ruban).
    Collapse,
    /// Chevron vers le bas (déplier).
    Expand,
}

/// Point relatif au carré `r` : (0,0) coin haut gauche, (1,1) coin bas droit.
fn at(r: Rect, x: f32, y: f32) -> Pos2 {
    pos2(r.left() + x * r.width(), r.top() + y * r.height())
}

fn line(p: &Painter, r: Rect, pts: &[(f32, f32)], s: Stroke) {
    p.add(Shape::line(pts.iter().map(|&(x, y)| at(r, x, y)).collect(), s));
}

fn poly(p: &Painter, r: Rect, pts: &[(f32, f32)], fill: Color32) {
    p.add(Shape::convex_polygon(pts.iter().map(|&(x, y)| at(r, x, y)).collect(), fill, Stroke::NONE));
}

fn arrow(p: &Painter, r: Rect, dir: Vec2, s: Stroke) {
    let c = r.center();
    let d = dir * r.width() * 0.32;
    let n = vec2(-dir.y, dir.x) * r.width() * 0.22;
    p.line_segment([c - d, c + d], s);
    p.add(Shape::line(vec![c + d - dir * r.width() * 0.22 + n, c + d, c + d - dir * r.width() * 0.22 - n], s));
}

/// Dessine `icon` dans `r` avec la couleur `c`.
pub fn paint(p: &Painter, r: Rect, icon: Icon, c: Color32) {
    let w = (r.width() / 12.0).clamp(1.2, 3.0);
    let s = Stroke::new(w, c);
    let cr = CornerRadius::same((r.width() * 0.12) as u8);
    match icon {
        Icon::Cnc => {
            // broche + fraise en spirale + matière
            p.rect_stroke(Rect::from_min_max(at(r, 0.3, 0.05), at(r, 0.7, 0.32)), cr, s, StrokeKind::Middle);
            line(p, r, &[(0.42, 0.32), (0.42, 0.72)], s);
            line(p, r, &[(0.58, 0.32), (0.58, 0.72)], s);
            line(p, r, &[(0.42, 0.4), (0.58, 0.5), (0.42, 0.6), (0.58, 0.7)], s);
            line(p, r, &[(0.1, 0.9), (0.9, 0.9)], s);
            line(p, r, &[(0.42, 0.72), (0.5, 0.82), (0.58, 0.72)], s);
        }
        Icon::Laser => {
            p.rect_stroke(Rect::from_min_max(at(r, 0.32, 0.05), at(r, 0.68, 0.35)), cr, s, StrokeKind::Middle);
            line(p, r, &[(0.5, 0.35), (0.5, 0.82)], Stroke::new(w * 1.4, c));
            line(p, r, &[(0.3, 0.88), (0.7, 0.88)], s);
            line(p, r, &[(0.5, 0.82), (0.3, 0.7)], Stroke::new(w * 0.7, c));
            line(p, r, &[(0.5, 0.82), (0.7, 0.7)], Stroke::new(w * 0.7, c));
        }
        Icon::Pcb => {
            p.rect_stroke(Rect::from_min_max(at(r, 0.25, 0.25), at(r, 0.75, 0.75)), cr, s, StrokeKind::Middle);
            for k in [0.38, 0.5, 0.62] {
                line(p, r, &[(k, 0.08), (k, 0.25)], s);
                line(p, r, &[(k, 0.75), (k, 0.92)], s);
                line(p, r, &[(0.08, k), (0.25, k)], s);
                line(p, r, &[(0.75, k), (0.92, k)], s);
            }
            p.circle_filled(at(r, 0.5, 0.5), r.width() * 0.07, c);
        }
        Icon::Machines => {
            let cen = r.center();
            let ro = r.width() * 0.42;
            let ri = r.width() * 0.3;
            let teeth = 8;
            let mut pts = Vec::new();
            for i in 0..teeth * 4 {
                let a = i as f32 / (teeth * 4) as f32 * std::f32::consts::TAU;
                let rad = if (i / 2) % 2 == 0 { ro } else { ri };
                pts.push(cen + vec2(a.cos(), a.sin()) * rad);
            }
            pts.push(pts[0]);
            p.add(Shape::line(pts, s));
            p.circle_stroke(cen, r.width() * 0.13, s);
        }
        Icon::Open => {
            line(p, r, &[(0.08, 0.82), (0.08, 0.2), (0.38, 0.2), (0.46, 0.3), (0.84, 0.3), (0.84, 0.45)], s);
            line(p, r, &[(0.08, 0.82), (0.22, 0.45), (0.95, 0.45), (0.8, 0.82), (0.08, 0.82)], s);
        }
        Icon::Export => {
            line(p, r, &[(0.15, 0.55), (0.15, 0.9), (0.85, 0.9), (0.85, 0.55)], s);
            line(p, r, &[(0.5, 0.65), (0.5, 0.08)], s);
            line(p, r, &[(0.3, 0.28), (0.5, 0.08), (0.7, 0.28)], s);
        }
        Icon::Plug | Icon::Unplug => {
            line(p, r, &[(0.38, 0.08), (0.38, 0.28)], s);
            line(p, r, &[(0.62, 0.08), (0.62, 0.28)], s);
            p.rect_stroke(Rect::from_min_max(at(r, 0.24, 0.28), at(r, 0.76, 0.58)), cr, s, StrokeKind::Middle);
            line(p, r, &[(0.5, 0.58), (0.5, 0.72), (0.5, 0.92)], s);
            if icon == Icon::Unplug {
                line(p, r, &[(0.12, 0.12), (0.88, 0.88)], Stroke::new(w * 1.2, c));
            }
        }
        Icon::Play => poly(p, r, &[(0.28, 0.15), (0.85, 0.5), (0.28, 0.85)], c),
        Icon::Pause => {
            p.rect_filled(Rect::from_min_max(at(r, 0.24, 0.15), at(r, 0.42, 0.85)), CornerRadius::same(2), c);
            p.rect_filled(Rect::from_min_max(at(r, 0.58, 0.15), at(r, 0.76, 0.85)), CornerRadius::same(2), c);
        }
        Icon::Stop => {
            p.rect_filled(Rect::from_min_max(at(r, 0.2, 0.2), at(r, 0.8, 0.8)), CornerRadius::same(3), c);
        }
        Icon::Up => arrow(p, r, vec2(0.0, -1.0), s),
        Icon::Down => arrow(p, r, vec2(0.0, 1.0), s),
        Icon::Left => arrow(p, r, vec2(-1.0, 0.0), s),
        Icon::Right => arrow(p, r, vec2(1.0, 0.0), s),
        Icon::Home => {
            line(p, r, &[(0.1, 0.5), (0.5, 0.12), (0.9, 0.5)], s);
            line(p, r, &[(0.22, 0.42), (0.22, 0.88), (0.78, 0.88), (0.78, 0.42)], s);
            line(p, r, &[(0.42, 0.88), (0.42, 0.62), (0.58, 0.62), (0.58, 0.88)], s);
        }
        Icon::Fit => {
            for (a, b, d) in [((0.1, 0.35), (0.1, 0.1), (0.35, 0.1)), ((0.65, 0.1), (0.9, 0.1), (0.9, 0.35)), ((0.9, 0.65), (0.9, 0.9), (0.65, 0.9)), ((0.35, 0.9), (0.1, 0.9), (0.1, 0.65))] {
                line(p, r, &[a, b, d], s);
            }
            p.rect_stroke(Rect::from_min_max(at(r, 0.32, 0.32), at(r, 0.68, 0.68)), CornerRadius::same(1), s, StrokeKind::Middle);
        }
        Icon::Info => {
            p.circle_stroke(r.center(), r.width() * 0.42, s);
            p.circle_filled(at(r, 0.5, 0.3), w * 0.9, c);
            line(p, r, &[(0.5, 0.45), (0.5, 0.75)], s);
        }
        Icon::Ok => {
            p.circle_filled(r.center(), r.width() * 0.45, c);
            line(p, r, &[(0.28, 0.52), (0.44, 0.68), (0.73, 0.36)], Stroke::new(w * 1.2, Color32::WHITE));
        }
        Icon::Warn => {
            poly(p, r, &[(0.5, 0.06), (0.96, 0.9), (0.04, 0.9)], c);
            line(p, r, &[(0.5, 0.36), (0.5, 0.62)], Stroke::new(w * 1.2, Color32::WHITE));
            p.circle_filled(at(r, 0.5, 0.75), w * 0.8, Color32::WHITE);
        }
        Icon::Error => {
            p.circle_filled(r.center(), r.width() * 0.45, c);
            let ws = Stroke::new(w * 1.2, Color32::WHITE);
            line(p, r, &[(0.33, 0.33), (0.67, 0.67)], ws);
            line(p, r, &[(0.67, 0.33), (0.33, 0.67)], ws);
        }
        Icon::Soon => {
            line(p, r, &[(0.22, 0.08), (0.78, 0.08)], s);
            line(p, r, &[(0.22, 0.92), (0.78, 0.92)], s);
            line(p, r, &[(0.28, 0.08), (0.28, 0.22), (0.72, 0.78), (0.72, 0.92)], s);
            line(p, r, &[(0.72, 0.08), (0.72, 0.22), (0.28, 0.78), (0.28, 0.92)], s);
            poly(p, r, &[(0.36, 0.86), (0.5, 0.7), (0.64, 0.86)], c);
        }
        Icon::Unlock => {
            p.rect_stroke(Rect::from_min_max(at(r, 0.2, 0.45), at(r, 0.8, 0.9)), cr, s, StrokeKind::Middle);
            line(p, r, &[(0.32, 0.45), (0.32, 0.28), (0.4, 0.14), (0.6, 0.14), (0.68, 0.28)], s);
        }
        Icon::Zero => {
            p.circle_stroke(r.center(), r.width() * 0.3, s);
            line(p, r, &[(0.5, 0.04), (0.5, 0.96)], Stroke::new(w * 0.7, c));
            line(p, r, &[(0.04, 0.5), (0.96, 0.5)], Stroke::new(w * 0.7, c));
        }
        Icon::Save => {
            line(p, r, &[(0.12, 0.1), (0.72, 0.1), (0.9, 0.28), (0.9, 0.9), (0.12, 0.9), (0.12, 0.1)], s);
            line(p, r, &[(0.3, 0.1), (0.3, 0.34), (0.66, 0.34), (0.66, 0.1)], s);
            p.rect_stroke(Rect::from_min_max(at(r, 0.28, 0.56), at(r, 0.74, 0.9)), CornerRadius::same(1), s, StrokeKind::Middle);
        }
        Icon::Undo | Icon::Redo => {
            let f = |x: f32| if icon == Icon::Undo { x } else { 1.0 - x };
            line(p, r, &[(f(0.2), 0.42), (f(0.62), 0.42), (f(0.82), 0.52), (f(0.86), 0.68), (f(0.78), 0.82), (f(0.6), 0.88)], s);
            line(p, r, &[(f(0.38), 0.22), (f(0.18), 0.42), (f(0.38), 0.62)], s);
        }
        Icon::Paths => {
            line(p, r, &[(0.1, 0.85), (0.1, 0.35), (0.35, 0.15), (0.65, 0.15), (0.9, 0.35), (0.9, 0.85)], s);
            line(p, r, &[(0.3, 0.85), (0.3, 0.45), (0.5, 0.35), (0.7, 0.45), (0.7, 0.85)], s);
        }
        Icon::Rapids => {
            for i in 0..4 {
                let a = 0.08 + i as f32 * 0.24;
                line(p, r, &[(a, 0.8 - a * 0.6), (a + 0.14, 0.8 - (a + 0.14) * 0.6)], s);
            }
            poly(p, r, &[(0.92, 0.18), (0.66, 0.2), (0.8, 0.42)], c);
        }
        Icon::File => {
            line(p, r, &[(0.2, 0.06), (0.62, 0.06), (0.82, 0.26), (0.82, 0.94), (0.2, 0.94), (0.2, 0.06)], s);
            line(p, r, &[(0.62, 0.06), (0.62, 0.26), (0.82, 0.26)], s);
            line(p, r, &[(0.34, 0.5), (0.68, 0.5)], s);
            line(p, r, &[(0.34, 0.68), (0.68, 0.68)], s);
        }
        Icon::Image => {
            p.rect_stroke(Rect::from_min_max(at(r, 0.08, 0.16), at(r, 0.92, 0.84)), cr, s, StrokeKind::Middle);
            line(p, r, &[(0.14, 0.78), (0.4, 0.48), (0.58, 0.66), (0.7, 0.54), (0.86, 0.72)], s);
            p.circle_filled(at(r, 0.7, 0.34), w * 1.3, c);
        }
        Icon::Collapse => line(p, r, &[(0.2, 0.64), (0.5, 0.34), (0.8, 0.64)], s),
        Icon::Expand => line(p, r, &[(0.2, 0.36), (0.5, 0.66), (0.8, 0.36)], s),
    }
}
