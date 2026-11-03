//! c2d_to_nc : glisser un projet Carbide Create (.c2d) dans la fenêtre, vérifier son contenu,
//! exporter le G-code Grbl (.nc) dans le même dossier que le projet.
//!
//! Le fichier peut aussi être déposé sur l'icône de l'exécutable (passé en argument).

// pas de console noire derrière la fenêtre (sauf en debug, pour voir les messages)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, RichText};
use makercraft_c2d::Project;
use makercraft_cam::Job;
use makercraft_cam::ext::{Check, Level, check, load, output_names, write_outputs};
use makercraft_cam::job::tool_label;

const ICON_PNG: &[u8] = include_bytes!("../../../assets/branding/makercraft-256.png");

struct Loaded {
    path: PathBuf,
    project: Project,
    job: Job,
    checks: Vec<Check>,
}

impl Loaded {
    fn blocking(&self) -> bool {
        self.checks.iter().any(|c| c.level == Level::Error) || self.job.groups.is_empty()
    }

    fn dir(&self) -> PathBuf {
        self.path.parent().map(Path::to_path_buf).unwrap_or_default()
    }
}

#[derive(Default)]
struct App {
    loaded: Option<Loaded>,
    error: Option<String>,
    single: bool,
    preview: bool,
    exported: Option<Result<Vec<PathBuf>, String>>,
}

impl App {
    fn open(&mut self, path: PathBuf) {
        self.exported = None;
        self.error = None;
        self.loaded = None;
        if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("c2d")) {
            self.error = Some(format!("« {} » n'est pas un projet Carbide Create (.c2d)", file_name(&path)));
            return;
        }
        match load(&path, None) {
            Ok((project, job)) => {
                let checks = check(&project, &job);
                self.loaded = Some(Loaded { path, project, job, checks });
            }
            Err(e) => self.error = Some(format!("{} : {e}", file_name(&path))),
        }
    }

    fn export(&mut self) {
        let Some(l) = &self.loaded else { return };
        self.exported = Some(write_outputs(&l.job, &l.dir(), self.single, self.preview));
    }
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn level_style(l: Level) -> (&'static str, Color32) {
    match l {
        Level::Ok => ("✔", Color32::from_rgb(46, 160, 67)),
        Level::Info => ("ℹ", Color32::from_rgb(56, 132, 255)),
        Level::Warn => ("⚠", Color32::from_rgb(210, 140, 0)),
        Level::Error => ("✖", Color32::from_rgb(220, 50, 50)),
    }
}

fn open_folder(dir: &Path) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(dir).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(dir).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // fichiers déposés
        let dropped: Vec<PathBuf> =
            ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        if let Some(p) = dropped.into_iter().next() {
            self.open(p);
        }
        let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());

        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("page").auto_shrink([false, false]).show(ui, |ui| self.page(ui, hovering));
        });
    }
}

impl App {
    fn page(&mut self, ui: &mut egui::Ui, hovering: bool) {
        {
            ui.horizontal(|ui| {
                ui.heading(RichText::new("c2d_to_nc").strong());
                ui.label(RichText::new("projet Carbide Create vers G-code Grbl").weak());
            });
            ui.add_space(6.0);

            // zone de dépôt
            let stroke_color = if hovering { Color32::from_rgb(56, 132, 255) } else { ui.visuals().widgets.noninteractive.bg_stroke.color };
            egui::Frame::group(ui.style()).stroke(egui::Stroke::new(if hovering { 2.5 } else { 1.0 }, stroke_color)).inner_margin(14.0).show(
                ui,
                |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.vertical_centered(|ui| {
                        let msg = if hovering { "Relâchez pour ouvrir le projet" } else { "Glissez un fichier .c2d ici" };
                        ui.label(RichText::new(msg).size(18.0));
                        ui.add_space(4.0);
                        if ui.button("Ouvrir un fichier…").clicked()
                            && let Some(p) = rfd::FileDialog::new().add_filter("Projet Carbide Create", &["c2d"]).pick_file()
                        {
                            self.open(p);
                        }
                    });
                },
            );
            ui.add_space(8.0);

            if let Some(e) = &self.error {
                ui.colored_label(level_style(Level::Error).1, format!("✖ {e}"));
            }

            let Some(l) = &self.loaded else {
                ui.add_space(10.0);
                ui.label(RichText::new("Le G-code produit est du Grbl standard (lisible par Candle, UGS, LaserGRBL, Carbide Motion…).").weak());
                return;
            };

            ui.label(RichText::new(file_name(&l.path)).strong().size(16.0));
            ui.label(RichText::new(l.dir().display().to_string()).weak().small());
            ui.add_space(6.0);

            // vérifications
            ui.label(RichText::new("Vérifications").strong());
            egui::ScrollArea::vertical().id_salt("checks").max_height(220.0).auto_shrink([false, true]).show(ui, |ui| {
                for c in &l.checks {
                    let (icon, color) = level_style(c.level);
                    ui.horizontal_wrapped(|ui| {
                        ui.colored_label(color, icon);
                        ui.label(&c.text);
                    });
                }
            });
            ui.add_space(8.0);

            // fichiers prévus
            if !l.job.groups.is_empty() {
                ui.label(RichText::new("G-code à produire").strong());
                let names = output_names(&l.job, self.single);
                let mut total = 0.0;
                // un fichier = son nom, puis l'outil et la durée en dessous (pas de débordement)
                let item = |ui: &mut egui::Ui, n: usize, name: &str, detail: String| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(format!("{n}.")).strong());
                        ui.label(RichText::new(name).monospace());
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.add_space(18.0);
                        ui.label(RichText::new(detail).weak());
                    });
                };
                if self.single || l.job.groups.len() <= 1 {
                    let s: f64 = l.job.groups.iter().map(|g| g.stats(l.job.heights).seconds).sum();
                    total = s;
                    let tools: Vec<String> = l.job.groups.iter().map(|g| format!("T{}", g.tool.number)).collect();
                    item(
                        ui,
                        1,
                        names.first().map_or("", String::as_str),
                        format!("outils {} - pause M0 aux changements - ≈ {:.0} min", tools.join(", "), s / 60.0),
                    );
                } else {
                    for (i, (g, name)) in l.job.groups.iter().zip(&names).enumerate() {
                        let s = g.stats(l.job.heights).seconds;
                        total += s;
                        item(ui, i + 1, name, format!("{} - ≈ {:.0} min", tool_label(&g.tool), s / 60.0));
                    }
                }
                ui.label(
                    RichText::new(format!(
                        "Durée totale estimée ≈ {:.0} min (hors changements d'outil). Zéro : X0 Y0 coin avant gauche, Z0 dessus du brut.",
                        total / 60.0
                    ))
                    .weak()
                    .small(),
                );
                if l.project.has_encrypted_gcode() {
                    ui.label(RichText::new("Les parcours sont recalculés à partir du dessin et des réglages du projet.").weak().small());
                }
                ui.add_space(6.0);
                ui.checkbox(&mut self.single, "Un seul fichier (pause M0 à chaque changement d'outil)");
                ui.checkbox(&mut self.preview, "Ajouter un aperçu SVG des trajectoires");
            }
            ui.add_space(10.0);

            let blocking = l.blocking();
            let dir = l.dir();
            ui.horizontal(|ui| {
                let export = ui.add_enabled(
                    !blocking,
                    egui::Button::new(RichText::new("Exporter les .nc").size(17.0).strong()).min_size(egui::vec2(180.0, 34.0)),
                );
                if blocking {
                    ui.colored_label(level_style(Level::Error).1, "export impossible : voir les ✖ ci-dessus");
                }
                if export.clicked() {
                    self.export();
                }
            });

            match &self.exported {
                Some(Ok(files)) => {
                    ui.add_space(6.0);
                    ui.colored_label(level_style(Level::Ok).1, format!("✔ {} fichier(s) écrit(s) dans le dossier du projet :", files.len()));
                    for f in files {
                        ui.label(format!("   {}", file_name(f)));
                    }
                    if ui.button("Ouvrir le dossier").clicked() {
                        open_folder(&dir);
                    }
                }
                Some(Err(e)) => {
                    ui.colored_label(level_style(Level::Error).1, format!("✖ échec de l'export : {e}"));
                }
                None => {}
            }
        }
    }
}

fn main() -> eframe::Result {
    let mut app = App { preview: false, ..App::default() };
    if let Some(arg) = std::env::args_os().nth(1) {
        app.open(PathBuf::from(arg));
    }
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("c2d_to_nc — MakerCraft")
        .with_inner_size([720.0, 780.0])
        .with_min_inner_size([520.0, 420.0])
        .with_drag_and_drop(true);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(ICON_PNG) {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("c2d_to_nc", options, Box::new(move |_cc| Ok(Box::new(app))))
}
