//! Dessin 2D éditable, porté par le moteur de CadCraft (`cadcraft-engine`) : commandes à
//! invites comme dans CadCraft (ligne, cercle, rectangle...), accroches, sélection, annuler.
//! Le format natif est celui de CadCraft (DXF ou DWG, lus et écrits par `cadcraft-io`).
//!
//! Convention MakerCraft : un calque = un contour à usiner ; les parcours visent des calques.

use std::path::Path;

pub use cadcraft_doc as doc;
pub use cadcraft_engine as engine;
pub use cadcraft_geom as cgeom;
pub use cadcraft_render as render;

use cadcraft_engine::Session;
use makercraft_geom::{Bounds2, Vec2};

use crate::Path2;

/// Calque du dessin, vu par MakerCraft.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerInfo {
    pub name: String,
    pub rgb: [u8; 3],
    pub visible: bool,
    /// Calque verrouillé : ses objets ne se modifient pas.
    pub locked: bool,
    /// Nombre d'objets sur le calque.
    pub objects: usize,
}

/// Le dessin et sa session d'édition.
pub struct Sketch {
    pub session: Session,
}

impl Default for Sketch {
    fn default() -> Self {
        Self::new()
    }
}

impl Sketch {
    /// Dessin vide en millimètres, avec un premier calque de contour courant.
    pub fn new() -> Self {
        let mut session = Session::empty();
        session.new_drawing(true);
        let mut s = Self { session };
        s.add_layer("Contour 1");
        s.set_current_layer("Contour 1");
        s
    }

    /// Lit un DXF ou un DWG (le nom donne le format).
    pub fn from_bytes(bytes: &[u8], name: &str) -> Result<Self, String> {
        let d = cadcraft_io::read(bytes, name).map_err(|e| e.to_string())?;
        let mut session = Session::empty();
        session.open_drawing(d, name, None);
        Ok(Self { session })
    }

    pub fn open(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{} : {e}", path.display()))?;
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        Self::from_bytes(&bytes, &name)
    }

    /// Écrit le dessin au format donné par `name` (`.dxf` ou `.dwg`).
    pub fn to_bytes(&self, name: &str) -> Result<Vec<u8>, String> {
        let d = self.session.doc().map_err(|e| e.to_string())?;
        cadcraft_io::write(d, name).map_err(|e| e.to_string())
    }

    fn drawing(&self) -> Option<&doc::Drawing> {
        self.session.doc().ok()
    }

    /// Nombre d'objets du dessin.
    pub fn objects(&self) -> usize {
        self.drawing().map_or(0, |d| d.model.len())
    }

    /// Le dessin a changé depuis la dernière ouverture ou le dernier enregistrement.
    pub fn revision(&self) -> u64 {
        self.session.state().map_or(0, |s| s.revision)
    }

    pub fn layers(&self) -> Vec<LayerInfo> {
        let Some(d) = self.drawing() else { return Vec::new() };
        d.layers
            .iter()
            .map(|l| {
                let c = l.color.resolve(l.color, l.color);
                LayerInfo {
                    name: l.name.clone(),
                    rgb: [c.0, c.1, c.2],
                    visible: l.on && !l.frozen,
                    locked: l.locked,
                    objects: d.model.iter().filter(|e| e.common.layer == l.name).count(),
                }
            })
            .collect()
    }

    /// Les objets de `hs` modifiables : ni sur un calque verrouillé, ni sur un calque caché.
    pub fn editable(&self, hs: &[doc::Handle]) -> Vec<doc::Handle> {
        let Some(d) = self.drawing() else { return Vec::new() };
        let ok = |name: &str| d.layers.iter().find(|l| l.name == name).is_none_or(|l| l.on && !l.frozen && !l.locked);
        d.model.iter().filter(|e| hs.contains(&e.handle) && ok(&e.common.layer)).map(|e| e.handle).collect()
    }

    /// Objets du calque `layer`.
    pub fn objects_on(&self, layer: &str) -> Vec<doc::Handle> {
        self.drawing().map(|d| d.model.iter().filter(|e| e.common.layer == layer).map(|e| e.handle).collect()).unwrap_or_default()
    }

    /// Calque de l'objet `h`.
    pub fn layer_of(&self, h: doc::Handle) -> Option<String> {
        self.drawing()?.model.iter().find(|e| e.handle == h).map(|e| e.common.layer.clone())
    }

    /// Étire les objets `hs` de `sx` en X et `sy` en Y depuis `base` (mm), en une seule étape
    /// d'annulation. Échelle non uniforme : un cercle devient une ellipse, un arc un arc
    /// d'ellipse, les arcs des polylignes des segments fins (0,01 mm). Facteurs positifs.
    pub fn stretch(&mut self, hs: &[doc::Handle], base: cgeom::Vec2, sx: f64, sy: f64) -> Result<(), String> {
        if !(sx > 0.0 && sy > 0.0 && sx.is_finite() && sy.is_finite()) {
            return Err("facteurs d'échelle invalides".to_owned());
        }
        let selection = self.session.selection();
        let st = self.session.state_mut().map_err(|e| e.to_string())?;
        let snap = cadcraft_engine::Snapshot { label: "Étirer".into(), doc: st.doc.clone(), selection };
        let m = cgeom::Mat3::translate(base)
            .then_before(cgeom::Mat3::scale(sx, sy))
            .then_before(cgeom::Mat3::translate(cgeom::Vec2::new(-base.x, -base.y)));
        let d = std::sync::Arc::make_mut(&mut st.doc);
        for h in hs {
            d.model.modify(*h, |e| stretch_kind(&mut e.kind, &m, sx, sy));
        }
        st.push_undo(snap);
        st.revision += 1;
        Ok(())
    }

    /// Pose une polyligne fermée (forme de `crate::shapes`) sur le contour courant, par la
    /// commande `pline` de CadCraft (annulable) ; renvoie son identifiant.
    pub fn add_closed_polyline(&mut self, v: &[crate::shapes::Vertex]) -> Result<Option<doc::Handle>, String> {
        let vs: Vec<makercraft_ext_api::Value> = v.iter().map(|p| makercraft_ext_api::json!({ "p": [p.x, p.y], "bulge": p.bulge })).collect();
        let before: Vec<doc::Handle> = self.drawing().map(|d| d.model.handles()).unwrap_or_default();
        self.session.execute("pline", &makercraft_ext_api::json!({ "vertices": vs, "closed": true })).map_err(|e| e.to_string())?;
        Ok(self.drawing().and_then(|d| d.model.handles().into_iter().find(|h| !before.contains(h))))
    }

    /// Paramètres du réseau associatif `h` (None si ce n'en est pas un).
    pub fn array_of(&self, h: doc::Handle) -> Option<doc::ArrayParams> {
        let d = self.drawing()?;
        d.array_params(d.model.get(h)?)
    }

    /// La forme d'origine d'un réseau est-elle sortie pour modification (ARRAYEDIT Source) ?
    pub fn array_editing(&self) -> bool {
        self.drawing().is_some_and(|d| d.model.iter().any(|e| d.array_params(e).is_some_and(|a| a.editing.is_some())))
    }

    /// Un des objets `hs` est-il un réseau associatif ?
    pub fn has_array(&self, hs: &[doc::Handle]) -> bool {
        self.drawing().is_some_and(|d| d.model.iter().any(|e| hs.contains(&e.handle) && d.array_params(e).is_some()))
    }

    /// Avant une commande de réseau : réseaux toujours associatifs (le lien avec la forme
    /// d'origine reste, le réseau reste modifiable) et posés sur le contour de la forme choisie.
    pub fn prepare_array(&mut self) {
        let sel = self.session.selection();
        if let Some(l) = sel.first().and_then(|h| self.layer_of(*h))
            && l != self.current_layer()
        {
            self.set_current_layer(&l);
        }
        if self.drawing().is_some_and(|d| d.header.i64("ARRAYASSOCIATIVITY", 1) != 1)
            && let Ok(d) = self.session.doc_mut()
        {
            d.header.set_i64("ARRAYASSOCIATIVITY", 1);
        }
    }

    /// Rayon de l'objet `h` s'il s'agit d'un cercle ou d'un arc.
    pub fn radius_of(&self, h: doc::Handle) -> Option<f64> {
        let e = self.drawing()?.model.iter().find(|e| e.handle == h)?;
        match &e.kind {
            doc::EntityKind::Circle(c) => Some(c.radius),
            doc::EntityKind::Arc(a) => Some(a.radius),
            _ => None,
        }
    }

    pub fn current_layer(&self) -> String {
        self.drawing().map(|d| d.header.str("CLAYER", "0")).unwrap_or_default()
    }

    /// Ajoute un calque s'il n'existe pas ; renvoie vrai s'il a été créé.
    pub fn add_layer(&mut self, name: &str) -> bool {
        match self.session.doc_mut() {
            Ok(d) => {
                let created = d.ensure_layer(name);
                if created {
                    let used: Vec<cadcraft_color::Color> = d.layers.iter().map(|l| l.color).collect();
                    if let Some(l) = d.layers.last_mut() {
                        l.color = new_layer_color(&used);
                    }
                }
                created
            }
            Err(_) => false,
        }
    }

    pub fn set_current_layer(&mut self, name: &str) {
        self.add_layer(name);
        if let Ok(d) = self.session.doc_mut() {
            d.header.set_str("CLAYER", name);
        }
    }

    /// Premier nom libre « Contour n ».
    pub fn next_contour_name(&self) -> String {
        let names: Vec<String> = self.layers().into_iter().map(|l| l.name).collect();
        (1..).map(|i| format!("Contour {i}")).find(|n| !names.contains(n)).unwrap_or_else(|| "Contour".into())
    }

    /// Liste d'affichage de l'espace objet (polylignes et triangles en coordonnées monde).
    pub fn display(&self, tolerance: f64) -> render::DisplayList {
        let Some(d) = self.drawing() else { return render::DisplayList::default() };
        let opts = render::Options { tolerance, ..render::Options::default() };
        render::build(d, &doc::Space::Model, &opts)
    }

    /// Tracés d'un calque, en polylignes (arcs et courbes découpés à `tolerance` mm).
    pub fn contours(&self, layer: &str, tolerance: f64) -> Vec<Path2> {
        let Some(d) = self.drawing() else { return Vec::new() };
        let opts = render::Options { tolerance, text: false, linetypes: false, ..render::Options::default() };
        let ents = d.model.iter().filter(|e| e.common.layer == layer).map(|e| e.as_ref());
        let list = render::build_entities(d, ents, &opts);
        list.prims
            .iter()
            .filter(|p| p.kind == render::Kind::Polyline)
            .filter_map(|p| {
                let pts: Vec<Vec2> = list.verts.get(p.start as usize..(p.start + p.len) as usize)?.iter().map(|v| Vec2::new(v.x, v.y)).collect();
                if pts.len() < 2 {
                    return None;
                }
                Some(close_path(layer, pts))
            })
            .collect()
    }

    /// Tous les tracés usinables, calque par calque (sans les lignes de construction ni les cotes).
    pub fn all_contours(&self, tolerance: f64) -> Vec<Path2> {
        self.layers().iter().filter(|l| !is_reference_layer(&l.name)).flat_map(|l| self.contours(&l.name, tolerance)).collect()
    }

    /// Encombrement du dessin (vide s'il n'y a rien).
    pub fn bounds(&self) -> Bounds2 {
        self.all_contours(0.05).iter().flat_map(|p| p.points.iter().copied()).collect()
    }
}

/// Un tracé dont les extrémités se rejoignent est fermé (le dernier point est retiré).
fn close_path(layer: &str, mut pts: Vec<Vec2>) -> Path2 {
    let closed = pts.len() > 2 && pts.first().zip(pts.last()).is_some_and(|(a, b)| (*a - *b).length() < 1e-6);
    if closed {
        pts.pop();
    }
    Path2 { layer: layer.to_owned(), points: pts, closed }
}

/// Calques de repère, jamais usinés : lignes de construction et cotes.
pub const REFERENCE_LAYERS: [&str; 2] = ["Construction", "Cotes"];

pub fn is_reference_layer(name: &str) -> bool {
    REFERENCE_LAYERS.contains(&name)
}

/// Couleur d'un nouveau contour, jamais déjà prise : d'abord des couleurs bien distinctes de la
/// palette AutoCAD (bleu, rouge, vert, magenta, cyan, jaune, orange...), puis des teintes
/// réparties par l'angle d'or, en couleurs vraies.
pub fn new_layer_color(used: &[cadcraft_color::Color]) -> cadcraft_color::Color {
    const ACI: [u8; 14] = [5, 1, 3, 6, 4, 2, 30, 150, 210, 90, 170, 40, 230, 120];
    if let Some(i) = ACI.iter().find(|i| !used.contains(&cadcraft_color::Color::Index(**i))) {
        return cadcraft_color::Color::Index(*i);
    }
    let mut k = used.len() as f64;
    loop {
        let h = (k * 137.508) % 360.0;
        let c = hsv(h, 0.65, 0.95);
        if !used.contains(&c) {
            return c;
        }
        k += 1.0;
    }
}

fn hsv(h: f64, s: f64, v: f64) -> cadcraft_color::Color {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let u = |f: f64| ((f + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    cadcraft_color::Color::True(cadcraft_color::Rgb(u(r), u(g), u(b)))
}

/// Échelle non uniforme d'un objet (`m` : la même transformation en matrice).
fn stretch_kind(kind: &mut doc::EntityKind, m: &cgeom::Mat3, sx: f64, sy: f64) {
    use std::f64::consts::{FRAC_PI_2, TAU};
    let uniform = (sx - sy).abs() <= 1e-12 * sx.max(sy);
    match kind {
        doc::EntityKind::Circle(c) if !uniform => {
            let ctr = m.apply(c.center.xy());
            let (ax, ay) = (c.radius * sx, c.radius * sy);
            let major = if ax >= ay { cgeom::Vec3::new(ax, 0.0, 0.0) } else { cgeom::Vec3::new(0.0, ay, 0.0) };
            *kind = doc::EntityKind::Ellipse(doc::Ellipse { center: cgeom::Vec3::new(ctr.x, ctr.y, c.center.z), major, ratio: ax.min(ay) / ax.max(ay), start: 0.0, end: TAU });
        }
        doc::EntityKind::Arc(a) if !uniform => {
            let ctr = m.apply(a.center.xy());
            let (ax, ay) = (a.radius * sx, a.radius * sy);
            // paramètre de l'ellipse : compté depuis le grand axe
            let (major, shift) = if ax >= ay { (cgeom::Vec3::new(ax, 0.0, 0.0), 0.0) } else { (cgeom::Vec3::new(0.0, ay, 0.0), FRAC_PI_2) };
            *kind = doc::EntityKind::Ellipse(doc::Ellipse {
                center: cgeom::Vec3::new(ctr.x, ctr.y, a.center.z),
                major,
                ratio: ax.min(ay) / ax.max(ay),
                start: a.start - shift,
                end: a.end - shift,
            });
        }
        doc::EntityKind::LwPolyline(p) if !uniform && p.vertices.iter().any(|v| v.bulge.abs() > 1e-12) => {
            let pts = flatten_bulges(&p.vertices, p.closed, 0.01);
            p.vertices = pts.into_iter().map(|q| cgeom::PolyVertex { p: m.apply(q), bulge: 0.0, start_width: 0.0, end_width: 0.0 }).collect();
        }
        k => k.transform(m),
    }
}

/// Sommets d'une polyligne à arcs (renflements) remplacés par des segments à `tol` près.
pub fn flatten_bulges(v: &[cgeom::PolyVertex], closed: bool, tol: f64) -> Vec<cgeom::Vec2> {
    let n = v.len();
    let mut out = Vec::with_capacity(n * 4);
    let count = if closed { n } else { n.saturating_sub(1) };
    if let Some(first) = v.first() {
        out.push(first.p);
    }
    for i in 0..count {
        let (Some(a), Some(b)) = (v.get(i), v.get((i + 1) % n.max(1))) else { continue };
        let last = closed && i + 1 == n;
        let b2 = a.bulge;
        if b2.abs() > 1e-12 {
            let (dx, dy) = (b.p.x - a.p.x, b.p.y - a.p.y);
            let d = (dx * dx + dy * dy).sqrt();
            if d > 1e-12 {
                let theta = 4.0 * b2.atan();
                let r = d * (1.0 + b2 * b2) / (4.0 * b2.abs());
                // centre : à gauche de la corde pour un renflement positif (sens trigonométrique)
                let k = (1.0 - b2 * b2) / (4.0 * b2);
                let (cx, cy) = (a.p.x + dx / 2.0 - dy * k, a.p.y + dy / 2.0 + dx * k);
                let a0 = (a.p.y - cy).atan2(a.p.x - cx);
                let step = if r > tol { 2.0 * (1.0 - tol / r).acos() } else { theta.abs() };
                let segs = ((theta.abs() / step.max(1e-3)).ceil() as usize).clamp(2, 720);
                for s in 1..segs {
                    let t = a0 + theta * s as f64 / segs as f64;
                    out.push(cgeom::Vec2::new(cx + r * t.cos(), cy + r * t.sin()));
                }
            }
        }
        if !last {
            out.push(b.p);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn layer_colors_never_repeat() {
        let mut used = Vec::new();
        for _ in 0..60 {
            let c = super::new_layer_color(&used);
            assert!(!used.contains(&c));
            used.push(c);
        }
    }

    /// Étoile et oblong posés par `pline` : tracés fermés du contour courant.
    #[test]
    fn star_and_oblong_are_closed_contours() {
        let mut sk = Sketch::new();
        let layer = sk.current_layer();
        let Ok(star) = crate::shapes::star(5, 25.0, 10.0, (50.0, 50.0), 0.0) else { panic!("étoile") };
        let Ok(oblong) = crate::shapes::oblong(60.0, 20.0, true, (0.0, 0.0), 0.0) else { panic!("oblong") };
        for v in [star, oblong] {
            match sk.add_closed_polyline(&v) {
                Ok(Some(_)) => {}
                other => panic!("pose : {other:?}"),
            }
        }
        let c = sk.contours(&layer, 0.01);
        assert_eq!(c.len(), 2);
        assert!(c.iter().all(|p| p.closed));
        // l'oblong a ses demi-cercles : bien plus que 4 points
        assert!(c.iter().any(|p| p.points.len() > 20));
    }

    /// Le lien du réseau (forme d'origine + paramètres) survit à l'enregistrement DXF du .mkc,
    /// et le réseau reste modifiable après relecture.
    #[test]
    fn associative_array_survives_dxf_roundtrip() {
        let mut sk = Sketch::new();
        if let Err(e) = sk.session.execute("circle", &serde_json::json!({ "center": [0.0, 0.0], "radius": 5.0 })) {
            panic!("cercle : {e}");
        }
        let hs: Vec<String> = sk.session.doc().map(|d| d.model.handles()).unwrap_or_default().iter().map(|h| h.hex()).collect();
        assert_eq!(hs.len(), 1);
        let p = serde_json::json!({ "handles": hs, "rows": 2, "cols": 3, "rowSpacing": 20.0, "colSpacing": 20.0 });
        if let Err(e) = sk.session.execute("arrayrect", &p) {
            panic!("réseau : {e}");
        }
        let arrays = |s: &Sketch| -> Vec<(doc::Handle, doc::ArrayParams)> {
            let Ok(d) = s.session.doc() else { return Vec::new() };
            d.model.iter().filter_map(|e| d.array_params(e).map(|a| (e.handle, a))).collect()
        };
        let before = arrays(&sk);
        assert_eq!(before.len(), 1, "le réseau doit être associatif");
        let bytes = match sk.to_bytes("essai.dxf") {
            Ok(b) => b,
            Err(e) => panic!("écriture : {e}"),
        };
        let mut back = match Sketch::from_bytes(&bytes, "essai.dxf") {
            Ok(s) => s,
            Err(e) => panic!("relecture : {e}"),
        };
        let after = arrays(&back);
        assert_eq!(after.len(), 1, "le lien du réseau est perdu à la relecture");
        assert_eq!(after.first().map(|a| &a.1.kind), before.first().map(|a| &a.1.kind));
        // toujours modifiable : 4 colonnes
        let Some((h, _)) = after.first() else { panic!("réseau absent") };
        if let Err(e) = back.session.execute("arrayedit", &serde_json::json!({ "handle": h.hex(), "cols": 4 })) {
            panic!("arrayedit après relecture : {e}");
        }
        let edited = arrays(&back);
        assert_eq!(edited.len(), 1);
        assert_ne!(edited.first().map(|a| &a.1.kind), after.first().map(|a| &a.1.kind), "le réseau n'a pas changé");
        // les copies sont des tracés usinables du contour
        // les 8 exemplaires sont des tracés fermés du contour d'origine, donc usinés
        let all = back.all_contours(0.05);
        assert_eq!(all.len(), 8);
        assert!(all.iter().all(|c| c.closed && c.layer == "Contour 1"));
    }

    #[test]
    fn bulge_semicircle_flattened_on_the_circle() {
        let v = [
            cgeom::PolyVertex { p: cgeom::Vec2::new(0.0, 0.0), bulge: 1.0, start_width: 0.0, end_width: 0.0 },
            cgeom::PolyVertex { p: cgeom::Vec2::new(2.0, 0.0), bulge: 0.0, start_width: 0.0, end_width: 0.0 },
        ];
        let pts = flatten_bulges(&v, false, 0.01);
        assert!(pts.len() > 10);
        for q in &pts {
            let r = ((q.x - 1.0).powi(2) + q.y.powi(2)).sqrt();
            assert!((r - 1.0).abs() < 1e-9, "point hors du cercle : {q:?}");
            assert!(q.y <= 1e-9, "arc positif : sous la corde de gauche à droite");
        }
    }

    use super::*;

    #[test]
    fn new_sketch_has_a_current_contour_layer() {
        let s = Sketch::new();
        assert_eq!(s.current_layer(), "Contour 1");
        assert!(s.layers().iter().any(|l| l.name == "Contour 1"));
        assert_eq!(s.next_contour_name(), "Contour 2");
    }

    #[test]
    fn rectangle_command_gives_a_closed_contour_that_survives_dxf() {
        let mut s = Sketch::new();
        if let Err(e) = s.session.cmdline("rectang") {
            panic!("rectang : {e}");
        }
        for t in ["0,0", "40,20"] {
            if let Err(e) = s.session.cmdline(t) {
                panic!("{t} : {e}");
            }
        }
        let c = s.contours("Contour 1", 0.05);
        assert_eq!(c.len(), 1, "{c:?}");
        assert!(c[0].closed);
        assert_eq!(c[0].points.len(), 4);
        let bytes = match s.to_bytes("essai.dxf") {
            Ok(b) => b,
            Err(e) => panic!("écriture : {e}"),
        };
        let back = match Sketch::from_bytes(&bytes, "essai.dxf") {
            Ok(s) => s,
            Err(e) => panic!("relecture : {e}"),
        };
        assert_eq!(back.contours("Contour 1", 0.05).len(), 1);
    }
}
