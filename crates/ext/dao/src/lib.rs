//! Extension `dao` : dessin 2D, greffé sur CadCraft.
//!
//! CadCraft est utilisé tel quel, en dépendance git épinglée (voir `UPSTREAM.md`) : seul son
//! crate `cadcraft-dxf` (lecture des fichiers DXF en groupes de codes) est lié, car les crates
//! graphiques de CadCraft et de SolidCraft ne peuvent pas cohabiter dans le même exécutable.
//! Cet adaptateur transforme les entités DXF en contours MakerCraft (polylignes en mm),
//! regroupés par calque, que la vue 2D sait afficher.
//!
//! Entités lues : LINE, LWPOLYLINE (avec arrondis « bulge »), POLYLINE/VERTEX, CIRCLE, ARC,
//! POINT. Les autres (textes, cotes, splines, blocs) sont comptées et signalées, pas dessinées.

pub mod shapes;
pub mod sketch;
pub use sketch::{LayerInfo, Sketch};

use std::collections::BTreeMap;
use std::f64::consts::TAU;
use std::path::Path;

use cadcraft_dxf::Tag;
use makercraft_ext_api::{CommandSpec, ExtError, Extension, Registry, Value, json};
use makercraft_geom::{Bounds2, Vec2};

/// Version de CadCraft greffée (doit suivre le tag de `Cargo.toml`).
pub const CADCRAFT_VERSION: &str = "0.20.1";

#[derive(Debug, thiserror::Error)]
pub enum DaoError {
    #[error("{0} : {1}")]
    Io(String, std::io::Error),
    #[error("DXF illisible : {0}")]
    Dxf(#[from] cadcraft_dxf::DxfError),
    #[error("aucune section ENTITIES dans ce DXF")]
    NoEntities,
}

/// Un tracé du dessin.
#[derive(Debug, Clone, PartialEq)]
pub struct Path2 {
    pub layer: String,
    pub points: Vec<Vec2>,
    pub closed: bool,
}

/// Dessin 2D lu dans un fichier.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Drawing {
    pub paths: Vec<Path2>,
    /// Calques dans l'ordre d'apparition.
    pub layers: Vec<String>,
    /// Entités non dessinées, par type.
    pub skipped: BTreeMap<String, usize>,
    /// Unité déclarée par le fichier ($INSUNITS), en mm par unité ; 1 si absente.
    pub mm_per_unit: f64,
}

impl Drawing {
    pub fn bounds(&self) -> Bounds2 {
        self.paths.iter().flat_map(|p| p.points.iter().copied()).collect()
    }
}

/// Pas angulaire maximal pour découper les arcs (rad) et écart de corde visé (mm).
const MAX_STEP: f64 = TAU / 72.0;
const CHORD: f64 = 0.05;

/// Points d'un arc de centre `c`, rayon `r`, depuis l'angle `a0` sur `sweep` radians (signé).
fn arc_points(c: Vec2, r: f64, a0: f64, sweep: f64) -> Vec<Vec2> {
    let by_chord = if r > CHORD { 2.0 * (1.0 - CHORD / r).clamp(-1.0, 1.0).acos() } else { MAX_STEP };
    let step = by_chord.clamp(1e-3, MAX_STEP);
    let n = ((sweep.abs() / step).ceil() as usize).clamp(1, 4096);
    (0..=n)
        .map(|i| {
            let a = a0 + sweep * i as f64 / n as f64;
            Vec2::new(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect()
}

/// Points d'un segment de polyligne avec arrondi (bulge = tan(angle/4), positif = sens
/// trigonométrique), sans le point de départ.
fn bulge_points(a: Vec2, b: Vec2, bulge: f64) -> Vec<Vec2> {
    let chord = b - a;
    let d = chord.length();
    if bulge.abs() < 1e-9 || d < 1e-12 {
        return vec![b];
    }
    let sweep = 4.0 * bulge.atan();
    let r = d / (2.0 * (sweep / 2.0).sin().abs());
    // centre : milieu de la corde, décalé à gauche de d (1 - b²) / (4 b)
    let left = Vec2::new(-chord.y / d, chord.x / d);
    let c = (a + b) * 0.5 + left * (d * (1.0 - bulge * bulge) / (4.0 * bulge));
    let a0 = (a.y - c.y).atan2(a.x - c.x);
    let mut pts = arc_points(c, r, a0, sweep);
    if !pts.is_empty() {
        pts.remove(0);
    }
    if let Some(last) = pts.last_mut() {
        *last = b;
    }
    pts
}

fn get(tags: &[Tag], code: i32) -> Option<&Tag> {
    tags.iter().find(|t| t.code == code)
}

fn num(tags: &[Tag], code: i32) -> f64 {
    get(tags, code).map(Tag::f64).unwrap_or(0.0)
}

fn layer_of(tags: &[Tag]) -> String {
    get(tags, 8).map(|t| t.str().trim().to_owned()).filter(|s| !s.is_empty()).unwrap_or_else(|| "0".to_owned())
}

/// Facteur mm par unité selon $INSUNITS (0 = sans unité : on suppose le mm).
fn insunits_to_mm(code: i64) -> f64 {
    match code {
        1 => 25.4,
        2 => 304.8,
        5 => 10.0,
        6 => 1000.0,
        _ => 1.0,
    }
}

fn expand(verts: &[(Vec2, f64)], closed: bool) -> Vec<Vec2> {
    let Some(&(first, _)) = verts.first() else { return Vec::new() };
    let mut out = vec![first];
    for w in verts.windows(2) {
        out.extend(bulge_points(w[0].0, w[1].0, w[0].1));
    }
    if closed
        && let Some(&(last, b)) = verts.last()
        && b.abs() > 1e-9
    {
        out.extend(bulge_points(last, first, b));
        out.pop();
    }
    out
}

fn add(d: &mut Drawing, layer: String, points: Vec<Vec2>, closed: bool) {
    if points.is_empty() {
        return;
    }
    if !d.layers.contains(&layer) {
        d.layers.push(layer.clone());
    }
    d.paths.push(Path2 { layer, points, closed });
}

/// POLYLINE ancienne forme en cours de lecture (ses sommets suivent en entités VERTEX).
struct PendingPoly {
    layer: String,
    /// (point, arrondi vers le sommet suivant)
    verts: Vec<(Vec2, f64)>,
    closed: bool,
}

/// Lit un DXF (ASCII ou binaire) depuis des octets.
pub fn read_dxf_bytes(bytes: &[u8]) -> Result<Drawing, DaoError> {
    let tags = cadcraft_dxf::parse(bytes)?;
    let sections = cadcraft_dxf::sections(&tags);
    let mut d = Drawing { mm_per_unit: 1.0, ..Drawing::default() };
    if let Some(h) = sections.iter().find(|s| s.name == "HEADER") {
        let mut it = h.tags.iter();
        while let Some(t) = it.next() {
            if t.code == 9 && t.str().trim() == "$INSUNITS" {
                if let Some(v) = it.next() {
                    d.mm_per_unit = insunits_to_mm(v.i64());
                }
                break;
            }
        }
    }
    let ents = sections.iter().find(|s| s.name == "ENTITIES").ok_or(DaoError::NoEntities)?;
    let k = d.mm_per_unit;
    let p = |x: f64, y: f64| Vec2::new(x * k, y * k);
    let mut poly: Option<PendingPoly> = None;
    for (kind, tags) in cadcraft_dxf::records(&ents.tags) {
        match kind.trim() {
            "LINE" => {
                let pts = vec![p(num(&tags, 10), num(&tags, 20)), p(num(&tags, 11), num(&tags, 21))];
                add(&mut d, layer_of(&tags), pts, false);
            }
            "CIRCLE" => {
                let mut pts = arc_points(p(num(&tags, 10), num(&tags, 20)), num(&tags, 40) * k, 0.0, TAU);
                pts.pop();
                add(&mut d, layer_of(&tags), pts, true);
            }
            "ARC" => {
                let (a0, a1) = (num(&tags, 50).to_radians(), num(&tags, 51).to_radians());
                let mut sweep = a1 - a0;
                if sweep <= 0.0 {
                    sweep += TAU;
                }
                let pts = arc_points(p(num(&tags, 10), num(&tags, 20)), num(&tags, 40) * k, a0, sweep);
                add(&mut d, layer_of(&tags), pts, false);
            }
            "POINT" => add(&mut d, layer_of(&tags), vec![p(num(&tags, 10), num(&tags, 20))], false),
            "LWPOLYLINE" => {
                let closed = get(&tags, 70).is_some_and(|t| t.i64() & 1 == 1);
                let mut verts: Vec<(Vec2, f64)> = Vec::new();
                let mut x = None;
                for t in &tags {
                    match t.code {
                        10 => x = Some(t.f64()),
                        20 => {
                            if let Some(xv) = x.take() {
                                verts.push((p(xv, t.f64()), 0.0));
                            }
                        }
                        42 => {
                            if let Some(last) = verts.last_mut() {
                                last.1 = t.f64();
                            }
                        }
                        _ => {}
                    }
                }
                add(&mut d, layer_of(&tags), expand(&verts, closed), closed);
            }
            "POLYLINE" => {
                let closed = get(&tags, 70).is_some_and(|t| t.i64() & 1 == 1);
                poly = Some(PendingPoly { layer: layer_of(&tags), verts: Vec::new(), closed });
            }
            "VERTEX" => {
                if let Some(pp) = poly.as_mut() {
                    pp.verts.push((p(num(&tags, 10), num(&tags, 20)), num(&tags, 42)));
                }
            }
            "SEQEND" => {
                if let Some(pp) = poly.take() {
                    add(&mut d, pp.layer, expand(&pp.verts, pp.closed), pp.closed);
                }
            }
            other => *d.skipped.entry(other.to_owned()).or_default() += 1,
        }
    }
    Ok(d)
}

/// Lit un fichier DXF.
pub fn read_dxf(path: &Path) -> Result<Drawing, DaoError> {
    let bytes = std::fs::read(path).map_err(|e| DaoError::Io(path.display().to_string(), e))?;
    read_dxf_bytes(&bytes)
}

pub struct DaoExtension;

impl Extension for DaoExtension {
    fn id(&self) -> &'static str {
        "dao"
    }

    fn label(&self) -> &'static str {
        "DAO 2D (CadCraft)"
    }

    fn register(&self, r: &mut Registry) -> Result<(), ExtError> {
        r.add_command(CommandSpec { id: "dao.info", label: "Version de CadCraft greffée", menu: "DAO", run: info })?;
        r.add_command(CommandSpec { id: "dao.dxf_info", label: "Contenu d'un fichier DXF", menu: "DAO/Fichiers", run: dxf_info })
    }
}

fn info(_: &Value) -> Result<Value, ExtError> {
    Ok(json!({ "cadcraft": CADCRAFT_VERSION, "formats": ["dxf"] }))
}

fn dxf_info(p: &Value) -> Result<Value, ExtError> {
    let path = p.get("path").and_then(Value::as_str).ok_or_else(|| ExtError::BadParams("attendu {\"path\": \"dessin.dxf\"}".into()))?;
    let d = read_dxf(Path::new(path)).map_err(|e| ExtError::Failed(e.to_string()))?;
    let b = d.bounds();
    Ok(json!({
        "paths": d.paths.len(),
        "layers": d.layers,
        "skipped": d.skipped,
        "mm_per_unit": d.mm_per_unit,
        "size_mm": if b.is_empty() { Value::Null } else { json!([b.size().x, b.size().y]) },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DXF: &str = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n\
0\nLINE\n8\nContour\n10\n0\n20\n0\n11\n100\n21\n0\n\
0\nCIRCLE\n8\nTrous\n10\n50\n20\n50\n40\n5\n\
0\nLWPOLYLINE\n8\nContour\n90\n4\n70\n1\n10\n0\n20\n0\n10\n10\n20\n0\n10\n10\n20\n10\n10\n0\n20\n10\n\
0\nTEXT\n8\nCotes\n1\nbonjour\n0\nENDSEC\n0\nEOF\n";

    fn read(text: &str) -> Drawing {
        match read_dxf_bytes(text.as_bytes()) {
            Ok(d) => d,
            Err(e) => panic!("{e}"),
        }
    }

    #[test]
    fn reads_basic_entities() {
        let d = read(DXF);
        assert_eq!(d.paths.len(), 3);
        assert_eq!(d.layers, ["Contour", "Trous"]);
        assert_eq!(d.skipped.get("TEXT"), Some(&1));
        let circle = &d.paths[1];
        assert!(circle.closed);
        assert!(circle.points.iter().all(|q| ((*q - Vec2::new(50.0, 50.0)).length() - 5.0).abs() < 1e-9));
        assert_eq!(d.paths[2].points.len(), 4);
        let b = d.bounds();
        assert!((b.size().x - 100.0).abs() < 1e-9 && (b.size().y - 55.0).abs() < 1e-9, "{:?}", b.size());
    }

    #[test]
    fn bulge_makes_a_half_circle() {
        let pts = bulge_points(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 1.0);
        assert!(pts.len() > 4);
        for q in &pts {
            assert!(((*q - Vec2::new(5.0, 0.0)).length() - 5.0).abs() < 1e-6, "{q:?}");
        }
        // bulge positif : sens trigonométrique, l'arc passe sous la corde (de (0,0) vers (10,0))
        assert!(pts.iter().any(|q| q.y < -4.9), "{pts:?}");
        assert_eq!(pts.last().copied(), Some(Vec2::new(10.0, 0.0)));
    }

    #[test]
    fn quarter_bulge_keeps_radius() {
        // quart de cercle de (10,0) à (0,10) autour de l'origine : bulge = tan(pi/8)
        let pts = bulge_points(Vec2::new(10.0, 0.0), Vec2::new(0.0, 10.0), (std::f64::consts::PI / 8.0).tan());
        for q in &pts {
            assert!((q.length() - 10.0).abs() < 1e-6, "{q:?}");
        }
    }

    #[test]
    fn inches_are_converted() {
        let d = read(&DXF.replace("70\n4\n0\nENDSEC", "70\n1\n0\nENDSEC"));
        assert!((d.mm_per_unit - 25.4).abs() < 1e-12);
        assert!((d.bounds().size().x - 2540.0).abs() < 1e-6);
    }

    #[test]
    fn commands_are_registered() {
        let mut host = makercraft_ext_api::Host::new();
        assert!(host.load(vec![Box::new(DaoExtension)]).is_ok());
        let v = host.run("dao.info", &Value::Null).ok();
        assert_eq!(v.and_then(|v| v["cadcraft"].as_str().map(str::to_owned)).as_deref(), Some(CADCRAFT_VERSION));
    }
}
