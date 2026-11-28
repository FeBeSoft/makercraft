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
use crate::ribbon::{Group, Item, Tab, Tone};
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
                            draw("cnc.cmd.circle", "Cercle", Icon::Circle),
                            draw("cnc.cmd.arc", "Arc", Icon::Arc),
                            draw("cnc.cmd.ellipse", "Ellipse", Icon::Ellipse).more(),
                            draw("cnc.cmd.polygon", "Polygone", Icon::Polygon).more(),
                            draw("cnc.cmd.spline", "Spline", Icon::Spline).more(),
                            draw("cnc.etoile", "Étoile", Icon::Star).more().enabled(false).tip("À venir"),
                            draw("cnc.oblong", "Oblong", Icon::Oblong).more().enabled(false).tip("À venir"),
                        ],
                    },
                    Group {
                        label: "Modifier",
                        items: vec![
                            modify("cnc.cmd.move", "Déplacer", Icon::Move),
                            modify("cnc.cmd.copy", "Copier", Icon::Copy),
                            modify("cnc.cmd.rotate", "Rotation", Icon::Rotate),
                            modify("cnc.cmd.mirror", "Miroir", Icon::Mirror),
                            modify("cnc.cmd.offset", "Décaler", Icon::Offset),
                            modify("cnc.cmd.fillet", "Raccord", Icon::Fillet),
                            modify("cnc.cmd.scale", "Échelle", Icon::Scale).more(),
                            modify("cnc.cmd.trim", "Ajuster", Icon::Trim).more(),
                            modify("cnc.cmd.break", "Couper", Icon::Break).more(),
                            modify("cnc.cmd.extend", "Prolonger", Icon::Extend).more(),
                            modify("cnc.cmd.join", "Joindre", Icon::Join).more(),
                            modify("cnc.cmd.explode", "Décomposer", Icon::Explode).more(),
                            modify("cnc.cmd.chamfer", "Chanfrein", Icon::Chamfer).more(),
                            modify("cnc.cmd.arrayrect", "Réseau", Icon::Array).more(),
                            modify("cnc.cmd.erase", "Effacer", Icon::Trash).more(),
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
