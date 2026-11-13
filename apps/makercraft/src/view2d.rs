//! Vue 2D : zoom à la molette (autour du curseur), déplacement en glissant, grille adaptative,
//! origine et axes, coordonnées du curseur. Les coordonnées du monde sont en mm, Y vers le haut
//! comme sur la machine.

use eframe::egui::{self, Align2, Color32, CornerRadius, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui, pos2, vec2};
use makercraft_geom::{Bounds2, Vec2};

use crate::prefs::Units;
use crate::theme::{self, Palette};

/// Passage monde (mm) vers écran (points egui).
#[derive(Debug, Clone, Copy)]
pub struct Xform {
    pub rect: Rect,
    /// Points par mm.
    pub scale: f32,
    /// Point du monde au centre de la vue.
    pub center: Vec2,
}

impl Xform {
    pub fn to_screen(self, p: Vec2) -> Pos2 {
        let c = self.rect.center();
        pos2(c.x + ((p.x - self.center.x) as f32) * self.scale, c.y - ((p.y - self.center.y) as f32) * self.scale)
    }

    pub fn to_world(self, p: Pos2) -> Vec2 {
        let c = self.rect.center();
        Vec2::new(self.center.x + f64::from((p.x - c.x) / self.scale), self.center.y - f64::from((p.y - c.y) / self.scale))
    }

    /// Polyligne (ou contour fermé) du monde.
    pub fn polyline(&self, painter: &Painter, pts: &[Vec2], closed: bool, stroke: Stroke) {
        if pts.len() < 2 {
            if let Some(p) = pts.first() {
                painter.circle_filled(self.to_screen(*p), stroke.width * 1.5, stroke.color);
            }
            return;
        }
        let v: Vec<Pos2> = pts.iter().map(|p| self.to_screen(*p)).collect();
        painter.add(if closed { Shape::closed_line(v, stroke) } else { Shape::line(v, stroke) });
    }
}

pub struct View2d {
    pub scale: f32,
    pub center: Vec2,
    /// Cadrage demandé (appliqué au prochain affichage, quand la taille de la vue est connue).
    pending_fit: Option<Bounds2>,
}

impl Default for View2d {
    fn default() -> Self {
        Self { scale: 3.0, center: Vec2::new(100.0, 70.0), pending_fit: Some(Bounds2 { min: Vec2::new(0.0, 0.0), max: Vec2::new(200.0, 140.0) }) }
    }
}

pub const MIN_SCALE: f32 = 0.0001;
pub const MAX_SCALE: f32 = 400.0;

/// Pas de grille (mm) : le plus petit qui laisse au moins `min_px` points entre deux lignes.
pub fn grid_step(scale: f32, min_px: f32) -> f64 {
    const STEPS: [f64; 17] =
        [0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0, 50.0, 100.0, 500.0, 1000.0, 5000.0, 10000.0, 50000.0, 100_000.0, 500_000.0, 1_000_000.0];
    STEPS.iter().copied().find(|s| (*s as f32) * scale >= min_px).unwrap_or(1_000_000.0)
}

impl View2d {
    /// Demande le cadrage de `b` (avec une marge).
    pub fn fit(&mut self, b: Bounds2) {
        if !b.is_empty() {
            self.pending_fit = Some(b);
        }
    }

    fn apply_fit(&mut self, rect: Rect) {
        let Some(b) = self.pending_fit.take() else { return };
        let size = b.size();
        let w = size.x.max(1.0) as f32;
        let h = size.y.max(1.0) as f32;
        self.scale = ((rect.width() - 80.0).max(50.0) / w).min((rect.height() - 80.0).max(50.0) / h).clamp(MIN_SCALE, MAX_SCALE);
        self.center = (b.min + b.max) * 0.5;
    }

    /// Affiche la vue dans tout l'espace disponible ; `draw` dessine le contenu.
    pub fn show(&mut self, ui: &mut Ui, pal: &Palette, units: Units, draw: impl FnOnce(&Painter, &Xform)) -> egui::Response {
        let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        self.apply_fit(rect);
        // déplacement : glisser (bouton gauche ou milieu)
        if resp.dragged() {
            let d = resp.drag_delta();
            self.center = self.center - Vec2::new(f64::from(d.x / self.scale), f64::from(-d.y / self.scale));
        }
        // zoom : molette autour du curseur
        if let Some(hover) = resp.hover_pos() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.0 {
                let before = Xform { rect, scale: self.scale, center: self.center }.to_world(hover);
                self.scale = (self.scale * (scroll / 300.0).exp()).clamp(MIN_SCALE, MAX_SCALE);
                let after = Xform { rect, scale: self.scale, center: self.center }.to_world(hover);
                self.center = self.center + (before - after);
            }
        }
        let xf = Xform { rect, scale: self.scale, center: self.center };
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, CornerRadius::same(10), pal.canvas);
        self.grid(&painter, &xf, pal);
        draw(&painter, &xf);
        self.origin(&painter, &xf, pal);
        // coordonnées du curseur et échelle
        let tag = |text: String, pos: Pos2, align: Align2| {
            let font = theme::mono(12.0);
            let g = painter.layout_no_wrap(text, font, pal.weak);
            let r = align.anchor_size(pos, g.size()).expand2(vec2(8.0, 4.0));
            painter.rect_filled(r, CornerRadius::same(6), theme::alpha(pal.card, 0.9));
            painter.galley(r.min + vec2(8.0, 4.0), g, pal.weak);
        };
        if let Some(h) = resp.hover_pos() {
            let w = xf.to_world(h);
            tag(format!("X {}   Y {}", units.fmt(w.x), units.fmt(w.y)), rect.left_bottom() + vec2(14.0, -12.0), Align2::LEFT_BOTTOM);
        }
        let step = grid_step(self.scale, 12.0);
        tag(format!("grille {}", units.fmt(step)), rect.right_bottom() + vec2(-14.0, -12.0), Align2::RIGHT_BOTTOM);
        resp
    }

    fn grid(&self, painter: &Painter, xf: &Xform, pal: &Palette) {
        let minor = grid_step(xf.scale, 12.0);
        let major = minor * 10.0;
        let a = xf.to_world(xf.rect.left_bottom());
        let b = xf.to_world(xf.rect.right_top());
        let draw = |step: f64, color: Color32| {
            let x0 = (a.x / step).floor() as i64;
            let x1 = (b.x / step).ceil() as i64;
            let y0 = (a.y / step).floor() as i64;
            let y1 = (b.y / step).ceil() as i64;
            if (x1 - x0) > 400 || (y1 - y0) > 400 {
                return;
            }
            let s = Stroke::new(1.0, color);
            for i in x0..=x1 {
                let x = xf.to_screen(Vec2::new(i as f64 * step, 0.0)).x;
                painter.vline(x, xf.rect.y_range(), s);
            }
            for j in y0..=y1 {
                let y = xf.to_screen(Vec2::new(0.0, j as f64 * step)).y;
                painter.hline(xf.rect.x_range(), y, s);
            }
        };
        draw(minor, pal.grid_minor);
        draw(major, pal.grid_major);
    }

    fn origin(&self, painter: &Painter, xf: &Xform, pal: &Palette) {
        let o = xf.to_screen(Vec2::ZERO);
        if !xf.rect.expand(40.0).contains(o) {
            return;
        }
        let len = 46.0;
        let sx = Stroke::new(2.0, pal.bad);
        let sy = Stroke::new(2.0, pal.ok);
        painter.arrow(o, vec2(len, 0.0), sx);
        painter.arrow(o, vec2(0.0, -len), sy);
        painter.circle_filled(o, 4.0, pal.text);
        painter.text(o + vec2(len + 6.0, 0.0), Align2::LEFT_CENTER, "X", theme::bold(12.0), pal.bad);
        painter.text(o + vec2(0.0, -len - 6.0), Align2::CENTER_BOTTOM, "Y", theme::bold(12.0), pal.ok);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_world_roundtrip() {
        let xf = Xform { rect: Rect::from_min_size(pos2(10.0, 20.0), vec2(400.0, 300.0)), scale: 2.5, center: Vec2::new(50.0, 30.0) };
        let p = Vec2::new(12.0, -7.5);
        let back = xf.to_world(xf.to_screen(p));
        assert!((back - p).length() < 1e-3, "{back:?}");
        // Y vers le haut : un point plus haut dans le monde est plus haut à l'écran
        assert!(xf.to_screen(Vec2::new(0.0, 10.0)).y < xf.to_screen(Vec2::new(0.0, 0.0)).y);
    }

    #[test]
    fn grid_step_grows_when_zooming_out() {
        assert_eq!(grid_step(20.0, 12.0), 1.0);
        assert_eq!(grid_step(2.0, 12.0), 10.0);
        assert_eq!(grid_step(0.1, 12.0), 500.0);
    }

    #[test]
    fn fit_centers_bounds() {
        let mut v = View2d::default();
        v.fit(Bounds2 { min: Vec2::new(0.0, 0.0), max: Vec2::new(300.0, 100.0) });
        v.apply_fit(Rect::from_min_size(pos2(0.0, 0.0), vec2(680.0, 480.0)));
        assert_eq!(v.center, Vec2::new(150.0, 50.0));
        assert!((v.scale - 2.0).abs() < 1e-6, "{}", v.scale);
    }
}
