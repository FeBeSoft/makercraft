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
                    objects: d.model.iter().filter(|e| e.common.layer == l.name).count(),
                }
            })
            .collect()
    }

    pub fn current_layer(&self) -> String {
        self.drawing().map(|d| d.header.str("CLAYER", "0")).unwrap_or_default()
    }

    /// Ajoute un calque s'il n'existe pas ; renvoie vrai s'il a été créé.
    pub fn add_layer(&mut self, name: &str) -> bool {
        // couleurs bien distinctes pour les contours : bleu, rouge, vert, magenta, cyan, jaune...
        const ACI: [u8; 8] = [5, 1, 3, 6, 4, 2, 30, 150];
        match self.session.doc_mut() {
            Ok(d) => {
                let created = d.ensure_layer(name);
                if created {
                    let n = d.layers.len();
                    if let Some(l) = d.layers.last_mut() {
                        l.color = cadcraft_color::Color::Index(ACI[n % ACI.len()]);
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

    /// Tous les tracés, calque par calque.
    pub fn all_contours(&self, tolerance: f64) -> Vec<Path2> {
        self.layers().iter().flat_map(|l| self.contours(&l.name, tolerance)).collect()
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

#[cfg(test)]
mod tests {
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
