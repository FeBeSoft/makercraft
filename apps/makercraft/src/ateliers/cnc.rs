//! Atelier CNC : DAO/CAO (vue 2D d'un projet .c2d, d'un DXF ou de l'encombrement d'un STEP)
//! et FAO fraiseuse (parcours, vérifications, aperçu des trajectoires, export .nc).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, CornerRadius, Rect, RichText, ScrollArea, Sense, Stroke, StrokeKind, Ui, UiBuilder, pos2, vec2};
use makercraft_c2d::Project;
use makercraft_cam::ext::{Check, Level, check, load, output_names, write_outputs};
use makercraft_cam::job::tool_label;
use makercraft_cam::{Job, Move, Step};
use makercraft_geom::{Bounds2, Vec2};

use super::Atelier;
use crate::icons::Icon;
use crate::prefs::Units;
use crate::ribbon::{Group, Item, Tab, Tone};
use crate::theme::{self, Palette};
use crate::view2d::{View2d, Xform};
use crate::widgets::{self, Kind};

/// Ce que l'atelier demande à l'application.
pub enum Request {
    /// Ouvrir la fenêtre de choix de fichier.
    OpenDialog,
    /// Mémoriser un dossier utilisé.
    RecentDir(PathBuf),
    /// Message pour la barre d'état.
    Status(String),
}

/// Trajectoires d'une opération, prêtes à dessiner.
struct OpPaths {
    cuts: Vec<Vec<Vec2>>,
    rapids: Vec<[Vec2; 2]>,
}

fn op_paths(steps: &[Step], safe_z: f64) -> OpPaths {
    let mut cuts: Vec<Vec<Vec2>> = Vec::new();
    let mut rapids = Vec::new();
    let mut pos = makercraft_geom::Vec3::new(0.0, 0.0, safe_z);
    let mut current: Vec<Vec2> = Vec::new();
    for st in steps {
        let Step::Move(m) = st else { continue };
        let t = m.target();
        match m {
            Move::Feed(..) if t.z < 0.0 => {
                if current.is_empty() {
                    current.push(pos.xy());
                }
                if (t.xy() - pos.xy()).length() > 1e-9 {
                    current.push(t.xy());
                }
            }
            _ => {
                if current.len() >= 2 {
                    cuts.push(std::mem::take(&mut current));
                } else {
                    current.clear();
                }
                if matches!(m, Move::Rapid(_)) && (t.xy() - pos.xy()).length() > 1e-6 {
                    rapids.push([pos.xy(), t.xy()]);
                }
            }
        }
        pos = t;
    }
    if current.len() >= 2 {
        cuts.push(current);
    }
    OpPaths { cuts, rapids }
}

/// Projet Carbide Create chargé avec son travail de FAO.
struct C2dDoc {
    project: Project,
    job: Job,
    checks: Vec<Check>,
    /// [groupe][opération]
    paths: Vec<Vec<OpPaths>>,
}

/// Document ouvert dans l'atelier.
enum DocKind {
    C2d(Box<C2dDoc>),
    #[cfg(feature = "dao")]
    Dxf(makercraft_dao::Drawing),
    #[cfg(feature = "cao")]
    Step(makercraft_cao::StepSummary),
}

struct Doc {
    path: PathBuf,
    kind: DocKind,
}

impl Doc {
    fn name(&self) -> String {
        self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }

    fn dir(&self) -> PathBuf {
        self.path.parent().map(Path::to_path_buf).unwrap_or_default()
    }

    fn layers(&self) -> Vec<String> {
        match &self.kind {
            DocKind::C2d(d) => {
                let mut v: Vec<String> = Vec::new();
                for e in &d.project.elements {
                    if !v.contains(&e.layer_name) {
                        v.push(e.layer_name.clone());
                    }
                }
                v
            }
            #[cfg(feature = "dao")]
            DocKind::Dxf(d) => d.layers.clone(),
            #[cfg(feature = "cao")]
            DocKind::Step(_) => Vec::new(),
        }
    }

    fn bounds(&self) -> Bounds2 {
        match &self.kind {
            DocKind::C2d(d) => {
                let s = &d.project.stock;
                let mut b: Bounds2 = d.project.elements.iter().flat_map(|e| e.contours.iter().flat_map(|c| c.points.iter().copied())).collect();
                if s.width > 0.0 && s.height > 0.0 {
                    b.include(Vec2::ZERO);
                    b.include(Vec2::new(s.width, s.height));
                }
                b
            }
            #[cfg(feature = "dao")]
            DocKind::Dxf(d) => d.bounds(),
            #[cfg(feature = "cao")]
            DocKind::Step(s) => {
                let mut b = Bounds2::default();
                for body in &s.bodies {
                    b.include(Vec2::new(body.bbox_min[0], body.bbox_min[1]));
                    b.include(Vec2::new(body.bbox_max[0], body.bbox_max[1]));
                }
                b
            }
        }
    }
}

/// Formats ouverts par l'atelier (pour la fenêtre de choix de fichier).
pub fn open_filters() -> Vec<(&'static str, Vec<&'static str>)> {
    let mut all = vec!["c2d"];
    let mut v = vec![("Carbide Create", vec!["c2d"])];
    #[cfg(feature = "dao")]
    {
        all.push("dxf");
        v.push(("Dessin DXF", vec!["dxf"]));
    }
    #[cfg(feature = "cao")]
    {
        all.extend(["step", "stp"]);
        v.push(("Modèle STEP", vec!["step", "stp"]));
    }
    v.insert(0, ("Fichiers MakerCraft", all));
    v
}

pub struct Cnc {
    doc: Option<Doc>,
    view: View2d,
    hidden_layers: BTreeSet<String>,
    show_paths: bool,
    show_rapids: bool,
    /// Opération sélectionnée (groupe, opération).
    selected: Option<(usize, usize)>,
    single: bool,
    svg: bool,
    exported: Option<Result<Vec<PathBuf>, String>>,
}

impl Default for Cnc {
    fn default() -> Self {
        Self { doc: None, view: View2d::default(), hidden_layers: BTreeSet::new(), show_paths: true, show_rapids: false, selected: None, single: false, svg: false, exported: None }
    }
}

impl Cnc {
    pub fn file_name(&self) -> Option<String> {
        self.doc.as_ref().map(Doc::name)
    }

    /// Ouvre un fichier selon son extension.
    pub fn open(&mut self, path: &Path) -> Result<String, String> {
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let kind = match ext.as_str() {
            "c2d" => {
                let (project, job) = load(path, None)?;
                let checks = check(&project, &job);
                let paths = job.groups.iter().map(|g| g.ops.iter().map(|o| op_paths(&o.steps, job.heights.safe_z)).collect()).collect();
                DocKind::C2d(Box::new(C2dDoc { project, job, checks, paths }))
            }
            #[cfg(feature = "dao")]
            "dxf" => DocKind::Dxf(makercraft_dao::read_dxf(path).map_err(|e| e.to_string())?),
            #[cfg(feature = "cao")]
            "step" | "stp" => DocKind::Step(makercraft_cao::step_summary(path)?),
            _ => return Err(format!("format non pris en charge : .{ext}")),
        };
        let doc = Doc { path: path.to_path_buf(), kind };
        self.view.fit(doc.bounds());
        self.hidden_layers.clear();
        self.selected = None;
        self.exported = None;
        let msg = format!("{} ouvert", doc.name());
        self.doc = Some(doc);
        Ok(msg)
    }

    fn c2d(&self) -> Option<&C2dDoc> {
        match self.doc.as_ref().map(|d| &d.kind) {
            Some(DocKind::C2d(c)) => Some(c),
            _ => None,
        }
    }

    /// Onglets du ruban pour l'atelier `at` (DAO/CAO ou FAO).
    pub fn ribbon(&self, at: Atelier) -> Vec<Tab> {
        let has = self.doc.is_some();
        let c2d = self.c2d().is_some();
        let fao = at == Atelier::CncFao;
        let mut view = vec![Item::large("cnc.ajuster", "Ajuster", Icon::Fit).enabled(has).tip("Ajuster la vue au document")];
        if fao {
            view.push(Item::small("cnc.trajectoires", "Trajectoires", Icon::Paths).enabled(c2d).checked(self.show_paths).tip("Afficher les trajectoires d'usinage"));
            view.push(Item::small("cnc.rapides", "Rapides", Icon::Rapids).enabled(c2d).checked(self.show_rapids).tip("Afficher les déplacements rapides"));
        }
        let mut tabs = vec![Tab::new(
            "Accueil",
            vec![
                Group { label: "Fichier", items: vec![Item::large("cnc.ouvrir", "Ouvrir", Icon::Open).tone(Tone::Design).tip("Ouvrir un projet .c2d, un dessin DXF ou un modèle STEP (Ctrl+O)")] },
                Group { label: "Vue", items: view },
            ],
        )];
        if fao {
            let ok = self.c2d().is_some_and(|c| !blocking(c));
            tabs.push(Tab::new(
                "Usinage",
                vec![Group {
                    label: "G-code",
                    items: vec![
                        Item::large("cnc.exporter", "Exporter .nc", Icon::Export).tone(Tone::Component).enabled(ok).tip(if c2d && !ok {
                            "Export impossible : corriger les points en rouge"
                        } else {
                            "Écrire les fichiers G-code pour Grbl"
                        }),
                        Item::small("cnc.un_fichier", "Un seul fichier", Icon::File).enabled(c2d).checked(self.single).tip("Pause M0 aux changements d'outil"),
                        Item::small("cnc.svg", "Aperçu SVG", Icon::Image).enabled(c2d).checked(self.svg).tip("Écrire aussi un aperçu SVG"),
                    ],
                }],
            ));
        }
        tabs
    }

    /// Exécute une commande du ruban.
    pub fn command(&mut self, id: &str, req: &mut Vec<Request>) {
        match id {
            "cnc.ouvrir" => req.push(Request::OpenDialog),
            "cnc.ajuster" => {
                if let Some(d) = &self.doc {
                    self.view.fit(d.bounds());
                }
            }
            "cnc.trajectoires" => self.show_paths = !self.show_paths,
            "cnc.rapides" => self.show_rapids = !self.show_rapids,
            "cnc.un_fichier" => self.single = !self.single,
            "cnc.svg" => self.svg = !self.svg,
            "cnc.exporter" => self.export_dialog(req),
            _ => {}
        }
    }

    /// Demande le dossier puis écrit les .nc du projet ouvert.
    fn export_dialog(&mut self, req: &mut Vec<Request>) {
        let Some(Doc { kind: DocKind::C2d(c), path }) = &self.doc else { return };
        if blocking(c) {
            return;
        }
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let Some(out) = rfd::FileDialog::new().set_title("Dossier des fichiers .nc").set_directory(&dir).pick_folder() else { return };
        let r = write_outputs(&c.job, &out, self.single, self.svg);
        req.push(Request::Status(match &r {
            Ok(f) => format!("{} fichier(s) .nc écrit(s) dans {}", f.len(), out.display()),
            Err(e) => format!("Échec de l'export : {e}"),
        }));
        req.push(Request::RecentDir(out));
        self.exported = Some(r);
    }

    pub fn ui(&mut self, ui: &mut Ui, pal: &Palette, units: Units, at: Atelier, req: &mut Vec<Request>) {
        let full = ui.available_rect_before_wrap();
        ui.allocate_rect(full, Sense::hover());
        let side_w = 340.0_f32.min(full.width() * 0.4);
        let side = Rect::from_min_size(full.min, vec2(side_w, full.height()));
        let main = Rect::from_min_max(pos2(side.right() + 16.0, full.top()), full.max);

        let mut side_ui = ui.new_child(UiBuilder::new().max_rect(side));
        ScrollArea::vertical().id_salt("cnc-side").auto_shrink([false, false]).show(&mut side_ui, |ui| {
            ui.set_max_width(side_w - 14.0);
            ui.spacing_mut().item_spacing.y = 14.0;
            match at {
                Atelier::CncFao => self.fao_panel(ui, pal, units, req),
                _ => self.dao_panel(ui, pal, units, req),
            }
        });

        let mut main_ui = ui.new_child(UiBuilder::new().max_rect(main));
        let show_paths = at == Atelier::CncFao && self.show_paths;
        let show_rapids = at == Atelier::CncFao && self.show_rapids;
        let (doc, hidden, selected) = (&self.doc, &self.hidden_layers, self.selected);
        self.view.show(&mut main_ui, pal, units, |p, xf| {
            if let Some(d) = doc {
                draw_doc(p, xf, pal, d, hidden, show_paths, show_rapids, selected);
            }
        });
    }

    fn doc_card(&self, ui: &mut Ui, pal: &Palette, units: Units) {
        widgets::card(ui, pal, Some("Document"), |ui| {
            let Some(d) = &self.doc else {
                ui.label(RichText::new("Aucun document ouvert.").color(pal.weak));
                let formats: Vec<String> = open_filters().into_iter().skip(1).map(|(n, e)| format!("{n} (.{})", e.join(", ."))).collect();
                ui.label(RichText::new(format!("Formats : {}.", formats.join(", "))).size(13.0).color(pal.weak));
                return;
            };
            ui.label(RichText::new(d.name()).font(theme::bold(14.5)));
            ui.label(RichText::new(d.dir().display().to_string()).size(12.0).color(pal.weak));
            let b = d.bounds();
            match &d.kind {
                DocKind::C2d(c) => {
                    let s = &c.project.stock;
                    widgets::caption(ui, pal, "Brut (largeur x hauteur x épaisseur)");
                    ui.label(format!("{} x {} x {}", units.fmt(s.width), units.fmt(s.height), units.fmt(s.thickness)));
                    widgets::caption(ui, pal, "Contenu");
                    ui.label(format!("{} forme(s), {} parcours", c.project.elements.len(), c.project.toolpaths.len()));
                }
                #[cfg(feature = "dao")]
                DocKind::Dxf(x) => {
                    widgets::caption(ui, pal, "Dessin DXF lu par CadCraft");
                    ui.label(format!("{} tracé(s)", x.paths.len()));
                    let unit = match x.mm_per_unit {
                        k if (k - 25.4).abs() < 1e-9 => "pouce",
                        k if (k - 304.8).abs() < 1e-9 => "pied",
                        k if (k - 10.0).abs() < 1e-9 => "cm",
                        k if (k - 1000.0).abs() < 1e-9 => "m",
                        _ => "mm",
                    };
                    ui.label(RichText::new(format!("Unité déclarée par le fichier : {unit}")).size(12.5).color(pal.weak));
                    if !b.is_empty() {
                        ui.label(format!("Encombrement {} x {}", units.fmt(b.size().x), units.fmt(b.size().y)));
                    }
                    if !x.skipped.is_empty() {
                        let s: Vec<String> = x.skipped.iter().map(|(k, n)| format!("{k} ({n})")).collect();
                        widgets::status_line(ui, pal, Icon::Warn, pal.warn, &format!("Non dessinés : {}", s.join(", ")));
                    }
                }
                #[cfg(feature = "cao")]
                DocKind::Step(s) => {
                    widgets::caption(ui, pal, "Modèle STEP lu par le noyau SolidCraft (vue de dessus : encombrement)");
                    for body in &s.bodies {
                        let size = [body.bbox_max[0] - body.bbox_min[0], body.bbox_max[1] - body.bbox_min[1], body.bbox_max[2] - body.bbox_min[2]];
                        ui.label(RichText::new(&body.name).font(theme::bold(13.5)));
                        ui.label(
                            RichText::new(format!(
                                "{} x {} x {}  -  {:.0} mm3{}",
                                units.fmt(size[0]),
                                units.fmt(size[1]),
                                units.fmt(size[2]),
                                body.volume,
                                if body.closed { "" } else { "  -  corps ouvert" }
                            ))
                            .size(12.5)
                            .color(pal.weak),
                        );
                    }
                    for w in &s.warnings {
                        widgets::status_line(ui, pal, Icon::Warn, pal.warn, w);
                    }
                }
            }
        });
    }

    fn dao_panel(&mut self, ui: &mut Ui, pal: &Palette, units: Units, _req: &mut Vec<Request>) {
        self.doc_card(ui, pal, units);
        let layers = self.doc.as_ref().map(Doc::layers).unwrap_or_default();
        if !layers.is_empty() {
            widgets::card(ui, pal, Some("Calques"), |ui| {
                for l in &layers {
                    let mut visible = !self.hidden_layers.contains(l);
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                        ui.painter().rect_filled(r, CornerRadius::same(3), layer_color(pal, &layers, l));
                        if ui.checkbox(&mut visible, l.as_str()).changed() {
                            if visible {
                                self.hidden_layers.remove(l);
                            } else {
                                self.hidden_layers.insert(l.clone());
                            }
                        }
                    });
                }
            });
        }
        widgets::card(ui, pal, Some("Vue"), |ui| {
            ui.label(RichText::new("Molette : zoom autour du curseur. Glisser : déplacer. Origine X0 Y0 = coin avant gauche du brut.").size(13.0).color(pal.weak));
        });
    }

    fn fao_panel(&mut self, ui: &mut Ui, pal: &Palette, units: Units, req: &mut Vec<Request>) {
        self.doc_card(ui, pal, units);
        let Some(Doc { kind: DocKind::C2d(c), .. }) = &self.doc else {
            widgets::card(ui, pal, Some("Parcours"), |ui| {
                ui.label(
                    RichText::new("La FAO de cette version part d'un projet Carbide Create (.c2d) : ouvrez-en un avec Fichier > Ouvrir.").color(pal.weak),
                );
            });
            return;
        };
        // parcours groupés par outil
        let mut clicked = None;
        widgets::card(ui, pal, Some("Parcours"), |ui| {
            if c.job.groups.is_empty() {
                ui.label(RichText::new("Aucun parcours à produire.").color(pal.weak));
            }
            for (gi, g) in c.job.groups.iter().enumerate() {
                widgets::pill(ui, &short_tool(&g.tool), pal.orange_soft, pal.orange).on_hover_text(tool_label(&g.tool));
                for (oi, op) in g.ops.iter().enumerate() {
                    let st = g.stats(c.job.heights);
                    let sel = self.selected == Some((gi, oi));
                    let text = format!("{}  -  {}", op.name, op.kind);
                    if ui.selectable_label(sel, text).on_hover_text(format!("Longueur usinée du groupe : {}", units.fmt(st.cut_length))).clicked() {
                        clicked = Some(if sel { None } else { Some((gi, oi)) });
                    }
                }
            }
        });
        if let Some(s) = clicked {
            self.selected = s;
        }
        widgets::card(ui, pal, Some("Vérifications"), |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            for ch in &c.checks {
                let (icon, color) = match ch.level {
                    Level::Ok => (Icon::Ok, pal.ok),
                    Level::Info => (Icon::Info, pal.blue),
                    Level::Warn => (Icon::Warn, pal.warn),
                    Level::Error => (Icon::Error, pal.bad),
                };
                widgets::status_line(ui, pal, icon, color, &ch.text);
            }
        });
        let blocking = blocking(c);
        let mut export = false;
        widgets::card(ui, pal, Some("Export G-code"), |ui| {
            ui.checkbox(&mut self.single, "Un seul fichier (pause M0 aux changements d'outil)");
            ui.checkbox(&mut self.svg, "Écrire aussi un aperçu SVG");
            widgets::caption(ui, pal, "Fichiers produits");
            for n in output_names(&c.job, self.single) {
                ui.label(RichText::new(n).font(theme::mono(12.5)));
            }
            ui.add_space(4.0);
            let b = widgets::button(ui, pal, Some(Icon::Export), "Exporter les .nc...", Kind::Component, !blocking);
            if blocking {
                b.on_hover_text("Export impossible : corriger les points en rouge");
            } else if b.clicked() {
                export = true;
            }
            match &self.exported {
                Some(Ok(files)) => widgets::status_line(ui, pal, Icon::Ok, pal.ok, &format!("{} fichier(s) écrit(s)", files.len())),
                Some(Err(e)) => widgets::status_line(ui, pal, Icon::Error, pal.bad, e),
                None => {}
            }
        });
        if export {
            self.export_dialog(req);
        }
    }
}

/// Vrai si l'export est impossible (erreur bloquante ou rien à produire).
fn blocking(c: &C2dDoc) -> bool {
    c.checks.iter().any(|ch| ch.level == Level::Error) || c.job.groups.is_empty()
}

/// Étiquette courte d'un outil : « T302 - V 30° - D6.35 » (le détail est dans l'info-bulle).
fn short_tool(t: &makercraft_c2d::Tool) -> String {
    use makercraft_c2d::ToolKind;
    let kind = match t.kind {
        ToolKind::Flat => "plate".to_owned(),
        ToolKind::Ball => "boule".to_owned(),
        ToolKind::VBit => format!("V {:.0}°", t.angle),
        ToolKind::Other(k) => format!("type {k}"),
    };
    format!("T{}  -  {kind}  -  D{:.2}", t.number, t.diameter)
}

/// Couleur d'un calque : bleus du design, déclinés.
fn layer_color(pal: &Palette, layers: &[String], name: &str) -> Color32 {
    const SHADES: [f32; 4] = [1.0, 0.75, 0.55, 0.4];
    let i = layers.iter().position(|l| l == name).unwrap_or(0);
    pal.drawing.gamma_multiply(SHADES[i % SHADES.len()])
}

#[allow(clippy::too_many_arguments)]
fn draw_doc(
    p: &egui::Painter,
    xf: &Xform,
    pal: &Palette,
    d: &Doc,
    hidden: &BTreeSet<String>,
    show_paths: bool,
    show_rapids: bool,
    selected: Option<(usize, usize)>,
) {
    let layers = d.layers();
    match &d.kind {
        DocKind::C2d(c) => {
            let s = &c.project.stock;
            if s.width > 0.0 && s.height > 0.0 {
                let r = Rect::from_two_pos(xf.to_screen(Vec2::ZERO), xf.to_screen(Vec2::new(s.width, s.height)));
                p.rect_filled(r, CornerRadius::ZERO, pal.stock_fill);
                p.rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.5, pal.stock_line), StrokeKind::Middle);
            }
            for e in &c.project.elements {
                if hidden.contains(&e.layer_name) {
                    continue;
                }
                let color = layer_color(pal, &layers, &e.layer_name);
                for ct in &e.contours {
                    xf.polyline(p, &ct.points, ct.closed, Stroke::new(1.4, color));
                }
            }
            if show_paths || show_rapids {
                for (gi, g) in c.paths.iter().enumerate() {
                    for (oi, op) in g.iter().enumerate() {
                        let strong = selected.is_none_or(|s| s == (gi, oi));
                        if show_rapids {
                            for seg in &op.rapids {
                                p.add(egui::Shape::dashed_line(&[xf.to_screen(seg[0]), xf.to_screen(seg[1])], Stroke::new(1.0, pal.rapid), 6.0, 4.0));
                            }
                        }
                        if show_paths {
                            let color = if strong { pal.orange } else { theme::alpha(pal.orange, 0.3) };
                            for cut in &op.cuts {
                                xf.polyline(p, cut, false, Stroke::new(if strong { 1.8 } else { 1.2 }, color));
                            }
                        }
                    }
                }
            }
        }
        #[cfg(feature = "dao")]
        DocKind::Dxf(x) => {
            for path in &x.paths {
                if hidden.contains(&path.layer) {
                    continue;
                }
                xf.polyline(p, &path.points, path.closed, Stroke::new(1.4, layer_color(pal, &layers, &path.layer)));
            }
        }
        #[cfg(feature = "cao")]
        DocKind::Step(s) => {
            for body in &s.bodies {
                let r = Rect::from_two_pos(
                    xf.to_screen(Vec2::new(body.bbox_min[0], body.bbox_min[1])),
                    xf.to_screen(Vec2::new(body.bbox_max[0], body.bbox_max[1])),
                );
                p.rect_filled(r, CornerRadius::same(2), theme::alpha(pal.drawing, 0.12));
                p.rect_stroke(r, CornerRadius::same(2), Stroke::new(1.5, pal.drawing), StrokeKind::Middle);
                p.text(r.left_top() + vec2(6.0, 6.0), egui::Align2::LEFT_TOP, &body.name, theme::regular(12.0), pal.drawing);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use makercraft_geom::Vec3;

    #[test]
    fn op_paths_split_cuts_and_rapids() {
        let steps = vec![
            Step::Move(Move::Rapid(Vec3::new(0.0, 0.0, 5.0))),
            Step::Move(Move::Rapid(Vec3::new(10.0, 0.0, 5.0))),
            Step::Move(Move::Feed(Vec3::new(10.0, 0.0, -1.0), 300.0)),
            Step::Move(Move::Feed(Vec3::new(20.0, 0.0, -1.0), 800.0)),
            Step::Move(Move::Feed(Vec3::new(20.0, 10.0, -1.0), 800.0)),
            Step::Move(Move::Rapid(Vec3::new(20.0, 10.0, 5.0))),
            Step::Comment("fin".into()),
        ];
        let p = op_paths(&steps, 5.0);
        assert_eq!(p.cuts.len(), 1);
        assert_eq!(p.cuts[0], vec![Vec2::new(10.0, 0.0), Vec2::new(20.0, 0.0), Vec2::new(20.0, 10.0)]);
        assert_eq!(p.rapids.len(), 1);
    }

    #[test]
    fn unknown_format_is_refused() {
        let mut c = Cnc::default();
        assert!(c.open(Path::new("dessin.xyz")).is_err());
        assert!(c.file_name().is_none());
    }

    #[test]
    fn filters_list_c2d_first() {
        let f = open_filters();
        assert!(f[0].1.contains(&"c2d"));
    }
}
