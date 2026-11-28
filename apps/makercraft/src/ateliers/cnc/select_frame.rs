//! Cadre de sélection de la vue 2D, façon Carbide Create : un cadre pointillé autour de la
//! sélection, huit poignées d'échelle et une poignée de rotation. On glisse dans le cadre pour
//! déplacer, sur une poignée pour mettre à l'échelle (depuis le coin ou le côté opposé ; Maj :
//! depuis le centre), sur la poignée ronde pour tourner (Maj : pas de 15°). Proportions liées
//! ou non selon le réglage des Propriétés, Ctrl l'inverse le temps du geste.
//! Le geste est appliqué au relâchement par les commandes `move`, `scale` et `rotate` de CadCraft
//! (l'échelle non uniforme par `Sketch::stretch`), donc annulable comme les autres.

use eframe::egui::{self, Color32, CornerRadius, CursorIcon, Painter, Pos2, Rect, Stroke, StrokeKind, vec2};
use makercraft_dao::Sketch;
use makercraft_dao::sketch::{cgeom, doc, render};
use serde_json::json;

use super::sketch_view::Xf;

type WVec = cgeom::Vec2;

/// Rayon de capture des poignées (pixels).
const GRIP_PX: f32 = 7.0;
/// Écart de la poignée de rotation au-dessus du cadre (pixels).
const ROT_PX: f32 = 26.0;

/// Boîte englobante (xmin, ymin, xmax, ymax) en mm.
pub type BBox = [f64; 4];

/// Poignée saisie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grip {
    Move,
    /// 0 bas-gauche, 1 bas-droite, 2 haut-droite, 3 haut-gauche.
    Corner(u8),
    /// 0 bas, 1 droite, 2 haut, 3 gauche.
    Edge(u8),
    Rotate,
}

impl Grip {
    pub fn cursor(self) -> CursorIcon {
        match self {
            Self::Move => CursorIcon::Move,
            Self::Corner(0 | 2) => CursorIcon::ResizeNeSw,
            Self::Corner(_) => CursorIcon::ResizeNwSe,
            Self::Edge(0 | 2) => CursorIcon::ResizeVertical,
            Self::Edge(_) => CursorIcon::ResizeHorizontal,
            Self::Rotate => CursorIcon::Grab,
        }
    }
}

/// Transformation en cours ou à appliquer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Xform {
    Move(WVec),
    Scale { base: WVec, factor: f64 },
    /// Échelle non uniforme.
    Stretch { base: WVec, sx: f64, sy: f64 },
    Rotate { base: WVec, deg: f64 },
}

impl Xform {
    pub fn apply(self, p: WVec) -> WVec {
        match self {
            Self::Move(d) => p + d,
            Self::Scale { base, factor } => base + (p - base) * factor,
            Self::Stretch { base, sx, sy } => WVec::new(base.x + (p.x - base.x) * sx, base.y + (p.y - base.y) * sy),
            Self::Rotate { base, deg } => {
                let (s, c) = deg.to_radians().sin_cos();
                let v = p - base;
                base + WVec::new(v.x * c - v.y * s, v.x * s + v.y * c)
            }
        }
    }

    fn is_identity(self) -> bool {
        match self {
            Self::Move(d) => d.x.abs() < 1e-9 && d.y.abs() < 1e-9,
            Self::Scale { factor, .. } => (factor - 1.0).abs() < 1e-9,
            Self::Stretch { sx, sy, .. } => (sx - 1.0).abs() < 1e-9 && (sy - 1.0).abs() < 1e-9,
            Self::Rotate { deg, .. } => deg.abs() < 1e-9,
        }
    }

    /// Applique la transformation aux objets `hs` par les commandes de CadCraft.
    pub fn execute(self, sketch: &mut Sketch, hs: &[doc::Handle]) -> Result<(), String> {
        if self.is_identity() || hs.is_empty() {
            return Ok(());
        }
        let finite = match self {
            Self::Move(d) => d.x.is_finite() && d.y.is_finite(),
            Self::Scale { base, factor } => base.x.is_finite() && base.y.is_finite() && factor.is_finite() && factor > 0.0,
            Self::Stretch { base, sx, sy } => base.x.is_finite() && base.y.is_finite() && sx.is_finite() && sy.is_finite() && sx > 0.0 && sy > 0.0,
            Self::Rotate { base, deg } => base.x.is_finite() && base.y.is_finite() && deg.is_finite(),
        };
        if !finite {
            return Err("valeur hors limites : modification ignorée".to_owned());
        }
        let x = match self {
            Self::Stretch { base, sx, sy } if (sx - sy).abs() < 1e-12 => Self::Scale { base, factor: sx },
            Self::Stretch { base, sx, sy } => return sketch.stretch(hs, base, sx, sy),
            x => x,
        };
        let handles: Vec<String> = hs.iter().map(|h| h.hex()).collect();
        let (id, params) = match x {
            Self::Move(d) => ("move", json!({ "handles": handles, "delta": [d.x, d.y] })),
            Self::Scale { base, factor } => ("scale", json!({ "handles": handles, "base": [base.x, base.y], "factor": factor })),
            Self::Stretch { .. } => return Ok(()),
            Self::Rotate { base, deg } => ("rotate", json!({ "handles": handles, "base": [base.x, base.y], "angle": deg })),
        };
        sketch.session.execute(id, &params).map(|_| ()).map_err(|e| e.to_string())
    }
}

/// Glissement en cours sur le cadre.
#[derive(Debug, Clone, Copy)]
pub struct Drag {
    pub grip: Grip,
    pub start: WVec,
    pub bbox: BBox,
}

impl Drag {
    /// Transformation pour le curseur `cur` ; `shift` : depuis le centre ou pas de 15° ;
    /// `free` : proportions déliées (largeur et hauteur indépendantes).
    pub fn xform(&self, cur: WVec, shift: bool, free: bool) -> Xform {
        let b = self.bbox;
        let center = WVec::new((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
        match self.grip {
            Grip::Move => Xform::Move(cur - self.start),
            Grip::Rotate => {
                let a0 = (self.start.y - center.y).atan2(self.start.x - center.x);
                let a1 = (cur.y - center.y).atan2(cur.x - center.x);
                let mut deg = (a1 - a0).to_degrees();
                if deg > 180.0 {
                    deg -= 360.0;
                } else if deg < -180.0 {
                    deg += 360.0;
                }
                if shift {
                    deg = (deg / 15.0).round() * 15.0;
                }
                Xform::Rotate { base: center, deg }
            }
            Grip::Corner(i) => {
                let base = if shift { center } else { corner(b, (i + 2) % 4) };
                let a = self.start - base;
                let v = cur - base;
                if free {
                    let k = |num: f64, den: f64| if den.abs() > 1e-9 { (num / den).max(1e-3) } else { 1.0 };
                    return Xform::Stretch { base, sx: k(v.x, a.x), sy: k(v.y, a.y) };
                }
                let n = a.x * a.x + a.y * a.y;
                let factor = if n > 1e-12 { (v.x * a.x + v.y * a.y) / n } else { 1.0 };
                Xform::Scale { base, factor: factor.max(1e-3) }
            }
            Grip::Edge(i) => {
                let base = if shift { center } else { edge(b, (i + 2) % 4) };
                let factor = if i % 2 == 0 {
                    let d = self.start.y - base.y;
                    if d.abs() > 1e-9 { (cur.y - base.y) / d } else { 1.0 }
                } else {
                    let d = self.start.x - base.x;
                    if d.abs() > 1e-9 { (cur.x - base.x) / d } else { 1.0 }
                };
                let factor = factor.max(1e-3);
                match (free, i % 2 == 0) {
                    (false, _) => Xform::Scale { base, factor },
                    (true, true) => Xform::Stretch { base, sx: 1.0, sy: factor },
                    (true, false) => Xform::Stretch { base, sx: factor, sy: 1.0 },
                }
            }
        }
    }
}

fn corner(b: BBox, i: u8) -> WVec {
    match i {
        0 => WVec::new(b[0], b[1]),
        1 => WVec::new(b[2], b[1]),
        2 => WVec::new(b[2], b[3]),
        _ => WVec::new(b[0], b[3]),
    }
}

fn edge(b: BBox, i: u8) -> WVec {
    let (cx, cy) = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
    match i {
        0 => WVec::new(cx, b[1]),
        1 => WVec::new(b[2], cy),
        2 => WVec::new(cx, b[3]),
        _ => WVec::new(b[0], cy),
    }
}

/// Boîte englobante des objets `sel` dans la liste d'affichage.
pub fn bbox(list: &render::DisplayList, sel: &[doc::Handle]) -> Option<BBox> {
    let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
    for prim in list.prims.iter().filter(|p| sel.contains(&p.handle)) {
        let (s, n) = (prim.start as usize, prim.len as usize);
        let pts = match prim.kind {
            render::Kind::Polyline => list.verts.get(s..s + n),
            render::Kind::Point => list.verts.get(s..s + 1),
            render::Kind::Tris => list.tris.get(s..s + n),
            _ => None,
        };
        for v in pts.into_iter().flatten() {
            b = [b[0].min(v.x), b[1].min(v.y), b[2].max(v.x), b[3].max(v.y)];
        }
    }
    b[0].is_finite().then_some(b)
}

/// Poignées à l'écran : (poignée, position).
fn grips(xf: &Xf, b: BBox) -> Vec<(Grip, Pos2)> {
    let mut g = Vec::with_capacity(9);
    for i in 0..4 {
        g.push((Grip::Corner(i), xf.to_screen(corner(b, i))));
        g.push((Grip::Edge(i), xf.to_screen(edge(b, i))));
    }
    let top = xf.to_screen(edge(b, 2));
    g.push((Grip::Rotate, top - vec2(0.0, ROT_PX)));
    g
}

/// Poignée sous le pointeur `pos`, ou `Move` dans le cadre.
pub fn hit(xf: &Xf, b: BBox, pos: Pos2) -> Option<Grip> {
    if let Some((g, _)) = grips(xf, b).into_iter().find(|(_, p)| p.distance(pos) <= GRIP_PX + 1.0) {
        return Some(g);
    }
    screen_rect(xf, b).expand(2.0).contains(pos).then_some(Grip::Move)
}

fn screen_rect(xf: &Xf, b: BBox) -> Rect {
    Rect::from_two_pos(xf.to_screen(WVec::new(b[0], b[1])), xf.to_screen(WVec::new(b[2], b[3])))
}

/// Dessine le cadre ; pendant un geste, le fantôme des objets et le cadre transformés.
#[allow(clippy::too_many_arguments)]
pub fn paint(p: &Painter, xf: &Xf, b: BBox, col: Color32, fill: Color32, list: &render::DisplayList, sel: &[doc::Handle], active: Option<Xform>) {
    let dash = |pts: &[Pos2], w: f32| p.extend(egui::Shape::dashed_line(pts, Stroke::new(w, col), 5.0, 4.0));
    match active {
        Some(x) => {
            for prim in list.prims.iter().filter(|q| sel.contains(&q.handle) && q.kind == render::Kind::Polyline) {
                let (s, n) = (prim.start as usize, prim.len as usize);
                if let Some(pts) = list.verts.get(s..s + n) {
                    let pts: Vec<Pos2> = pts.iter().map(|v| xf.to_screen(x.apply(*v))).collect();
                    dash(&pts, 1.4);
                }
            }
            let c: Vec<Pos2> = (0..5).map(|i| xf.to_screen(x.apply(corner(b, i % 4)))).collect();
            dash(&c, 1.0);
            if let Xform::Rotate { deg, .. } = x {
                let at = xf.to_screen(x.apply(edge(b, 2))) - vec2(0.0, ROT_PX);
                p.text(at, egui::Align2::CENTER_BOTTOM, format!("{deg:.1}°"), crate::theme::bold(12.0), col);
            }
        }
        None => {
            let r = screen_rect(xf, b);
            let c = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
            dash(&c, 1.2);
            let top = xf.to_screen(edge(b, 2));
            for (g, at) in grips(xf, b) {
                if g == Grip::Rotate {
                    p.line_segment([top, at], Stroke::new(1.0, col));
                    p.circle(at, GRIP_PX * 0.75, fill, Stroke::new(1.4, col));
                } else {
                    let q = Rect::from_center_size(at, vec2(GRIP_PX, GRIP_PX) * 1.15);
                    p.rect(q, CornerRadius::same(1), fill, Stroke::new(1.4, col), StrokeKind::Middle);
                }
            }
        }
    }
}

/// Rectangle de sélection par glissement (de `a` à `b`, écran) : à gauche vers la droite,
/// fenêtre (objets entièrement dedans) ; vers la gauche, capture (objets touchés).
pub fn paint_window(p: &Painter, a: Pos2, b: Pos2, col: Color32) {
    let r = Rect::from_two_pos(a, b);
    let crossing = b.x < a.x;
    p.rect_filled(r, CornerRadius::ZERO, col.gamma_multiply(0.08));
    let c = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    if crossing {
        p.extend(egui::Shape::dashed_line(&c, Stroke::new(1.0, col), 5.0, 4.0));
    } else {
        p.add(egui::Shape::line(c.to_vec(), Stroke::new(1.0, col)));
    }
}

/// Objets choisis par un rectangle (monde) : fenêtre ou capture.
pub fn window_pick(list: &render::DisplayList, a: WVec, b: WVec) -> Vec<doc::Handle> {
    let crossing = b.x < a.x;
    let (x0, x1, y0, y1) = (a.x.min(b.x), a.x.max(b.x), a.y.min(b.y), a.y.max(b.y));
    let inside = |v: &WVec| v.x >= x0 && v.x <= x1 && v.y >= y0 && v.y <= y1;
    let mut all: Vec<(doc::Handle, bool, bool)> = Vec::new();
    for prim in &list.prims {
        let (s, n) = (prim.start as usize, prim.len as usize);
        let pts = match prim.kind {
            render::Kind::Polyline => list.verts.get(s..s + n),
            render::Kind::Point => list.verts.get(s..s + 1),
            _ => None,
        };
        let Some(pts) = pts else { continue };
        let any = pts.iter().any(inside) || (crossing && segments_cross(pts, x0, y0, x1, y1));
        let every = pts.iter().all(inside);
        match all.iter_mut().find(|(h, _, _)| *h == prim.handle) {
            Some(e) => {
                e.1 |= any;
                e.2 &= every;
            }
            None => all.push((prim.handle, any, every)),
        }
    }
    all.into_iter().filter(|(_, any, every)| if crossing { *any } else { *every }).map(|(h, _, _)| h).collect()
}

/// Un segment de la polyligne coupe-t-il le bord du rectangle ?
fn segments_cross(pts: &[WVec], x0: f64, y0: f64, x1: f64, y1: f64) -> bool {
    let edges = [
        (WVec::new(x0, y0), WVec::new(x1, y0)),
        (WVec::new(x1, y0), WVec::new(x1, y1)),
        (WVec::new(x1, y1), WVec::new(x0, y1)),
        (WVec::new(x0, y1), WVec::new(x0, y0)),
    ];
    pts.windows(2).any(|w| edges.iter().any(|(c, d)| seg_seg(w[0], w[1], *c, *d)))
}

fn seg_seg(a: WVec, b: WVec, c: WVec, d: WVec) -> bool {
    let cross = |o: WVec, p: WVec, q: WVec| (p.x - o.x) * (q.y - o.y) - (p.y - o.y) * (q.x - o.x);
    let (d1, d2, d3, d4) = (cross(c, d, a), cross(c, d, b), cross(a, b, c), cross(a, b, d));
    (d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: WVec, b: WVec) -> bool {
        (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
    }

    #[test]
    fn corner_scale_keeps_opposite_corner() {
        let d = Drag { grip: Grip::Corner(2), start: WVec::new(10.0, 10.0), bbox: [0.0, 0.0, 10.0, 10.0] };
        let x = d.xform(WVec::new(20.0, 20.0), false, false);
        assert_eq!(x, Xform::Scale { base: WVec::new(0.0, 0.0), factor: 2.0 });
        assert!(close(x.apply(WVec::new(10.0, 0.0)), WVec::new(20.0, 0.0)));
    }

    #[test]
    fn rotation_snaps_with_shift() {
        let d = Drag { grip: Grip::Rotate, start: WVec::new(5.0, 20.0), bbox: [0.0, 0.0, 10.0, 10.0] };
        let Xform::Rotate { deg, .. } = d.xform(WVec::new(-9.0, 6.0), true, false) else { panic!("rotation attendue") };
        assert!((deg - 90.0).abs() < 1e-9);
    }

    #[test]
    fn edge_scale_from_opposite_edge() {
        let d = Drag { grip: Grip::Edge(1), start: WVec::new(10.0, 5.0), bbox: [0.0, 0.0, 10.0, 10.0] };
        assert_eq!(d.xform(WVec::new(5.0, 0.0), false, false), Xform::Scale { base: WVec::new(0.0, 5.0), factor: 0.5 });
        // proportions déliées : un seul axe
        assert_eq!(d.xform(WVec::new(5.0, 0.0), false, true), Xform::Stretch { base: WVec::new(0.0, 5.0), sx: 0.5, sy: 1.0 });
    }

    #[test]
    fn free_corner_stretches_both_axes() {
        let d = Drag { grip: Grip::Corner(2), start: WVec::new(10.0, 10.0), bbox: [0.0, 0.0, 10.0, 10.0] };
        let x = d.xform(WVec::new(20.0, 5.0), false, true);
        assert_eq!(x, Xform::Stretch { base: WVec::new(0.0, 0.0), sx: 2.0, sy: 0.5 });
        assert!(close(x.apply(WVec::new(10.0, 10.0)), WVec::new(20.0, 5.0)));
    }
}
