//! Atelier CNC (fraisage 2,5D), de la machine à l'usinage. Les onglets du ruban suivent la
//! chronologie : Machine > Brut > Dessin > Parcours > Simulation > Usinage.
//!
//! La fenêtre : à gauche le panneau de l'étape, puis le dessin 2D (moteur de CadCraft :
//! un calque = un contour) et, à droite, la vue 3D en lecture seule (rendu SolidCraft).
//! Le projet s'enregistre en `.mkc` : dessin CadCraft (DXF ou DWG), modèle SolidCraft (.sc)
//! et données MakerCraft (machine, brut, brides, parcours).

mod panels;
mod props;
mod select_frame;
pub mod sketch_view;
pub mod toolpaths;
pub mod view3d;

use std::path::{Path, PathBuf};

use eframe::egui::{self, CornerRadius, Rect, ScrollArea, Sense, Ui, UiBuilder, pos2, vec2};
use makercraft_cam::job::Job;
use makercraft_cao::view3d::{Box3, Line3, Scene3};
use makercraft_dao::Sketch;
use makercraft_mkc::{MkcFile, PathKind, Project, Toolpath};
use makercraft_tooldb::ToolDb;

use crate::icons::Icon;
use crate::prefs::{Prefs, Units};
use crate::ribbon::{Group, Item, Tab, Tone, Variant};
use crate::theme::Palette;
use sketch_view::SketchView;
use view3d::View3d;

/// Ce que l'atelier demande à l'application.
pub enum Request {
    OpenDialog,
    Status(String),
    /// Ouvrir ce programme dans l'atelier Machines.
    SendProgram(PathBuf),
    /// Mémoriser ce fichier dans les récents.
    Recent(PathBuf),
}

/// Étapes, dans l'ordre de la conception à l'usinage (un onglet de ruban chacune).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Step {
    Machine,
    #[default]
    Brut,
    Dessin,
    Parcours,
    Simulation,
    Usinage,
}

impl Step {
    pub const ALL: [Step; 6] = [Step::Machine, Step::Brut, Step::Dessin, Step::Parcours, Step::Simulation, Step::Usinage];

    pub fn label(self) -> &'static str {
        match self {
            Self::Machine => "Machine",
            Self::Brut => "Brut",
            Self::Dessin => "Dessin",
            Self::Parcours => "Parcours",
            Self::Simulation => "Simulation",
            Self::Usinage => "Usinage",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    pub fn from_index(i: usize) -> Self {
        Self::ALL.get(i).copied().unwrap_or_default()
    }
}

/// Fichiers que l'atelier ouvre.
pub fn open_filters() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![("Projet MakerCraft", vec!["mkc"]), ("Dessin CadCraft (DXF, DWG)", vec!["dxf", "dwg"])]
}

/// Largeur du panneau de l'étape.
const PANEL_W: f32 = 312.0;

pub struct Cnc {
    pub project: Project,
    pub sketch: Sketch,
    /// Projet `.mkc` ouvert ou enregistré.
    pub path: Option<PathBuf>,
    /// Nom du dessin dans l'archive : `dessin.dxf` ou `dessin.dwg`.
    drawing_name: String,
    /// Modèle SolidCraft (.sc) conservé tel quel.
    model: Option<Vec<u8>>,
    saved: (Project, usize),
    pub step: Step,
    view2d: SketchView,
    view3d: View3d,
    /// Part du dessin 2D dans la largeur partagée avec la 3D.
    split: f32,
    pub(crate) selected_path: Option<usize>,
    pub(crate) selected_clamp: Option<usize>,
    pub(crate) show_rapids: bool,
    tooldb: Option<Result<ToolDb, String>>,
    job_cache: Option<((Project, usize), Job)>,
    pub(crate) written: Option<Result<PathBuf, String>>,
    /// Panneau de droite de l'étape Dessin : onglet affiché et champs des propriétés.
    side_tab: props::SideTab,
    props: props::PropsState,
    /// Boîte des cotes de la sélection (Entrée) ouverte.
    dims_open: bool,
    /// Contours dépliés dans l'arbre.
    pub(crate) expanded: std::collections::BTreeSet<String>,
    /// « Courant seul » actif : visibilité des contours à rétablir.
    pub(crate) solo: Option<Vec<(String, bool)>>,
    /// Marquer l'origine dans la vue 3D (Options).
    pub origin_3d: bool,
    /// Décomposer un réseau : confirmation demandée (le lien avec la forme d'origine se perd).
    explode_confirm: bool,
    /// Boîte Étoile / Oblong ouverte.
    shape: Option<ShapeDialog>,
    /// Contour en cours de renommage : (ancien nom, saisie).
    pub(crate) renaming: Option<(String, String)>,
    /// Contour à supprimer, en attente de confirmation.
    pub(crate) delete_contour: Option<String>,
    /// Contour courant à rétablir après une ligne de construction ou une cote.
    restore_layer: Option<String>,
    /// Boîte des dégagements d'angle : (T-bone, rayon de la fraise).
    relief: Option<(bool, f64)>,
}

/// Boîte des formes paramétrées (étoile, oblong).
#[derive(Debug, Clone, Copy)]
struct ShapeDialog {
    star: bool,
    branches: u32,
    outer: f64,
    inner: f64,
    length: f64,
    width: f64,
    /// Longueur hors tout (sinon entraxe des demi-cercles).
    overall: bool,
    rot: f64,
    cx: f64,
    cy: f64,
}

impl Default for Cnc {
    fn default() -> Self {
        let sketch = Sketch::new();
        let project = Project::default();
        let doc = doc_key(&sketch);
        Self {
            saved: (project.clone(), doc),
            project,
            sketch,
            path: None,
            drawing_name: "dessin.dxf".into(),
            model: None,
            step: Step::Brut,
            view2d: SketchView::default(),
            view3d: View3d::default(),
            split: 0.55,
            selected_path: None,
            selected_clamp: None,
            show_rapids: false,
            tooldb: None,
            job_cache: None,
            written: None,
            side_tab: props::SideTab::Props,
            props: props::PropsState::default(),
            dims_open: false,
            expanded: std::collections::BTreeSet::new(),
            solo: None,
            origin_3d: true,
            explode_confirm: false,
            shape: None,
            renaming: None,
            delete_contour: None,
            restore_layer: None,
            relief: None,
        }
    }
}

fn doc_key(s: &Sketch) -> usize {
    s.session.state().map_or(0, |st| std::sync::Arc::as_ptr(&st.doc) as usize)
}

impl Cnc {
    pub fn file_name(&self) -> Option<String> {
        let name = self.path.as_ref().and_then(|p| p.file_name()).map_or_else(|| "Sans titre.mkc".to_owned(), |n| n.to_string_lossy().into_owned());
        Some(if self.dirty() { format!("{name} *") } else { name })
    }

    pub fn dirty(&self) -> bool {
        self.saved.0 != self.project || self.saved.1 != doc_key(&self.sketch)
    }

    fn mark_saved(&mut self) {
        self.saved = (self.project.clone(), doc_key(&self.sketch));
    }

    /// Banque d'outils (ouverte au premier besoin).
    pub(crate) fn tooldb(&mut self) -> Result<&ToolDb, String> {
        if self.tooldb.is_none() {
            self.tooldb = Some(ToolDb::open(&Prefs::dir().join("outils")).map_err(|e| e.to_string()));
        }
        match &self.tooldb {
            Some(Ok(db)) => Ok(db),
            Some(Err(e)) => Err(e.clone()),
            None => Err("banque d'outils indisponible".into()),
        }
    }

    /// Travail d'usinage, recalculé seulement si le projet ou le dessin a changé.
    pub(crate) fn job(&mut self) -> &Job {
        let key = (self.project.clone(), doc_key(&self.sketch));
        let stale = self.job_cache.as_ref().is_none_or(|(k, _)| *k != key);
        if stale {
            let title = self.path.as_ref().and_then(|p| p.file_stem()).map_or_else(|| "Sans titre".to_owned(), |s| s.to_string_lossy().into_owned());
            let mut job = toolpaths::job(&self.project, &self.sketch, &title);
            if let Ok(db) = self.tooldb() {
                set_holders(&mut job, &key.0, db);
            }
            self.job_cache = Some((key, job));
        }
        match &self.job_cache {
            Some((_, j)) => j,
            None => unreachable_job(),
        }
    }

    pub fn new_project(&mut self) {
        *self = Self { tooldb: self.tooldb.take(), split: self.split, ..Self::default() };
    }

    /// Ouvre un projet `.mkc` ou un dessin DXF/DWG (nouveau projet autour du dessin).
    pub fn open(&mut self, path: &Path) -> Result<String, String> {
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        match ext.as_str() {
            "mkc" => {
                let f = MkcFile::read(path)?;
                let (sketch, name) = match &f.drawing {
                    Some((name, bytes)) => (Sketch::from_bytes(bytes, name)?, name.clone()),
                    None => (Sketch::new(), "dessin.dxf".to_owned()),
                };
                self.new_project();
                self.project = f.project;
                self.sketch = sketch;
                self.drawing_name = name;
                self.model = f.model;
                self.path = Some(path.to_path_buf());
                self.mark_saved();
                Ok(format!("{} ouvert", path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()))
            }
            "dxf" | "dwg" => {
                let sketch = Sketch::open(path)?;
                self.new_project();
                self.sketch = sketch;
                self.drawing_name = format!("dessin.{ext}");
                self.fit_stock_to_drawing();
                self.step = Step::Dessin;
                Ok(format!("Dessin {} importé dans un nouveau projet", path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()))
            }
            _ => Err(format!("format non pris en charge ici : .{ext}")),
        }
    }

    /// Remplace le dessin du projet par un DXF ou un DWG (le reste du projet est gardé).
    fn import_drawing(&mut self, req: &mut Vec<Request>) {
        let Some(p) = rfd::FileDialog::new().set_title("Importer un dessin").add_filter("Dessin CadCraft (DXF, DWG)", &["dxf", "dwg"]).pick_file() else { return };
        match Sketch::open(&p) {
            Ok(s) => {
                self.sketch = s;
                let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_else(|| "dxf".into());
                self.drawing_name = format!("dessin.{ext}");
                self.view2d.reset();
                req.push(Request::Status(format!("Dessin {} importé ({} objets)", p.display(), self.sketch.objects())));
            }
            Err(e) => req.push(Request::Status(format!("Import impossible : {e}"))),
        }
    }

    fn save(&mut self, ask: bool, req: &mut Vec<Request>) {
        let path = match (&self.path, ask) {
            (Some(p), false) => p.clone(),
            _ => {
                let Some(mut p) = rfd::FileDialog::new().set_title("Enregistrer le projet").add_filter("Projet MakerCraft", &["mkc"]).set_file_name("projet.mkc").save_file() else {
                    return;
                };
                if p.extension().is_none() {
                    p.set_extension("mkc");
                }
                p
            }
        };
        let drawing = match self.sketch.to_bytes(&self.drawing_name) {
            Ok(b) => b,
            Err(e) => {
                req.push(Request::Status(format!("Dessin non écrit : {e}")));
                return;
            }
        };
        let f = MkcFile { project: self.project.clone(), drawing: Some((self.drawing_name.clone(), drawing)), model: self.model.clone() };
        match f.write(&path) {
            Ok(()) => {
                self.path = Some(path.clone());
                self.mark_saved();
                req.push(Request::Status(format!("Projet enregistré : {}", path.display())));
                req.push(Request::Recent(path));
            }
            Err(e) => req.push(Request::Status(format!("Échec de l'enregistrement : {e}"))),
        }
    }

    /// Brut = encombrement du dessin + 10 mm autour (zéro pièce au coin avant gauche).
    pub(crate) fn fit_stock_to_drawing(&mut self) -> bool {
        let b = self.sketch.bounds();
        if b.is_empty() {
            return false;
        }
        let s = b.size();
        self.project.stock.size[0] = (s.x + 20.0).ceil();
        self.project.stock.size[1] = (s.y + 20.0).ceil();
        self.project.stock.xy_origin = makercraft_mkc::XyOrigin::AvantGauche;
        true
    }

    fn stock_rect(&self) -> [f64; 4] {
        let (ox, oy) = toolpaths::stock_offset(&self.project);
        let s = self.project.stock.size;
        [ox, oy, ox + s[0], oy + s[1]]
    }

    /// Lance une commande de dessin de CadCraft (la commande en cours est annulée).
    fn draw_command(&mut self, name: &str, req: &mut Vec<Request>) {
        self.step = Step::Dessin;
        let sel = self.sketch.session.selection();
        if name == "explode" && !self.explode_confirm && self.sketch.has_array(&sel) {
            self.explode_confirm = true;
            return;
        }
        if name.starts_with("array") {
            self.sketch.prepare_array();
        }
        // lignes de construction et cotes : sur leur contour de repère, jamais usiné ; le contour
        // courant revient quand la commande est finie
        let reference = if matches!(name, "xline" | "ray") {
            Some("Construction")
        } else if name.starts_with("dim") {
            Some("Cotes")
        } else {
            None
        };
        if let Some(l) = reference {
            let cur = self.sketch.current_layer();
            if cur != l {
                self.restore_layer.get_or_insert(cur);
                self.sketch.set_current_layer(l);
            }
        }
        // réseau rectangulaire sur la sélection : créé tout de suite (2 x 2, pas = taille + 5 mm),
        // puis réglé dans Propriétés, chaque valeur mettant à jour le réseau dans la vue
        if name == "arrayrect"
            && !sel.is_empty()
            && let Some(b) = self.view2d.bbox_of(&self.sketch, &self.sketch.editable(&sel))
        {
            let hs: Vec<String> = self.sketch.editable(&sel).iter().map(|h| h.hex()).collect();
            let p = serde_json::json!({ "handles": hs, "rows": 2, "cols": 2, "colSpacing": (b[2] - b[0]) + 5.0, "rowSpacing": (b[3] - b[1]) + 5.0 });
            match self.sketch.session.execute("arrayrect", &p) {
                Ok(v) => {
                    if let Some(h) = v.get("handle").and_then(|h| h.as_str()).and_then(makercraft_dao::sketch::doc::Handle::parse_hex) {
                        self.sketch.session.set_selection(vec![h]);
                    }
                    self.side_tab = props::SideTab::Props;
                    req.push(Request::Status("Réseau créé : colonnes, rangées et pas se règlent dans Propriétés".into()));
                }
                Err(e) => req.push(Request::Status(e.to_string())),
            }
            return;
        }
        self.sketch.session.cancel();
        if let Err(e) = self.sketch.session.cmdline(name) {
            req.push(Request::Status(e.to_string()));
        }
    }

    fn add_toolpath(&mut self, kind: PathKind) {
        let n = self.project.toolpaths.len() + 1;
        let current = self.sketch.current_layer();
        let layers = if current == "0" { Vec::new() } else { vec![current] };
        let mut tp = Toolpath { name: format!("{} {n}", kind.label()), kind, layers, ..Toolpath::default() };
        tp.depth = match kind {
            PathKind::Gravure => 0.5,
            PathKind::Percage => self.project.stock.size[2],
            PathKind::ContourExterieur | PathKind::ContourInterieur | PathKind::SurLeTrait => self.project.stock.size[2],
            PathKind::Poche => 2.0,
        };
        self.project.toolpaths.push(tp);
        self.selected_path = Some(self.project.toolpaths.len() - 1);
        self.step = Step::Parcours;
    }

    // ------------------------------------------------------------------ ruban

    /// Onglets du ruban : un par étape, dans l'ordre.
    pub fn ribbon(&self) -> Vec<Tab> {
        let busy = self.sketch.session.running.is_some();
        let has_path = self.selected_path.is_some_and(|i| i < self.project.toolpaths.len());
        let has_clamp = self.selected_clamp.is_some_and(|i| i < self.project.clamps.len());
        let has_paths = self.project.toolpaths.iter().any(|t| t.enabled);
        let project = Group {
            label: "Projet",
            items: vec![
                Item::large("cnc.nouveau", "Nouveau", Icon::New).tip("Nouveau projet"),
                Item::large("cnc.ouvrir", "Ouvrir", Icon::Open).tone(Tone::Design).tip("Ouvrir un projet .mkc ou un dessin DXF/DWG (Ctrl+O)"),
                Item::large("cnc.enregistrer", "Enregistrer", Icon::Save).tone(Tone::Design).tip("Enregistrer le projet .mkc (Ctrl+S)"),
                Item::small("cnc.enregistrer_sous", "Enregistrer sous", Icon::Save),
            ],
        };
        let view = Group {
            label: "Vue",
            items: vec![
                Item::small("cnc.ajuster", "Ajuster la 2D", Icon::Fit).tip("Cadrer le brut et le dessin"),
                Item::small("cnc.vue3d", "Vue 3D de départ", Icon::Cube),
            ],
        };
        let draw = |id: &'static str, label: &'static str, icon: Icon| Item::large(id, label, icon).tone(Tone::Design);
        let modify = |id: &'static str, label: &'static str, icon: Icon| Item::large(id, label, icon);
        let mut tabs = Vec::new();
        for step in Step::ALL {
            let groups = match step {
                Step::Machine => vec![project_clone(&project), Group { label: "Machine", items: vec![Item::large("cnc.etape.machine", "Choisir", Icon::Machines).tone(Tone::Component).tip("Machine, courses et contrôleur (panneau de gauche)")] }],
                Step::Brut => vec![
                    Group { label: "Brut", items: vec![Item::large("cnc.brut_dessin", "Ajuster au dessin", Icon::Stock).tone(Tone::Component).enabled(self.sketch.objects() > 0).tip("Brut = encombrement du dessin + 10 mm autour")] },
                    Group {
                        label: "Bridage",
                        items: vec![
                            Item::large("cnc.bride_ajouter", "Ajouter une bride", Icon::Clamp).tone(Tone::Component).tip("Zone interdite à l'outil"),
                            Item::small("cnc.bride_supprimer", "Supprimer", Icon::Trash).enabled(has_clamp),
                        ],
                    },
                    view_clone(&view),
                ],
                Step::Dessin => vec![
                    Group { label: "Fichier", items: vec![Item::large("cnc.importer", "Importer", Icon::Open).tone(Tone::Design).tip("Remplacer le dessin par un DXF ou un DWG")] },
                    Group {
                        label: "Dessiner",
                        items: vec![
                            draw("cnc.cmd.line", "Ligne", Icon::Line),
                            draw("cnc.cmd.pline", "Polyligne", Icon::Polyline),
                            draw("cnc.cmd.rectang", "Rectangle", Icon::Rect),
                            draw("cnc.cmd.circle", "Cercle", Icon::Circle).variants(vec![
                                Variant::new("cnc.cmd.circle", "Centre, rayon", "Cercle", Icon::Circle),
                                Variant::new("cnc.cmd.circle.cd", "Centre, diamètre", "Cercle Ø", Icon::CircleCD),
                                Variant::new("cnc.cmd.circle.2p", "2 points (diamètre)", "Cercle 2 pts", Icon::Circle2P),
                                Variant::new("cnc.cmd.circle.3p", "3 points", "Cercle 3 pts", Icon::Circle3P),
                                Variant::new("cnc.cmd.circle.ttr", "Tangent, tangent, rayon", "Cercle TTR", Icon::CircleTTR),
                                Variant::new("cnc.cmd.circle.ttt", "3 tangentes", "Cercle TTT", Icon::CircleTTT),
                            ]),
                            draw("cnc.cmd.arc", "Arc", Icon::Arc).variants(vec![
                                Variant::new("cnc.cmd.arc", "3 points", "Arc", Icon::Arc3P),
                                Variant::new("cnc.cmd.arc.sce", "Départ, centre, fin", "Arc DCF", Icon::ArcSCE),
                                Variant::new("cnc.cmd.arc.sca", "Départ, centre, angle", "Arc DCA", Icon::ArcSCA),
                                Variant::new("cnc.cmd.arc.scl", "Départ, centre, longueur de corde", "Arc DCL", Icon::ArcSCL),
                                Variant::new("cnc.cmd.arc.sea", "Départ, fin, angle", "Arc DFA", Icon::ArcSEA),
                                Variant::new("cnc.cmd.arc.sed", "Départ, fin, direction", "Arc DFD", Icon::ArcSED),
                                Variant::new("cnc.cmd.arc.ser", "Départ, fin, rayon", "Arc DFR", Icon::ArcSER),
                                Variant::new("cnc.cmd.arc.cse", "Centre, départ, fin", "Arc CDF", Icon::ArcCSE),
                                Variant::new("cnc.cmd.arc.csa", "Centre, départ, angle", "Arc CDA", Icon::ArcCSA),
                                Variant::new("cnc.cmd.arc.csl", "Centre, départ, longueur de corde", "Arc CDL", Icon::ArcCSL),
                                Variant::new("cnc.cmd.arc.continue", "Dans la suite du dernier tracé", "Arc suite", Icon::ArcCont),
                            ]),
                            draw("cnc.cmd.ellipse", "Ellipse", Icon::Ellipse).more().variants(vec![
                                Variant::new("cnc.cmd.ellipse", "Centre, axes", "Ellipse", Icon::Ellipse),
                                Variant::new("cnc.cmd.ellipse.axis", "Axe, extrémité", "Ellipse axe", Icon::EllipseAxis),
                                Variant::new("cnc.cmd.ellipse.arc", "Arc d'ellipse", "Arc d'ellipse", Icon::EllipseArc),
                            ]),
                            draw("cnc.cmd.polygon", "Polygone", Icon::Polygon).more(),
                            draw("cnc.cmd.point", "Point", Icon::Point).more().tip("Point : repère de perçage").variants(vec![
                                Variant::new("cnc.cmd.point", "Un point", "Point", Icon::Point),
                                Variant::new("cnc.cmd.point.multiple", "Plusieurs points (Entrée termine)", "Points", Icon::PointMulti),
                            ]),
                            draw("cnc.cmd.spline", "Spline", Icon::Spline).more(),
                            draw("cnc.etoile", "Étoile", Icon::Star).more().tip("Étoile : branches, rayons, rotation"),
                            draw("cnc.oblong", "Oblong", Icon::Oblong).more().tip("Oblong (rainure, boutonnière) : longueur, largeur"),
                        ],
                    },
                    Group {
                        label: "Modifier",
                        items: vec![
                            modify("cnc.cmd.move", "Déplacer", Icon::Move),
                            modify("cnc.cmd.copy", "Copier", Icon::Copy),
                            modify("cnc.cmd.rotate", "Rotation", Icon::Rotate),
                            modify("cnc.cmd.mirror", "Miroir", Icon::Mirror).variants(vec![
                                Variant::new("cnc.cmd.mirror", "Par deux points (copie ou non)", "Miroir", Icon::Mirror),
                                Variant::new("cnc.retourner_h", "Retourner horizontalement (sur place)", "Retourner H", Icon::FlipH),
                                Variant::new("cnc.retourner_v", "Retourner verticalement (sur place)", "Retourner V", Icon::FlipV),
                            ]),
                            modify("cnc.cmd.offset", "Décaler", Icon::Offset),
                            modify("cnc.cmd.fillet", "Raccord", Icon::Fillet),
                            modify("cnc.cmd.scale", "Échelle", Icon::Scale).more(),
                            modify("cnc.cmd.trim", "Ajuster", Icon::Trim).more(),
                            modify("cnc.cmd.break", "Couper", Icon::Break).more(),
                            modify("cnc.cmd.extend", "Prolonger", Icon::Extend).more(),
                            modify("cnc.cmd.join", "Joindre", Icon::Join).more(),
                            modify("cnc.cmd.explode", "Décomposer", Icon::Explode).more(),
                            modify("cnc.cmd.chamfer", "Chanfrein", Icon::Chamfer).more(),
                            // le long d'un tracé : pas encore associatif dans CadCraft 0.20.1 (copies sans
                            // lien), donc pas proposé tant que le lien avec la forme d'origine ne tient pas
                            modify("cnc.cmd.arrayrect", "Réseau", Icon::Array).more().variants(vec![
                                Variant::new("cnc.cmd.arrayrect", "Rectangulaire", "Réseau", Icon::Array),
                                Variant::new("cnc.cmd.arraypolar", "Circulaire", "Réseau circ.", Icon::ArrayPolar),
                            ]),
                            modify("cnc.cmd.align", "Caler", Icon::AlignPts).more().tip("Caler la sélection : point source puis point cible (deux paires pour tourner, mise à l'échelle au choix)"),
                            modify("cnc.cmd.stretch", "Étirer", Icon::Stretch).more().tip("Étirer : fenêtre de capture (de droite à gauche), puis déplacement"),
                            modify("cnc.remplir_brut", "Remplir le brut", Icon::FillStock).more().tip("Mettre la sélection à l'échelle du brut, sans déformer, et la centrer"),
                            modify("cnc.cmd.pedit", "Polyligne", Icon::Polyline).more().tip("Modifier une polyligne : fermer, ouvrir, joindre, lisser, sommets"),
                            modify("cnc.cmd.divide", "Diviser", Icon::PointMulti).more().tip("Points régulièrement répartis sur un tracé (nombre de parts)"),
                            modify("cnc.cmd.measure", "Jalonner", Icon::PointMulti).more().tip("Points tous les N mm le long d'un tracé"),
                            modify("cnc.cmd.lengthen", "Allonger", Icon::Extend).more().tip("Allonger ou raccourcir un trait ou un arc"),
                            modify("cnc.cmd.reverse", "Inverser le sens", Icon::Redo).more().tip("Inverser le sens de parcours d'un tracé (utile aux parcours d'usinage)"),
                            modify("cnc.cmd.blend", "Raccord lisse", Icon::Spline).more().tip("Courbe de raccord continue entre deux tracés"),
                            modify("cnc.cmd.overkill", "Doublons", Icon::Trash).more().tip("Supprimer les objets en double et les traits superposés"),
                            modify("cnc.reparer", "Réparer", Icon::Repair).more().tip("Fermer les tracés presque fermés (0,05 mm) et joindre ceux qui se touchent"),
                            modify("cnc.degagement", "Dégagements", Icon::Dogbone).more().tip("Dégagements d'angle pour l'assemblage : la fraise atteint le fond des coins").variants(vec![
                                Variant::new("cnc.degagement", "Dogbone (selon la bissectrice)", "Dogbone", Icon::Dogbone),
                                Variant::new("cnc.degagement_t", "T-bone (dans le prolongement d'un côté)", "T-bone", Icon::Dogbone),
                            ]),
                            modify("cnc.cmd.erase", "Effacer", Icon::Trash).more(),
                        ],
                    },
                    Group {
                        label: "Aides",
                        items: vec![
                            modify("cnc.cmd.xline", "Construction", Icon::XLine).tip("Ligne de construction, sur le contour Construction (jamais usiné)").variants(vec![
                                Variant::new("cnc.cmd.xline", "Ligne infinie", "Construction", Icon::XLine),
                                Variant::new("cnc.cmd.ray", "Demi-droite", "Demi-droite", Icon::XLine),
                            ]),
                            modify("cnc.cmd.dimlinear", "Cote", Icon::Dim).tip("Cote, sur le contour Cotes (jamais usiné)").variants(vec![
                                Variant::new("cnc.cmd.dimlinear", "Linéaire (horizontale, verticale)", "Cote", Icon::Dim),
                                Variant::new("cnc.cmd.dimaligned", "Alignée", "Cote alignée", Icon::Dim),
                                Variant::new("cnc.cmd.dimradius", "Rayon", "Cote R", Icon::Dim),
                                Variant::new("cnc.cmd.dimdiameter", "Diamètre", "Cote Ø", Icon::Dim),
                                Variant::new("cnc.cmd.dimangular", "Angle", "Cote angle", Icon::Dim),
                            ]),
                            modify("cnc.cmd.autoconstrain", "Contraintes", Icon::Constraint).tip("Contraintes géométriques et cotes pilotantes de CadCraft").variants(vec![
                                Variant::new("cnc.cmd.autoconstrain", "Automatiques (sur la sélection)", "Contraintes", Icon::Constraint),
                                Variant::new("cnc.cmd.geomconstraint", "Géométrique (parallèle, perpendiculaire...)", "Contrainte", Icon::Constraint),
                                Variant::new("cnc.cmd.dimconstraint", "Cote pilotante", "Cote pilote", Icon::Constraint),
                                Variant::new("cnc.cmd.delconstraint", "Supprimer les contraintes", "Sans contrainte", Icon::Constraint),
                            ]),
                            modify("cnc.cmd.dist", "Mesurer", Icon::Measure).tip("Distance entre deux points ; le résultat s'affiche sous la vue").variants(vec![
                                Variant::new("cnc.cmd.dist", "Distance", "Mesurer", Icon::Measure),
                                Variant::new("cnc.cmd.area", "Aire et périmètre", "Aire", Icon::Measure),
                            ]),
                        ],
                    },
                    Group {
                        label: "Organiser",
                        items: vec![
                            modify("cnc.grouper", "Grouper", Icon::Group).enabled(false).tip("À venir"),
                            modify("cnc.booleens", "Booléens", Icon::Boolean).enabled(false).tip("À venir"),
                            modify("cnc.aligner", "Aligner", Icon::Align).more().enabled(false).tip("À venir"),
                            modify("cnc.repartir", "Répartir", Icon::Distribute).more().enabled(false).tip("À venir"),
                        ],
                    },
                    Group {
                        label: "Contours",
                        items: vec![Item::large("cnc.contour_nouveau", "Nouveau contour", Icon::Layers).tone(Tone::Design).tip("Un calque = un contour à usiner")],
                    },
                    Group {
                        label: "Historique",
                        items: vec![modify("cnc.annuler", "Annuler", Icon::Undo).enabled(!busy), modify("cnc.retablir", "Rétablir", Icon::Redo).enabled(!busy)],
                    },
                    view_clone(&view),
                ],
                Step::Parcours => vec![
                    Group {
                        label: "Ajouter un parcours",
                        items: vec![
                            Item::large("cnc.parcours.ext", "Contour extérieur", Icon::Rect).tone(Tone::Component),
                            Item::large("cnc.parcours.poche", "Poche", Icon::Pocket).tone(Tone::Component),
                            Item::small("cnc.parcours.int", "Contour intérieur", Icon::Offset),
                            Item::small("cnc.parcours.trait", "Sur le trait", Icon::Line),
                            Item::small("cnc.parcours.gravure", "Gravure", Icon::Engrave),
                            Item::large("cnc.parcours.percage", "Perçage", Icon::Drill).tone(Tone::Component),
                        ],
                    },
                    Group {
                        label: "Parcours",
                        items: vec![
                            Item::small("cnc.parcours.vitesses", "Proposer les vitesses", Icon::Check).enabled(has_path).tip("Avance, plongée, rotation et passes d'après la banque d'outils"),
                            Item::small("cnc.parcours.dupliquer", "Dupliquer", Icon::Copy).enabled(has_path),
                            Item::small("cnc.parcours.supprimer", "Supprimer", Icon::Trash).enabled(has_path),
                        ],
                    },
                    view_clone(&view),
                ],
                Step::Simulation => vec![
                    Group { label: "Affichage", items: vec![Item::small("cnc.rapides", "Déplacements rapides", Icon::Rapids).checked(self.show_rapids)] },
                    view_clone(&view),
                ],
                Step::Usinage => vec![Group {
                    label: "G-code",
                    items: vec![
                        Item::large("cnc.gcode", "Écrire le .nc", Icon::Export).tone(Tone::Component).enabled(has_paths).tip("Programme Grbl, pause M0 aux changements d'outil"),
                        Item::large("cnc.envoyer", "Vers Machines", Icon::Play).tone(Tone::Component).enabled(has_paths).tip("Ouvrir le programme dans l'atelier Machines"),
                    ],
                }],
            };
            tabs.push(Tab::new(step.label(), groups));
        }
        tabs
    }

    /// Exécute une commande du ruban ou de la barre d'accès rapide.
    pub fn command(&mut self, id: &str, req: &mut Vec<Request>) {
        if let Some(name) = id.strip_prefix("cnc.cmd.") {
            self.draw_command(name, req);
            return;
        }
        match id {
            "cnc.nouveau" => {
                self.new_project();
                req.push(Request::Status("Nouveau projet".into()));
            }
            "cnc.ouvrir" => req.push(Request::OpenDialog),
            "cnc.enregistrer" => self.save(false, req),
            "cnc.enregistrer_sous" => self.save(true, req),
            "cnc.ajuster" => self.view2d.reset(),
            "cnc.vue3d" => self.view3d.home(),
            "cnc.etape.machine" => self.step = Step::Machine,
            "cnc.brut_dessin" => {
                if self.fit_stock_to_drawing() {
                    self.view2d.reset();
                }
            }
            "cnc.bride_ajouter" => {
                let n = self.project.clamps.len() + 1;
                let s = self.project.stock.size;
                let (x, y) = match n % 4 {
                    1 => (-10.0, s[1] / 2.0 - 10.0),
                    2 => (s[0] - 10.0, s[1] / 2.0 - 10.0),
                    3 => (s[0] / 2.0 - 10.0, -10.0),
                    _ => (s[0] / 2.0 - 10.0, s[1] - 10.0),
                };
                self.project.clamps.push(makercraft_mkc::Clamp { name: format!("Bride {n}"), x, y, ..makercraft_mkc::Clamp::default() });
                self.selected_clamp = Some(self.project.clamps.len() - 1);
                self.step = Step::Brut;
            }
            "cnc.bride_supprimer" => {
                if let Some(i) = self.selected_clamp.filter(|i| *i < self.project.clamps.len()) {
                    self.project.clamps.remove(i);
                    self.selected_clamp = None;
                }
            }
            "cnc.importer" => self.import_drawing(req),
            "cnc.contour_nouveau" => {
                let name = self.sketch.next_contour_name();
                self.sketch.set_current_layer(&name);
                req.push(Request::Status(format!("Calque courant : {name}")));
            }
            "cnc.annuler" => {
                if let Err(e) = self.sketch.session.undo() {
                    req.push(Request::Status(e.to_string()));
                }
            }
            "cnc.retablir" => {
                if let Err(e) = self.sketch.session.redo() {
                    req.push(Request::Status(e.to_string()));
                }
            }
            "cnc.parcours.ext" => self.add_toolpath(PathKind::ContourExterieur),
            "cnc.parcours.int" => self.add_toolpath(PathKind::ContourInterieur),
            "cnc.parcours.trait" => self.add_toolpath(PathKind::SurLeTrait),
            "cnc.parcours.poche" => self.add_toolpath(PathKind::Poche),
            "cnc.parcours.percage" => self.add_toolpath(PathKind::Percage),
            "cnc.parcours.gravure" => self.add_toolpath(PathKind::Gravure),
            "cnc.parcours.vitesses" => {
                if let Some(i) = self.selected_path {
                    let msg = panels::suggest_speeds(self, i);
                    req.push(Request::Status(msg));
                }
            }
            "cnc.parcours.dupliquer" => {
                if let Some(tp) = self.selected_path.and_then(|i| self.project.toolpaths.get(i)).cloned() {
                    self.project.toolpaths.push(Toolpath { name: format!("{} (copie)", tp.name), ..tp });
                    self.selected_path = Some(self.project.toolpaths.len() - 1);
                }
            }
            "cnc.parcours.supprimer" => {
                if let Some(i) = self.selected_path.filter(|i| *i < self.project.toolpaths.len()) {
                    self.project.toolpaths.remove(i);
                    self.selected_path = None;
                }
            }
            "cnc.retourner_h" | "cnc.retourner_v" => self.flip(id == "cnc.retourner_h", req),
            "cnc.remplir_brut" => self.fill_stock(req),
            "cnc.reparer" => {
                if let Some((sel, _)) = self.selection_box(req) {
                    match self.sketch.repair(&sel, 0.05) {
                        Ok(n) => req.push(Request::Status(format!("Réparé : {n} tracé(s) fermé(s), tracés qui se touchent joints"))),
                        Err(e) => req.push(Request::Status(e)),
                    }
                }
            }
            "cnc.degagement" | "cnc.degagement_t" => {
                if self.selection_box(req).is_some() {
                    self.relief = Some((id == "cnc.degagement_t", self.relief.map_or(1.5875, |r| r.1)));
                }
            }
            "cnc.etoile" | "cnc.oblong" => {
                self.step = Step::Dessin;
                let s = self.stock_rect();
                let (cx, cy) = ((s[0] + s[2]) / 2.0, (s[1] + s[3]) / 2.0);
                self.shape = Some(ShapeDialog { star: id == "cnc.etoile", branches: 5, outer: 25.0, inner: 10.0, length: 60.0, width: 20.0, overall: true, rot: 0.0, cx, cy });
            }
            "cnc.rapides" => self.show_rapids = !self.show_rapids,
            "cnc.gcode" => panels::write_gcode(self, req, false),
            "cnc.envoyer" => panels::write_gcode(self, req, true),
            _ => {}
        }
    }

    /// Réglages du dessin choisis dans les Options.
    pub fn apply_prefs(&mut self, p: &Prefs) {
        self.view2d.hide_origin = !p.origin_2d;
        self.view2d.no_commands = !p.cadcraft_commands;
        self.origin_3d = p.origin_3d;
    }

    // ------------------------------------------------------------------ fenêtre

    pub fn ui(&mut self, ui: &mut Ui, pal: &Palette, units: Units, req: &mut Vec<Request>) {
        let full = ui.available_rect_before_wrap();
        ui.allocate_rect(full, Sense::hover());
        let panel = Rect::from_min_size(full.min, vec2(PANEL_W.min(full.width() * 0.3), full.height()));
        let work = Rect::from_min_max(pos2(panel.right() + 14.0, full.top()), full.max);
        let dessin = self.step == Step::Dessin;
        // étape Dessin : colonne de droite à onglets (Propriétés, 3D, Commentaires) ;
        // ailleurs : 2D et 3D côte à côte, séparation déplaçable
        let side_w = PANEL_W.min(work.width() * 0.4);
        let split_x = if dessin { work.right() - side_w - 7.0 } else { work.left() + work.width() * self.split };
        let r2d = Rect::from_min_max(work.min, pos2(split_x - 5.0, work.bottom()));
        let r3d = Rect::from_min_max(pos2(split_x + 5.0, work.top()), work.max);

        // panneau de l'étape
        let mut pui = ui.new_child(UiBuilder::new().max_rect(panel));
        ScrollArea::vertical().id_salt(("cnc-panneau", self.step.index())).auto_shrink([false, false]).show(&mut pui, |ui| {
            ui.set_max_width(panel.width() - 12.0);
            ui.spacing_mut().item_spacing.y = 12.0;
            panels::show(self, ui, pal, units, req);
        });

        // séparation 2D | 3D
        if !dessin {
            let sep = Rect::from_center_size(pos2(split_x, work.center().y), vec2(10.0, work.height()));
            let sresp = ui.interact(sep, ui.scope_id().with("separation-2d-3d"), Sense::drag()).on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
            if sresp.dragged() {
                self.split = ((split_x + sresp.drag_delta().x - work.left()) / work.width()).clamp(0.25, 0.8);
            }
            ui.painter().rect_filled(Rect::from_center_size(sep.center(), vec2(3.0, 40.0)), CornerRadius::same(2), if sresp.hovered() { pal.blue } else { pal.line2 });
        }

        // 2D
        if self.view2d.needs_frame() {
            let mut b = self.stock_rect();
            let d = self.sketch.bounds();
            if !d.is_empty() {
                b = [b[0].min(d.min.x), b[1].min(d.min.y), b[2].max(d.max.x), b[3].max(d.max.y)];
            }
            self.view2d.frame(&mut self.sketch, b, f64::from(r2d.width() / r2d.height().max(1.0)));
        }
        self.view2d.editable = self.step == Step::Dessin;
        self.view2d.free_scale = !self.props.linked;
        self.view2d.stock = self.stock_rect();
        let stock = self.stock_rect();
        let clamps: Vec<[f64; 4]> = {
            let (ox, oy) = toolpaths::stock_offset(&self.project);
            self.project.clamps.iter().map(|c| [c.x + ox, c.y + oy, c.x + ox + c.w, c.y + oy + c.h]).collect()
        };
        let sel_clamp = self.selected_clamp;
        let show_paths = matches!(self.step, Step::Parcours | Step::Simulation | Step::Usinage);
        let paths2d: Vec<Vec<[f64; 3]>> = if show_paths { self.feed_lines() } else { Vec::new() };
        let stock_fill = pal.stock_fill;
        let stock_line = pal.stock_line;
        let (orange, orange_soft) = (pal.orange, pal.orange_soft);
        let out = self.view2d.show(
            ui,
            r2d,
            pal,
            &mut self.sketch,
            |p, xf| {
                let r = Rect::from_two_pos(xf.to_screen(cg(stock[0], stock[1])), xf.to_screen(cg(stock[2], stock[3])));
                p.rect_filled(r, CornerRadius::ZERO, stock_fill);
                p.rect_stroke(r, CornerRadius::ZERO, egui::Stroke::new(1.2, stock_line), egui::StrokeKind::Middle);
                for (i, c) in clamps.iter().enumerate() {
                    let cr = Rect::from_two_pos(xf.to_screen(cg(c[0], c[1])), xf.to_screen(cg(c[2], c[3])));
                    p.rect_filled(cr, CornerRadius::same(2), orange_soft);
                    let w = if sel_clamp == Some(i) { 2.4 } else { 1.2 };
                    p.rect_stroke(cr, CornerRadius::same(2), egui::Stroke::new(w, orange), egui::StrokeKind::Middle);
                }
            },
            |p, xf| {
                for l in &paths2d {
                    let pts: Vec<egui::Pos2> = l.iter().map(|q| xf.to_screen(cg(q[0], q[1]))).collect();
                    p.add(egui::Shape::line(pts, egui::Stroke::new(1.2, orange)));
                }
            },
        );
        if let Some(s) = out.status {
            req.push(Request::Status(s));
        }
        if dessin {
            if out.open_dims {
                self.dims_open = true;
            }
            if let Some(c) = out.command {
                self.draw_command(c, req);
            }
            if self.sketch.session.running.is_none()
                && let Some(l) = self.restore_layer.take()
            {
                self.sketch.set_current_layer(&l);
            }
            self.explode_window(ui, req);
            self.shape_window(ui, req);
            self.relief_window(ui, req);
            // objets des contours cachés ou verrouillés : hors sélection
            let sel = self.sketch.session.selection();
            let ok = self.sketch.editable(&sel);
            if ok.len() != sel.len() {
                self.sketch.session.set_selection(ok);
            }
            self.side_panel(ui, r3d, pal, req);
            self.dims_window(ui, r2d, pal, req);
            return;
        }

        // 3D
        let scene = self.scene3d(pal);
        let caption = match self.step {
            Step::Machine => "courses de la machine",
            Step::Brut => "brut et brides",
            Step::Dessin => "contours posés sur le brut",
            Step::Parcours | Step::Usinage => "trajectoires de l'outil",
            Step::Simulation => "trajectoires (enlèvement de matière : à venir)",
        };
        self.view3d.show(ui, r3d, pal, &scene, caption);
    }

    /// Colonne de droite de l'étape Dessin : un panneau à la fois, onglets en bas.
    fn side_panel(&mut self, ui: &mut Ui, r: Rect, pal: &Palette, req: &mut Vec<Request>) {
        const TABS_H: f32 = 30.0;
        let body = Rect::from_min_max(r.min, pos2(r.right(), r.bottom() - TABS_H - 6.0));
        let tabs = Rect::from_min_max(pos2(r.left(), r.bottom() - TABS_H), r.max);
        if self.side_tab == props::SideTab::View3d {
            let scene = self.scene3d(pal);
            self.view3d.show(ui, body, pal, &scene, "contours posés sur le brut");
        } else {
            let mut cui = ui.new_child(UiBuilder::new().max_rect(body));
            ScrollArea::vertical().id_salt(("cnc-droite", self.side_tab.label())).auto_shrink([false, false]).show(&mut cui, |ui| {
                ui.set_max_width(body.width() - 12.0);
                if self.side_tab == props::SideTab::Props {
                    crate::widgets::card(ui, pal, Some("Propriétés"), |ui| {
                        let sel = self.sketch.editable(&self.sketch.session.selection());
                        let b = self.view2d.bbox_of(&self.sketch, &sel);
                        if let Some(m) = props::editor(ui, pal, &mut self.props, &mut self.sketch, &sel, b) {
                            req.push(Request::Status(m));
                        }
                    });
                } else {
                    let layer = self.sketch.session.selection().first().and_then(|h| self.sketch.layer_of(*h)).unwrap_or_else(|| self.sketch.current_layer());
                    crate::widgets::card(ui, pal, Some("Commentaires"), |ui| props::comments(ui, pal, &mut self.project.comments, &layer));
                }
            });
        }
        let mut tui = ui.new_child(UiBuilder::new().max_rect(tabs).layout(egui::Layout::left_to_right(egui::Align::Center)));
        for t in props::SideTab::ALL {
            tui.selectable_value(&mut self.side_tab, t, t.label());
        }
    }

    /// Objets choisis modifiables et leur boîte (None et message s'il n'y en a pas).
    fn selection_box(&mut self, req: &mut Vec<Request>) -> Option<(Vec<makercraft_dao::sketch::doc::Handle>, [f64; 4])> {
        self.step = Step::Dessin;
        let sel = self.sketch.editable(&self.sketch.session.selection());
        match self.view2d.bbox_of(&self.sketch, &sel) {
            Some(b) if !sel.is_empty() => Some((sel, b)),
            _ => {
                req.push(Request::Status("Choisir d'abord les objets (clic ou rectangle dans la vue)".into()));
                None
            }
        }
    }

    /// Retourne la sélection sur place (miroir par son axe vertical ou horizontal, sans copie).
    fn flip(&mut self, horizontal: bool, req: &mut Vec<Request>) {
        let Some((sel, b)) = self.selection_box(req) else { return };
        let (cx, cy) = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
        let (p1, p2) = if horizontal { ([cx, b[1]], [cx, b[3]]) } else { ([b[0], cy], [b[2], cy]) };
        let hs: Vec<String> = sel.iter().map(|h| h.hex()).collect();
        if let Err(e) = self.sketch.session.execute("mirror", &serde_json::json!({ "handles": hs, "p1": p1, "p2": p2, "erase": true })) {
            req.push(Request::Status(e.to_string()));
        }
    }

    /// Met la sélection à l'échelle du brut sans la déformer, puis la centre dessus.
    fn fill_stock(&mut self, req: &mut Vec<Request>) {
        let Some((sel, b)) = self.selection_box(req) else { return };
        let s = self.stock_rect();
        let (w, h) = (b[2] - b[0], b[3] - b[1]);
        let k = [(w, s[2] - s[0]), (h, s[3] - s[1])].iter().filter(|(a, _)| *a > 1e-9).map(|(a, t)| t / a).fold(f64::INFINITY, f64::min);
        if !k.is_finite() || k <= 0.0 {
            req.push(Request::Status("Sélection sans largeur ni hauteur".into()));
            return;
        }
        let c = cg((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
        let target = cg((s[0] + s[2]) / 2.0, (s[1] + s[3]) / 2.0);
        let r = select_frame::Xform::Scale { base: c, factor: k }
            .execute(&mut self.sketch, &sel)
            .and_then(|()| select_frame::Xform::Move(cg(target.x - c.x, target.y - c.y)).execute(&mut self.sketch, &sel));
        match r {
            Ok(()) => req.push(Request::Status(format!("Sélection mise à l'échelle du brut (x {k:.3}) et centrée"))),
            Err(e) => req.push(Request::Status(e)),
        }
    }

    /// Boîte Étoile / Oblong : valeurs, centre (milieu du brut par défaut), puis Placer.
    fn shape_window(&mut self, ui: &mut Ui, req: &mut Vec<Request>) {
        let Some(mut d) = self.shape else { return };
        let mut open = true;
        let mut place = false;
        let mut cancel = false;
        let title = if d.star { "Étoile" } else { "Oblong" };
        egui::Window::new(title).collapsible(false).resizable(false).open(&mut open).show(ui.ctx(), |ui| {
            let row = |ui: &mut Ui, label: &str, v: &mut f64, min: f64, suffix: &str| {
                ui.horizontal(|ui| {
                    ui.add_sized(vec2(140.0, 20.0), egui::Label::new(label));
                    ui.add(egui::DragValue::new(v).range(min..=1e5).speed(0.1).max_decimals(3).suffix(suffix));
                });
            };
            if d.star {
                ui.horizontal(|ui| {
                    ui.add_sized(vec2(140.0, 20.0), egui::Label::new("Branches"));
                    ui.add(egui::DragValue::new(&mut d.branches).range(3..=64));
                });
                row(ui, "Rayon extérieur", &mut d.outer, 0.01, " mm");
                row(ui, "Rayon intérieur", &mut d.inner, 0.01, " mm");
            } else {
                ui.horizontal(|ui| {
                    ui.add_sized(vec2(140.0, 20.0), egui::Label::new("Longueur"));
                    ui.radio_value(&mut d.overall, true, "hors tout");
                    ui.radio_value(&mut d.overall, false, "entraxe");
                });
                row(ui, "Longueur", &mut d.length, 0.01, " mm");
                row(ui, "Largeur", &mut d.width, 0.01, " mm");
            }
            row(ui, "Rotation", &mut d.rot, -360.0, "°");
            row(ui, "Centre X", &mut d.cx, -1e5, " mm");
            row(ui, "Centre Y", &mut d.cy, -1e5, " mm");
            ui.label(egui::RichText::new("Posée sur le contour courant ; elle se déplace ensuite avec le cadre de sélection ou dans Propriétés.").size(12.0).weak());
            ui.horizontal(|ui| {
                if ui.button("Placer").clicked() {
                    place = true;
                }
                if ui.button("Annuler").clicked() {
                    cancel = true;
                }
            });
        });
        self.shape = Some(d);
        if place {
            let v = if d.star {
                makercraft_dao::shapes::star(d.branches, d.outer, d.inner, (d.cx, d.cy), d.rot)
            } else {
                makercraft_dao::shapes::oblong(d.length, d.width, d.overall, (d.cx, d.cy), d.rot)
            };
            match v.and_then(|v| self.sketch.add_closed_polyline(&v)) {
                Ok(h) => {
                    self.sketch.session.set_selection(h.into_iter().collect());
                    self.side_tab = props::SideTab::Props;
                    open = false;
                }
                Err(e) => req.push(Request::Status(e)),
            }
        }
        if !open || cancel || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.shape = None;
        }
    }

    /// Boîte des dégagements d'angle : rayon de la fraise, puis Appliquer.
    fn relief_window(&mut self, ui: &mut Ui, req: &mut Vec<Request>) {
        let Some((tbone, mut r)) = self.relief else { return };
        let mut go = None;
        egui::Modal::new(egui::Id::unique("degagements")).show(ui.ctx(), |ui| {
            ui.set_width(360.0);
            ui.label(egui::RichText::new(if tbone { "Dégagements T-bone" } else { "Dégagements dogbone" }).strong().size(15.0));
            ui.label("À chaque coin saillant des tracés fermés choisis (poche, trou, mortaise) : une boucle du diamètre de la fraise, pour que la pièce qui s'emboîte aille jusqu'au fond.");
            ui.horizontal(|ui| {
                ui.label("Rayon de la fraise");
                ui.add(egui::DragValue::new(&mut r).range(0.05..=25.0).speed(0.01).max_decimals(4).suffix(" mm"));
                ui.label(egui::RichText::new(format!("(Ø {})", (r * 2000.0).round() / 1000.0)).weak());
            });
            ui.horizontal(|ui| {
                if ui.button("Appliquer").clicked() {
                    go = Some(true);
                }
                if ui.button("Annuler").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    go = Some(false);
                }
            });
        });
        self.relief = Some((tbone, r));
        match go {
            Some(true) => {
                self.relief = None;
                let sel = self.sketch.editable(&self.sketch.session.selection());
                match self.sketch.relieve_corners(&sel, r, tbone) {
                    Ok(0) => req.push(Request::Status("Aucun coin à dégager : il faut des polylignes fermées sans arcs".into())),
                    Ok(n) => req.push(Request::Status(format!("{n} dégagement(s) ajouté(s)"))),
                    Err(e) => req.push(Request::Status(e)),
                }
            }
            Some(false) => self.relief = None,
            None => {}
        }
    }

    /// Confirmation avant de décomposer un réseau.
    fn explode_window(&mut self, ui: &mut Ui, req: &mut Vec<Request>) {
        if !self.explode_confirm {
            return;
        }
        let mut choice = None;
        egui::Modal::new(egui::Id::unique("decomposer-reseau")).show(ui.ctx(), |ui| {
            ui.set_width(380.0);
            ui.label(egui::RichText::new("Décomposer le réseau ?").strong().size(15.0));
            ui.add_space(4.0);
            ui.label("Ses exemplaires deviennent des objets séparés : le réseau ne sera plus modifiable, et la forme d'origine ne le pilotera plus.");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Décomposer").clicked() {
                    choice = Some(true);
                }
                if ui.button("Annuler").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    choice = Some(false);
                }
            });
        });
        match choice {
            Some(true) => {
                // explode_confirm reste vrai le temps de relancer la commande sans redemander
                self.draw_command("explode", req);
                self.explode_confirm = false;
            }
            Some(false) => self.explode_confirm = false,
            None => {}
        }
    }

    /// Boîte des cotes de la sélection, ouverte par Entrée (à la manière de Galaad).
    fn dims_window(&mut self, ui: &mut Ui, r2d: Rect, pal: &Palette, req: &mut Vec<Request>) {
        if !self.dims_open {
            return;
        }
        let sel = self.sketch.editable(&self.sketch.session.selection());
        let b = self.view2d.bbox_of(&self.sketch, &sel);
        let mut open = true;
        let mut close = false;
        egui::Window::new("Cotes de la sélection")
            .collapsible(false)
            .resizable(false)
            .default_pos(r2d.center() - vec2(150.0, 120.0))
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.set_width(300.0);
                if let Some(m) = props::editor(ui, pal, &mut self.props, &mut self.sketch, &sel, b) {
                    req.push(Request::Status(m));
                }
                if ui.button("Fermer (Échap)").clicked() {
                    close = true;
                }
            });
        if !open || close || sel.is_empty() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.dims_open = false;
        }
    }

    /// Trajets en usinage (avance) de tous les parcours actifs, en polylignes 3D.
    fn feed_lines(&mut self) -> Vec<Vec<[f64; 3]>> {
        let start = [0.0, 0.0, self.project.clearance()];
        let job = self.job();
        job.groups.iter().flat_map(|g| g.ops.iter()).flat_map(|op| toolpaths::polylines(op, start)).filter(|(_, rapid)| !rapid).map(|(p, _)| p).collect()
    }

    fn scene3d(&mut self, pal: &Palette) -> Scene3 {
        let rgb = |c: egui::Color32| [c.r(), c.g(), c.b()];
        let mut sc = Scene3::default();
        let s = self.project.stock.size;
        let (ox, oy) = toolpaths::stock_offset(&self.project);
        if self.step == Step::Machine {
            let t = self.project.machine.travel;
            let c = rgb(pal.weak);
            let rect = |z: f64| vec![[0.0, 0.0, z], [t[0], 0.0, z], [t[0], t[1], z], [0.0, t[1], z], [0.0, 0.0, z]];
            sc.lines.push(Line3 { points: rect(-s[2]), rgb: c, width: 1.5, on_top: false });
            sc.lines.push(Line3 { points: rect(t[2] - s[2]), rgb: c, width: 1.0, on_top: false });
            for (x, y) in [(0.0, 0.0), (t[0], 0.0), (t[0], t[1]), (0.0, t[1])] {
                sc.lines.push(Line3 { points: vec![[x, y, -s[2]], [x, y, t[2] - s[2]]], rgb: c, width: 1.0, on_top: false });
            }
        }
        let see_through = matches!(self.step, Step::Parcours | Step::Simulation | Step::Usinage);
        sc.boxes.push(Box3 { min: [ox, oy, -s[2]], max: [ox + s[0], oy + s[1], 0.0], rgb: [196, 160, 112], opacity: if see_through { 0.45 } else { 1.0 } });
        for c in &self.project.clamps {
            sc.boxes.push(Box3 { min: [c.x + ox, c.y + oy, 0.0], max: [c.x + ox + c.w, c.y + oy + c.h, c.height], rgb: rgb(pal.orange), opacity: 1.0 });
        }
        if self.step != Step::Machine {
            for p in self.sketch.all_contours(0.05) {
                let mut pts: Vec<[f64; 3]> = p.points.iter().map(|v| [v.x, v.y, 0.05]).collect();
                if p.closed
                    && let Some(f) = pts.first().copied()
                {
                    pts.push(f);
                }
                sc.lines.push(Line3 { points: pts, rgb: rgb(pal.blue), width: 1.6, on_top: true });
            }
        }
        if see_through {
            let start = [0.0, 0.0, self.project.clearance()];
            let rapids = self.show_rapids;
            let (feed_c, rapid_c) = (rgb(pal.orange), rgb(pal.rapid));
            let job = self.job();
            for op in job.groups.iter().flat_map(|g| g.ops.iter()) {
                for (pts, rapid) in toolpaths::polylines(op, start) {
                    if rapid && !rapids {
                        continue;
                    }
                    sc.lines.push(Line3 { points: pts, rgb: if rapid { rapid_c } else { feed_c }, width: if rapid { 1.0 } else { 1.6 }, on_top: false });
                }
            }
        }
        if self.origin_3d {
            // zéro pièce : X rouge, Y vert, Z bleu
            let k = (s[0].max(s[1]) * 0.12).clamp(5.0, 40.0);
            for (end, c) in [([k, 0.0, 0.0], pal.red), ([0.0, k, 0.0], pal.green), ([0.0, 0.0, k], pal.blue)] {
                sc.lines.push(Line3 { points: vec![[0.0, 0.0, 0.0], end], rgb: rgb(c), width: 2.0, on_top: true });
            }
        }
        sc
    }
}

fn cg(x: f64, y: f64) -> makercraft_dao::sketch::cgeom::Vec2 {
    makercraft_dao::sketch::cgeom::Vec2::new(x, y)
}

fn project_clone(g: &Group) -> Group {
    Group { label: g.label, items: g.items.iter().map(clone_item).collect() }
}

fn view_clone(g: &Group) -> Group {
    project_clone(g)
}

fn clone_item(i: &Item) -> Item {
    Item {
        id: i.id,
        label: i.label,
        icon: i.icon,
        large: i.large,
        enabled: i.enabled,
        more: i.more,
        checked: i.checked,
        tone: i.tone,
        tip: i.tip.clone(),
        choices: i.choices.clone(),
        selected: i.selected,
        width: i.width,
        variants: i.variants.clone(),
    }
}

/// Pince ER11 (profil de l'atelier) et queue (banque d'outils) de chaque outil du travail, pour le
/// rappel écrit avant chaque changement d'outil.
fn set_holders(job: &mut Job, project: &Project, db: &ToolDb) {
    let atelier = db.profile_tools(&project.machine.tooldb_id).unwrap_or_default();
    let known: Vec<(String, f64, Option<f64>, Option<f64>)> = project
        .toolpaths
        .iter()
        .filter(|tp| !tp.tool.id.is_empty())
        .map(|tp| {
            let collet = atelier.iter().find(|a| a.tool.id == tp.tool.id).map(|a| a.collet_mm);
            let shank = db.tool(&tp.tool.id).ok().and_then(|t| t.shank_mm);
            (tp.tool.name.clone(), tp.tool.diameter, collet, shank)
        })
        .collect();
    job.set_holders(|t| {
        known.iter().find(|k| k.0 == t.name && (k.1 - t.diameter).abs() < 1e-9).map_or((None, None), |k| (k.2, k.3))
    });
}

/// Le cache est toujours rempli juste avant : ce chemin n'est jamais pris.
fn unreachable_job() -> &'static Job {
    static EMPTY: std::sync::OnceLock<Job> = std::sync::OnceLock::new();
    EMPTY.get_or_init(|| Job {
        title: String::new(),
        stock_size: (0.0, 0.0, 0.0),
        machine: String::new(),
        heights: makercraft_cam::Heights { safe_z: 5.0, clear_z: 1.0 },
        groups: Vec::new(),
        design: Vec::new(),
        warnings: Vec::new(),
        park: makercraft_cam::Park::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_follow_the_making_order() {
        let labels: Vec<&str> = Step::ALL.iter().map(|s| s.label()).collect();
        assert_eq!(labels, ["Machine", "Brut", "Dessin", "Parcours", "Simulation", "Usinage"]);
        for s in Step::ALL {
            assert_eq!(Step::from_index(s.index()), s);
        }
        let c = Cnc::default();
        assert_eq!(c.ribbon().len(), 6);
    }

    #[test]
    fn new_project_is_clean_and_drawing_marks_it_dirty() {
        let mut c = Cnc::default();
        assert!(!c.dirty());
        let mut req = Vec::new();
        c.command("cnc.cmd.rectang", &mut req);
        for t in ["0,0", "50,30"] {
            if let Err(e) = c.sketch.session.cmdline(t) {
                panic!("{e}");
            }
        }
        assert!(c.dirty());
        assert!(c.fit_stock_to_drawing());
        assert!((c.project.stock.size[0] - 70.0).abs() < 1e-9);
    }
}
