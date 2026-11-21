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
    /// Déplacements rapides (tirets).
    Rapids,
    /// Un seul fichier.
    File,
    /// Chevron vers le haut (replier le ruban).
    Collapse,
    /// Chevron vers le bas (déplier).
    Expand,
    New,
    Trash,
    Line,
    Polyline,
    Rect,
    Circle,
    Arc,
    Move,
    Copy,
    Offset,
    Trim,
    Fillet,
    /// Calques (contours).
    Layers,
    Pocket,
    Drill,
    Engrave,
    /// Brut (pavé).
    Stock,
    /// Bride de serrage.
    Clamp,
    /// Cube (vue 3D).
    Cube,
    Check,
    /// Rangée de livres (bibliothèque d'outils).
    Library,
    /// Portique de fraiseuse (page Machines).
    Gantry,
    /// Chariot (liste d'achat).
    Cart,
    /// Carnet à spirale (journal).
    Journal,
    /// Livre ouvert (documentation).
    Book,
    /// Point d'interrogation cerclé (aide).
    Help,
    /// Roue crantée à 6 dents (options).
    Settings,
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
        Icon::Collapse => line(p, r, &[(0.2, 0.64), (0.5, 0.34), (0.8, 0.64)], s),
        Icon::Expand => line(p, r, &[(0.2, 0.36), (0.5, 0.66), (0.8, 0.36)], s),
        Icon::New => {
            line(p, r, &[(0.2, 0.06), (0.62, 0.06), (0.82, 0.26), (0.82, 0.94), (0.2, 0.94), (0.2, 0.06)], s);
            line(p, r, &[(0.51, 0.4), (0.51, 0.76)], s);
            line(p, r, &[(0.33, 0.58), (0.69, 0.58)], s);
        }
        Icon::Trash => {
            line(p, r, &[(0.14, 0.24), (0.86, 0.24)], s);
            line(p, r, &[(0.38, 0.24), (0.42, 0.1), (0.58, 0.1), (0.62, 0.24)], s);
            line(p, r, &[(0.24, 0.24), (0.3, 0.92), (0.7, 0.92), (0.76, 0.24)], s);
            line(p, r, &[(0.44, 0.4), (0.44, 0.78)], s);
            line(p, r, &[(0.56, 0.4), (0.56, 0.78)], s);
        }
        Icon::Line => {
            line(p, r, &[(0.15, 0.85), (0.85, 0.15)], s);
            p.circle_filled(at(r, 0.15, 0.85), w * 1.4, c);
            p.circle_filled(at(r, 0.85, 0.15), w * 1.4, c);
        }
        Icon::Polyline => {
            line(p, r, &[(0.1, 0.8), (0.35, 0.3), (0.6, 0.65), (0.9, 0.2)], s);
            for (x, y) in [(0.1, 0.8), (0.35, 0.3), (0.6, 0.65), (0.9, 0.2)] {
                p.circle_filled(at(r, x, y), w * 1.2, c);
            }
        }
        Icon::Rect => {
            p.rect_stroke(Rect::from_min_max(at(r, 0.12, 0.24), at(r, 0.88, 0.76)), CornerRadius::ZERO, s, StrokeKind::Middle);
        }
        Icon::Circle => {
            p.circle_stroke(r.center(), r.width() * 0.38, s);
            p.circle_filled(r.center(), w * 1.1, c);
        }
        Icon::Arc => {
            let pts: Vec<Pos2> = (0..=16).map(|i| { let a = std::f32::consts::PI * (1.0 + i as f32 / 16.0); at(r, 0.5 + 0.38 * a.cos(), 0.72 + 0.38 * a.sin()) }).collect();
            p.add(Shape::line(pts, s));
        }
        Icon::Move => {
            arrow(p, r, vec2(1.0, 0.0), s);
            arrow(p, r, vec2(0.0, -1.0), s);
        }
        Icon::Copy => {
            p.rect_stroke(Rect::from_min_max(at(r, 0.1, 0.3), at(r, 0.62, 0.9)), cr, s, StrokeKind::Middle);
            line(p, r, &[(0.38, 0.3), (0.38, 0.1), (0.9, 0.1), (0.9, 0.7), (0.62, 0.7)], s);
        }
        Icon::Offset => {
            p.rect_stroke(Rect::from_min_max(at(r, 0.3, 0.3), at(r, 0.7, 0.7)), CornerRadius::ZERO, s, StrokeKind::Middle);
            p.rect_stroke(Rect::from_min_max(at(r, 0.1, 0.1), at(r, 0.9, 0.9)), cr, Stroke::new(w * 0.7, c), StrokeKind::Middle);
        }
        Icon::Trim => {
            line(p, r, &[(0.5, 0.08), (0.5, 0.92)], s);
            line(p, r, &[(0.1, 0.5), (0.5, 0.5)], s);
            line(p, r, &[(0.6, 0.5), (0.9, 0.5)], Stroke::new(w * 0.6, theme_dim(c)));
        }
        Icon::Fillet => {
            line(p, r, &[(0.15, 0.9), (0.15, 0.5)], s);
            line(p, r, &[(0.5, 0.15), (0.9, 0.15)], s);
            let pts: Vec<Pos2> = (0..=10).map(|i| { let a = std::f32::consts::PI * (1.0 + 0.5 * i as f32 / 10.0); at(r, 0.5 + 0.35 * a.cos(), 0.5 + 0.35 * a.sin()) }).collect();
            p.add(Shape::line(pts, s));
        }
        Icon::Layers => {
            for dy in [0.0, 0.2, 0.4] {
                line(p, r, &[(0.5, 0.12 + dy), (0.9, 0.3 + dy), (0.5, 0.48 + dy), (0.1, 0.3 + dy), (0.5, 0.12 + dy)], s);
            }
        }
        Icon::Pocket => {
            p.rect_stroke(Rect::from_min_max(at(r, 0.1, 0.1), at(r, 0.9, 0.9)), cr, s, StrokeKind::Middle);
            for y in [0.3, 0.5, 0.7] {
                line(p, r, &[(0.24, y), (0.76, y)], Stroke::new(w * 0.8, c));
            }
        }
        Icon::Drill => {
            line(p, r, &[(0.36, 0.06), (0.36, 0.6), (0.5, 0.94), (0.64, 0.6), (0.64, 0.06)], s);
            line(p, r, &[(0.36, 0.2), (0.64, 0.34)], Stroke::new(w * 0.8, c));
            line(p, r, &[(0.36, 0.4), (0.64, 0.54)], Stroke::new(w * 0.8, c));
        }
        Icon::Engrave => {
            line(p, r, &[(0.3, 0.06), (0.5, 0.5), (0.7, 0.06)], s);
            line(p, r, &[(0.08, 0.8), (0.28, 0.66), (0.5, 0.8), (0.72, 0.66), (0.92, 0.8)], s);
        }
        Icon::Stock | Icon::Cube => {
            line(p, r, &[(0.1, 0.35), (0.5, 0.15), (0.9, 0.35), (0.9, 0.7), (0.5, 0.9), (0.1, 0.7), (0.1, 0.35)], s);
            line(p, r, &[(0.1, 0.35), (0.5, 0.55), (0.9, 0.35)], s);
            line(p, r, &[(0.5, 0.55), (0.5, 0.9)], s);
        }
        Icon::Clamp => {
            p.rect_stroke(Rect::from_min_max(at(r, 0.08, 0.3), at(r, 0.92, 0.5)), cr, s, StrokeKind::Middle);
            line(p, r, &[(0.5, 0.06), (0.5, 0.3)], Stroke::new(w * 1.3, c));
            line(p, r, &[(0.1, 0.86), (0.9, 0.86)], s);
            line(p, r, &[(0.2, 0.5), (0.2, 0.86)], s);
        }
        Icon::Check => line(p, r, &[(0.15, 0.52), (0.4, 0.78), (0.86, 0.24)], Stroke::new(w * 1.3, c)),
        Icon::Library => {
            for (x0, x1) in [(0.1, 0.26), (0.3, 0.46)] {
                p.rect_stroke(Rect::from_min_max(at(r, x0, 0.12), at(r, x1, 0.88)), CornerRadius::ZERO, s, StrokeKind::Middle);
            }
            line(p, r, &[(0.56, 0.2), (0.7, 0.16), (0.9, 0.84), (0.76, 0.88), (0.56, 0.2)], s);
            line(p, r, &[(0.05, 0.94), (0.95, 0.94)], s);
        }
        Icon::Gantry => {
            line(p, r, &[(0.18, 0.86), (0.18, 0.12), (0.82, 0.12), (0.82, 0.86)], s);
            p.rect_stroke(Rect::from_min_max(at(r, 0.4, 0.2), at(r, 0.6, 0.5)), CornerRadius::ZERO, s, StrokeKind::Middle);
            line(p, r, &[(0.5, 0.5), (0.5, 0.64)], s);
            p.rect_stroke(Rect::from_min_max(at(r, 0.06, 0.7), at(r, 0.94, 0.86)), CornerRadius::ZERO, s, StrokeKind::Middle);
        }
        Icon::Cart => {
            line(p, r, &[(0.04, 0.16), (0.2, 0.16), (0.3, 0.64), (0.84, 0.64), (0.94, 0.28), (0.24, 0.28)], s);
            p.circle_stroke(at(r, 0.36, 0.82), r.width() * 0.07, s);
            p.circle_stroke(at(r, 0.78, 0.82), r.width() * 0.07, s);
        }
        Icon::Journal => {
            p.rect_stroke(Rect::from_min_max(at(r, 0.22, 0.06), at(r, 0.84, 0.94)), cr, s, StrokeKind::Middle);
            for y in [0.2, 0.4, 0.6, 0.8] {
                line(p, r, &[(0.14, y), (0.3, y)], s);
            }
            line(p, r, &[(0.42, 0.32), (0.72, 0.32)], s);
            line(p, r, &[(0.42, 0.5), (0.72, 0.5)], s);
            line(p, r, &[(0.42, 0.68), (0.62, 0.68)], s);
        }
        Icon::Book => {
            line(p, r, &[(0.5, 0.24), (0.3, 0.14), (0.06, 0.16), (0.06, 0.82), (0.3, 0.8), (0.5, 0.9), (0.7, 0.8), (0.94, 0.82), (0.94, 0.16), (0.7, 0.14), (0.5, 0.24), (0.5, 0.9)], s);
        }
        Icon::Help => {
            p.circle_stroke(r.center(), r.width() * 0.42, s);
            line(p, r, &[(0.36, 0.38), (0.4, 0.28), (0.5, 0.24), (0.6, 0.28), (0.64, 0.38), (0.6, 0.46), (0.5, 0.52), (0.5, 0.62)], s);
            p.circle_filled(at(r, 0.5, 0.75), w * 0.9, c);
        }
        Icon::Settings => {
            let cen = r.center();
            let (ro, ri) = (r.width() * 0.44, r.width() * 0.32);
            let mut pts = Vec::new();
            for i in 0..24 {
                let a = (i as f32 + 0.5) / 24.0 * std::f32::consts::TAU;
                let rad = if (i / 2) % 2 == 0 { ro } else { ri };
                pts.push(cen + vec2(a.cos(), a.sin()) * rad);
            }
            if let Some(first) = pts.first().copied() {
                pts.push(first);
            }
            p.add(Shape::line(pts, s));
            p.circle_stroke(cen, r.width() * 0.14, s);
        }
    }
}

/// Variante atténuée d'une couleur (traits secondaires).
fn theme_dim(c: Color32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 110)
}
