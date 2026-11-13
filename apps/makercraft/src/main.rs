//! MakerCraft : application de bureau. Coquille à ateliers (CNC, Laser, PCB, Machines) qui
//! charge les extensions compilées avec elle (machine, c2d, cam, dao, cao).
//!
//! Un fichier (.c2d, .dxf, .step, .nc) peut être passé en argument ou déposé sur l'icône.

// pas de console noire derrière la fenêtre (sauf en debug, pour voir les messages)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod ateliers;
mod hote;
mod icons;
mod prefs;
mod theme;
mod view2d;
mod widgets;

use std::path::PathBuf;

use eframe::egui;

fn main() -> eframe::Result {
    let files: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("MakerCraft")
        .with_app_id("fr.fe-be.makercraft")
        .with_inner_size([1360.0, 860.0])
        .with_min_inner_size([1024.0, 680.0])
        .with_drag_and_drop(true);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(app::ICON_PNG) {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        "MakerCraft",
        options,
        Box::new(move |cc| {
            theme::setup_fonts(&cc.egui_ctx);
            Ok(Box::new(app::App::new(files)))
        }),
    )
}
