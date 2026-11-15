//! Du projet aux trajectoires : chaque parcours prend les contours de ses calques et son outil,
//! et les opérations 2,5D de `makercraft-cam` (contour, poche, perçage, gravure) en tirent les
//! mouvements. Le travail obtenu donne le G-code, les lignes de la vue 3D et les vérifications.

use makercraft_c2d::{Contour, Tool, ToolKind};
use makercraft_cam::job::{Job, ToolGroup};
use makercraft_cam::ops::{self, Cut, Side};
use makercraft_cam::{Heights, Move, Operation, Step};
use makercraft_dao::Sketch;
use makercraft_geom::Vec2;
use makercraft_mkc::{PathKind, Project, Toolpath};

/// Finesse du découpage des arcs du dessin pour l'usinage (mm).
pub const TOLERANCE: f64 = 0.01;

fn contours_of(sketch: &Sketch, layers: &[String]) -> Vec<Contour> {
    layers.iter().flat_map(|l| sketch.contours(l, TOLERANCE)).map(|p| Contour { points: p.points, closed: p.closed }).collect()
}

fn tool_of(tp: &Toolpath, number: i64) -> Tool {
    let kind = match tp.tool.kind.as_str() {
        "ballnose" => ToolKind::Ball,
        "vbit" | "engraver" => ToolKind::VBit,
        _ => ToolKind::Flat,
    };
    Tool { number, name: tp.tool.name.clone(), kind, diameter: tp.tool.diameter, angle: tp.tool.angle }
}

/// Centre (moyenne des sommets) d'un contour fermé : point de perçage.
fn center(c: &Contour) -> Vec2 {
    let n = c.points.len().max(1) as f64;
    let s = c.points.iter().fold(Vec2::new(0.0, 0.0), |a, p| a + *p);
    Vec2::new(s.x / n, s.y / n)
}

/// Une opération pour un parcours.
pub fn operation(project: &Project, sketch: &Sketch, tp: &Toolpath) -> Operation {
    let h = Heights { safe_z: project.clearance(), clear_z: 1.0 };
    let cs = contours_of(sketch, &tp.layers);
    let refs: Vec<&Contour> = cs.iter().collect();
    let cut = Cut {
        tool_radius: tp.tool.diameter / 2.0,
        start_depth: 0.0,
        end_depth: tp.depth.abs(),
        stepdown: tp.stepdown.abs().max(0.05),
        feed: tp.feed,
        plunge: tp.plunge,
        climb: false,
    };
    let mut warnings = Vec::new();
    if tp.layers.is_empty() {
        warnings.push("aucun calque choisi".to_owned());
    } else if cs.is_empty() {
        warnings.push("les calques choisis sont vides".to_owned());
    }
    let closed: Vec<&Contour> = refs.iter().copied().filter(|c| c.closed).collect();
    let steps = match tp.kind {
        PathKind::ContourExterieur | PathKind::ContourInterieur => {
            let side = if tp.kind == PathKind::ContourExterieur { Side::Outside } else { Side::Inside };
            let items: Vec<(&Contour, Side)> = closed.iter().map(|c| (*c, side)).collect();
            if items.len() < refs.len() {
                warnings.push("contours ouverts ignorés (utiliser « Sur le trait »)".to_owned());
            }
            ops::profile(h, &items, &cut, &mut warnings)
        }
        PathKind::SurLeTrait => {
            let items: Vec<(&Contour, Side)> = refs.iter().map(|c| (*c, Side::OnLine)).collect();
            ops::profile(h, &items, &cut, &mut warnings)
        }
        PathKind::Poche => ops::pocket(h, &ops::regions(&closed), &cut, tp.stepover.max(0.05), 0.0, &mut warnings),
        PathKind::Percage => {
            let pts: Vec<Vec2> = closed.iter().map(|c| center(c)).collect();
            ops::drill(h, &pts, tp.depth.abs(), tp.stepdown.abs(), tp.plunge)
        }
        PathKind::Gravure => ops::engrave(h, &refs, &cut),
    };
    Operation { name: tp.name.clone(), kind: tp.kind.label().to_owned(), rpm: tp.rpm, steps, warnings }
}

/// Travail complet : opérations regroupées par outil, dans l'ordre des parcours.
pub fn job(project: &Project, sketch: &Sketch, title: &str) -> Job {
    let mut groups: Vec<ToolGroup> = Vec::new();
    for tp in project.toolpaths.iter().filter(|t| t.enabled) {
        let op = operation(project, sketch, tp);
        let same = |g: &&mut ToolGroup| (g.tool.diameter - tp.tool.diameter).abs() < 1e-9 && g.tool.name == tp.tool.name;
        if let Some(g) = groups.iter_mut().find(same) {
            g.ops.push(op);
        } else {
            let number = groups.len() as i64 + 1;
            groups.push(ToolGroup::new(tool_of(tp, number), vec![op]));
        }
    }
    let s = project.stock.size;
    let all: Vec<String> = sketch.layers().into_iter().map(|l| l.name).collect();
    Job {
        title: title.to_owned(),
        stock_size: (s[0], s[1], s[2]),
        machine: project.machine.name.clone(),
        heights: Heights { safe_z: project.clearance(), clear_z: 1.0 },
        groups,
        design: contours_of(sketch, &all),
        warnings: Vec::new(),
        park: makercraft_cam::Park::default(),
    }
}

/// Mouvements d'une opération en polylignes 3D : (points, rapide ?).
pub fn polylines(op: &Operation, start: [f64; 3]) -> Vec<(Vec<[f64; 3]>, bool)> {
    let mut out: Vec<(Vec<[f64; 3]>, bool)> = Vec::new();
    let mut last = start;
    for st in &op.steps {
        let Step::Move(m) = st else { continue };
        let (p, rapid) = match m {
            Move::Rapid(p) => (*p, true),
            Move::Feed(p, _) => (*p, false),
        };
        let p = [p.x, p.y, p.z];
        match out.last_mut() {
            Some((pts, r)) if *r == rapid => pts.push(p),
            _ => out.push((vec![last, p], rapid)),
        }
        last = p;
    }
    out
}

/// Position du coin avant gauche du brut dans le repère du dessin (zéro pièce).
pub fn stock_offset(project: &Project) -> (f64, f64) {
    match project.stock.xy_origin {
        makercraft_mkc::XyOrigin::AvantGauche => (0.0, 0.0),
        makercraft_mkc::XyOrigin::Centre => (-project.stock.size[0] / 2.0, -project.stock.size[1] / 2.0),
    }
}

/// Usinages qui passent dans une zone de bridage (sous le dessus de la bride) : (parcours, bride).
pub fn clamp_hits(project: &Project, job: &Job) -> Vec<(String, String)> {
    let mut hits = Vec::new();
    let off = stock_offset(project);
    for g in &job.groups {
        let r = g.tool.diameter / 2.0;
        for op in &g.ops {
            for c in &project.clamps {
                let (x0, y0) = (c.x + off.0 - r, c.y + off.1 - r);
                let (x1, y1) = (c.x + off.0 + c.w + r, c.y + off.1 + c.h + r);
                let inside = op.moves().any(|m| {
                    let p = m.target();
                    p.x > x0 && p.x < x1 && p.y > y0 && p.y < y1 && p.z < c.height
                });
                if inside {
                    hits.push((op.name.clone(), c.name.clone()));
                }
            }
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use makercraft_mkc::{Clamp, Toolpath};

    fn square_sketch() -> Sketch {
        let mut s = Sketch::new();
        for t in ["rectang", "20,20", "60,50"] {
            if let Err(e) = s.session.cmdline(t) {
                panic!("{t} : {e}");
            }
        }
        s
    }

    #[test]
    fn outside_profile_follows_the_layer() {
        let s = square_sketch();
        let mut p = Project::default();
        p.toolpaths.push(Toolpath { name: "Découpe".into(), layers: vec!["Contour 1".into()], depth: 3.0, stepdown: 1.0, ..Toolpath::default() });
        let job = job(&p, &s, "essai");
        assert_eq!(job.groups.len(), 1);
        let op = &job.groups[0].ops[0];
        assert!(op.warnings.is_empty(), "{:?}", op.warnings);
        let lowest = op.moves().map(|m| m.target().z).fold(f64::INFINITY, f64::min);
        assert!((lowest + 3.0).abs() < 1e-6, "{lowest}");
        // contour extérieur : l'outil passe hors du carré
        let min_x = op.moves().map(|m| m.target().x).fold(f64::INFINITY, f64::min);
        assert!(min_x < 20.0 - 1.0, "{min_x}");
        assert!(!polylines(op, [0.0, 0.0, 5.0]).is_empty());
        let code = makercraft_cam::gcode::write_all(&job);
        assert!(code.contains("G1"));
    }

    #[test]
    fn empty_layer_choice_is_reported_and_clamps_are_checked() {
        let s = square_sketch();
        let mut p = Project::default();
        p.toolpaths.push(Toolpath { name: "Vide".into(), ..Toolpath::default() });
        p.toolpaths.push(Toolpath { name: "Découpe".into(), layers: vec!["Contour 1".into()], ..Toolpath::default() });
        p.clamps.push(Clamp { name: "B1".into(), x: 15.0, y: 15.0, w: 10.0, h: 10.0, height: 15.0 });
        let job = job(&p, &s, "essai");
        assert!(job.groups[0].ops[0].warnings.iter().any(|w| w.contains("aucun calque")));
        let hits = clamp_hits(&p, &job);
        assert!(hits.iter().any(|(op, c)| op == "Découpe" && c == "B1"), "{hits:?}");
    }
}
