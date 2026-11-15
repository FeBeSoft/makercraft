//! Bibliothèque d'outils MakerCraft (application autonome).
//! Choisir un outil, un matériau et une machine : paramètres calculés à côté des valeurs
//! éprouvées, enregistrement des réglages et des outils personnels.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use eframe::egui::{self, Color32, RichText};
use makercraft_tooldb::{CutParams, Machine, Origin, Tool, ToolDb, UserCut, supported};

const ICON_PNG: &[u8] = include_bytes!("../../../assets/branding/makercraft-256.png");
const ACCENT: Color32 = Color32::from_rgb(0xf9, 0x73, 0x16);
const PERSO: Color32 = Color32::from_rgb(0x8b, 0x5c, 0xf6);
const TYPES: [(&str, &str); 7] = [
    ("flat", "Fraise plate"),
    ("ballnose", "Fraise hémisphérique"),
    ("bullnose", "Fraise torique"),
    ("vbit", "V-bit"),
    ("engraver", "Pointe à graver"),
    ("drill", "Foret"),
    ("form", "Fraise de forme"),
];

fn type_label(k: &str) -> &str {
    TYPES.iter().find(|(id, _)| *id == k).map_or(k, |(_, l)| l)
}

fn data_dir() -> PathBuf {
    std::env::var_os("APPDATA").map_or_else(|| PathBuf::from("."), PathBuf::from).join("MakerCraft")
}

/// Champ perso éditable : case cochée = valeur saisie, sinon calculée.
#[derive(Default, Clone, Copy)]
struct Field {
    on: bool,
    v: f64,
}

impl Field {
    fn get(self) -> Option<f64> {
        self.on.then_some(self.v)
    }
    fn from(o: Option<f64>, fallback: f64) -> Self {
        Self { on: o.is_some(), v: o.unwrap_or(fallback) }
    }
}

#[derive(Default)]
struct Edit {
    rpm: Field,
    feed: Field,
    plunge: Field,
    stepdown: Field,
    stepover: Field,
    note: String,
}

struct NewTool {
    open: bool,
    tool: Tool,
}

struct App {
    db: Option<ToolDb>,
    error: Option<String>,
    status: String,
    tools: Vec<Tool>,
    machines: Vec<Machine>,
    materials: Vec<(String, String)>,
    filter: String,
    sel_tool: Option<String>,
    sel_material: String,
    sel_machine: String,
    computed: Option<CutParams>,
    result: Option<CutParams>,
    edit: Edit,
    has_user: bool,
    new_tool: NewTool,
    dirty: bool,
}

impl App {
    fn new() -> Self {
        let mut app = Self {
            db: None,
            error: None,
            status: String::new(),
            tools: vec![],
            machines: vec![],
            materials: vec![],
            filter: String::new(),
            sel_tool: None,
            sel_material: "mdf".into(),
            sel_machine: "cnc3018".into(),
            computed: None,
            result: None,
            edit: Edit::default(),
            has_user: false,
            new_tool: NewTool { open: false, tool: blank_tool() },
            dirty: true,
        };
        match ToolDb::open(&data_dir()) {
            Ok(db) => {
                app.db = Some(db);
                app.reload_lists();
            }
            Err(e) => app.error = Some(e.to_string()),
        }
        app
    }

    fn reload_lists(&mut self) {
        let Some(db) = &self.db else { return };
        let r = (|| -> makercraft_tooldb::Result<_> { Ok((db.tools()?, db.machines()?, db.materials()?)) })();
        match r {
            Ok((t, m, mat)) => {
                self.tools = t;
                self.machines = m;
                self.materials = mat.into_iter().filter(|(id, _)| id != "generique").collect();
                if self.sel_tool.as_ref().is_none_or(|id| !self.tools.iter().any(|t| &t.id == id)) {
                    self.sel_tool = self.tools.first().map(|t| t.id.clone());
                }
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        self.dirty = true;
    }

    /// Recalcule après un changement de sélection, recharge les valeurs perso dans l'éditeur.
    fn refresh(&mut self) {
        self.dirty = false;
        let (Some(db), Some(tid)) = (&self.db, &self.sel_tool) else { return };
        let (mat, mac) = (&self.sel_material, &self.sel_machine);
        let r = (|| -> makercraft_tooldb::Result<_> {
            Ok((db.compute_for(tid, mat, mac)?, db.suggest(tid, mat, mac)?, db.user_cut(tid, mat, mac)?))
        })();
        match r {
            Ok((c, s, u)) => {
                let base = s.clone().or_else(|| c.clone());
                let f = |o: Option<f64>, g: fn(&CutParams) -> f64| Field::from(o, base.as_ref().map_or(0.0, g));
                self.has_user = u.is_some();
                let u = u.unwrap_or_default();
                self.edit = Edit {
                    rpm: f(u.rpm, |p| p.rpm),
                    feed: f(u.feed_mm_min, |p| p.feed_mm_min),
                    plunge: f(u.plunge_mm_min, |p| p.plunge_mm_min),
                    stepdown: f(u.stepdown_mm, |p| p.stepdown_mm),
                    stepover: f(u.stepover_mm, |p| p.stepover_mm.unwrap_or(0.0)),
                    note: u.note.unwrap_or_default(),
                };
                self.computed = c;
                self.result = s;
            }
            Err(e) => self.status = e.to_string(),
        }
    }

    fn save_user(&mut self) {
        let (Some(db), Some(tid)) = (&self.db, &self.sel_tool) else { return };
        let e = &self.edit;
        let u = UserCut {
            rpm: e.rpm.get(),
            feed_mm_min: e.feed.get(),
            plunge_mm_min: e.plunge.get(),
            stepdown_mm: e.stepdown.get(),
            stepover_mm: e.stepover.get(),
            note: (!e.note.trim().is_empty()).then(|| e.note.trim().to_owned()),
        };
        let empty = u == UserCut::default();
        let r = if empty {
            db.delete_user_cut(tid, &self.sel_material, &self.sel_machine)
        } else {
            db.save_user_cut(tid, &self.sel_material, &self.sel_machine, &u)
        };
        self.status = match r {
            Ok(()) if empty => "Valeurs perso effacées".into(),
            Ok(()) => "Valeurs perso enregistrées".into(),
            Err(e) => e.to_string(),
        };
        self.dirty = true;
    }

    fn clear_user(&mut self) {
        let (Some(db), Some(tid)) = (&self.db, &self.sel_tool) else { return };
        self.status = match db.delete_user_cut(tid, &self.sel_material, &self.sel_machine) {
            Ok(()) => "Valeurs perso effacées".into(),
            Err(e) => e.to_string(),
        };
        self.dirty = true;
    }

    fn selected(&self) -> Option<&Tool> {
        self.sel_tool.as_ref().and_then(|id| self.tools.iter().find(|t| &t.id == id))
    }

    fn is_user_tool(&self, id: &str) -> bool {
        self.db.as_ref().is_some_and(|db| db.is_user_tool(id).unwrap_or(false))
    }

    // --- panneaux -----------------------------------------------------------

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Bibliothèque d'outils").heading().color(ACCENT));
            ui.separator();
            ui.label("Machine");
            let cur = self.machines.iter().find(|m| m.id == self.sel_machine).map_or(self.sel_machine.clone(), |m| m.name.clone());
            egui::ComboBox::from_id_salt("machine").width(330.0).selected_text(cur).show_ui(ui, |ui| {
                for m in &self.machines {
                    if ui.selectable_value(&mut self.sel_machine, m.id.clone(), &m.name).changed() {
                        self.dirty = true;
                    }
                }
            });
            ui.label("Matériau");
            let cur = self.materials.iter().find(|(id, _)| *id == self.sel_material).map_or(self.sel_material.clone(), |(_, n)| n.clone());
            egui::ComboBox::from_id_salt("materiau").width(230.0).selected_text(cur).show_ui(ui, |ui| {
                for (id, name) in &self.materials {
                    if ui.selectable_value(&mut self.sel_material, id.clone(), name).changed() {
                        self.dirty = true;
                    }
                }
            });
        });
    }

    fn tool_list(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("Filtrer…").desired_width(170.0));
            if ui.button("➕ Outil").on_hover_text("Ajouter un outil personnel").clicked() {
                self.new_tool = NewTool { open: true, tool: blank_tool() };
            }
        });
        ui.separator();
        let f = self.filter.to_lowercase();
        let mut clicked = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (kind, label) in TYPES.iter().map(|(k, l)| (*k, *l)).chain(std::iter::once(("", "Autres"))) {
                let list: Vec<&Tool> = self
                    .tools
                    .iter()
                    .filter(|t| if kind.is_empty() { !TYPES.iter().any(|(k, _)| *k == t.kind) } else { t.kind == kind })
                    .filter(|t| f.is_empty() || t.name.to_lowercase().contains(&f) || t.id.contains(&f))
                    .collect();
                if list.is_empty() {
                    continue;
                }
                egui::CollapsingHeader::new(format!("{label} ({})", list.len())).default_open(true).show(ui, |ui| {
                    for t in list {
                        let sel = self.sel_tool.as_deref() == Some(t.id.as_str());
                        let mut txt = RichText::new(&t.name);
                        if self.is_user_tool(&t.id) {
                            txt = txt.color(PERSO);
                        }
                        if ui.selectable_label(sel, txt).clicked() {
                            clicked = Some(t.id.clone());
                        }
                    }
                });
            }
        });
        if let Some(id) = clicked {
            self.sel_tool = Some(id);
            self.dirty = true;
        }
    }

    fn details(&mut self, ui: &mut egui::Ui) {
        let Some(t) = self.selected().cloned() else {
            ui.label("Aucun outil.");
            return;
        };
        let user_tool = self.is_user_tool(&t.id);
        ui.horizontal(|ui| {
            ui.heading(&t.name);
            if user_tool {
                ui.label(RichText::new("outil perso").color(PERSO));
            }
        });
        let mut geo = format!("{} · Ø{} mm · {} dent{}", type_label(&t.kind), t.diameter_mm, t.flutes, if t.flutes > 1 { "s" } else { "" });
        if let Some(a) = t.angle_deg {
            geo += &format!(" · {a}°");
        }
        if let Some(p) = t.tip_diameter_mm {
            geo += &format!(" · pointe {p} mm");
        }
        if let Some(l) = t.flute_length_mm {
            geo += &format!(" · longueur utile {l} mm");
        }
        if let Some(s) = t.shank_mm {
            geo += &format!(" · queue {s} mm");
        }
        ui.label(RichText::new(geo).weak());
        if user_tool {
            ui.horizontal(|ui| {
                if ui.button("Modifier l'outil").clicked() {
                    self.new_tool = NewTool { open: true, tool: t.clone() };
                }
                if ui.button("Supprimer l'outil").clicked() {
                    if let Some(db) = &self.db {
                        self.status = match db.delete_user_tool(&t.id) {
                            Ok(()) => format!("Outil {} supprimé", t.name),
                            Err(e) => e.to_string(),
                        };
                    }
                    self.reload_lists();
                }
            });
        }
        ui.add_space(10.0);

        if !supported(&t.kind) {
            ui.label(RichText::new("Pas de calcul automatique pour ce type d'outil : saisir les valeurs éprouvées.").color(Color32::YELLOW));
        } else if self.computed.is_none() {
            ui.label(RichText::new("Pas de référence fabricant pour ce matériau : saisir les valeurs éprouvées.").color(Color32::YELLOW));
        }

        let c = self.computed.clone();
        let e = &mut self.edit;
        egui::Grid::new("params").num_columns(4).spacing([18.0, 8.0]).striped(true).show(ui, |ui| {
            ui.label("");
            ui.label(RichText::new("Calculé").strong());
            ui.label(RichText::new("Mes valeurs").strong().color(PERSO));
            ui.end_row();
            let rows: [(&str, &str, &mut Field, Option<f64>, f64); 5] = [
                ("Vitesse de broche", "tr/min", &mut e.rpm, c.as_ref().map(|p| p.rpm), 100.0),
                ("Avance", "mm/min", &mut e.feed, c.as_ref().map(|p| p.feed_mm_min), 10.0),
                ("Plongée", "mm/min", &mut e.plunge, c.as_ref().map(|p| p.plunge_mm_min), 10.0),
                ("Profondeur de passe", "mm", &mut e.stepdown, c.as_ref().map(|p| p.stepdown_mm), 0.05),
                ("Pas latéral", "mm", &mut e.stepover, c.as_ref().and_then(|p| p.stepover_mm), 0.05),
            ];
            for (label, unit, field, calc, speed) in rows {
                ui.label(label);
                ui.label(calc.map_or("-".into(), |v| format!("{v} {unit}")));
                ui.horizontal(|ui| {
                    ui.checkbox(&mut field.on, "");
                    ui.add_enabled(
                        field.on,
                        egui::DragValue::new(&mut field.v).range(0.0..=1.0e6).speed(speed).suffix(format!(" {unit}")),
                    );
                });
                ui.end_row();
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Note");
            ui.add(egui::TextEdit::singleline(&mut self.edit.note).hint_text("ex. CTP peuplier, essai OK").desired_width(360.0));
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button(RichText::new("💾 Enregistrer mes valeurs").color(PERSO)).clicked() {
                self.save_user();
            }
            if self.has_user && ui.button("Revenir au calcul").clicked() {
                self.clear_user();
            }
        });
        ui.label(RichText::new("Case cochée = valeur perso ; case vide = valeur calculée.").small().weak());

        ui.add_space(14.0);
        ui.separator();
        match &self.result {
            Some(p) => {
                let (txt, col) = match p.origin {
                    Origin::Computed => ("Retenu : calcul", ACCENT),
                    Origin::User => ("Retenu : mes valeurs", PERSO),
                    Origin::Mixed => ("Retenu : mes valeurs + calcul", PERSO),
                    Origin::Atelier => ("Retenu : valeurs de l'atelier", PERSO),
                };
                ui.label(RichText::new(txt).strong().color(col));
                let mut s = format!("{} tr/min · avance {} mm/min · plongée {} mm/min · passe {} mm",
                                    p.rpm, p.feed_mm_min, p.plunge_mm_min, p.stepdown_mm);
                if let Some(so) = p.stepover_mm {
                    s += &format!(" · pas latéral {so} mm");
                }
                ui.label(RichText::new(s).size(15.0));
                ui.label(RichText::new(format!("Charge par dent : {} mm", p.fz_mm)).weak());
                for n in &p.notes {
                    ui.label(format!("• {n}"));
                }
            }
            None => {
                ui.label(RichText::new("Aucune valeur disponible pour cette combinaison.").weak());
            }
        }
    }

    fn new_tool_window(&mut self, ctx: &egui::Context) {
        if !self.new_tool.open {
            return;
        }
        let mut open = true;
        let mut save = false;
        egui::Window::new("Outil personnel").open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
            let t = &mut self.new_tool.tool;
            egui::Grid::new("newtool").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                ui.label("Identifiant");
                ui.text_edit_singleline(&mut t.id);
                ui.end_row();
                ui.label("Nom");
                ui.text_edit_singleline(&mut t.name);
                ui.end_row();
                ui.label("Type");
                egui::ComboBox::from_id_salt("type").selected_text(type_label(&t.kind).to_owned()).show_ui(ui, |ui| {
                    for (k, l) in TYPES {
                        ui.selectable_value(&mut t.kind, k.to_owned(), l);
                    }
                });
                ui.end_row();
                ui.label("Diamètre");
                ui.add(egui::DragValue::new(&mut t.diameter_mm).range(0.01..=100.0).speed(0.05).suffix(" mm"));
                ui.end_row();
                ui.label("Dents");
                ui.add(egui::DragValue::new(&mut t.flutes).range(1..=8));
                ui.end_row();
                opt_row(ui, "Angle", " °", &mut t.angle_deg, 1.0);
                opt_row(ui, "Pointe", " mm", &mut t.tip_diameter_mm, 0.01);
                opt_row(ui, "Longueur utile", " mm", &mut t.flute_length_mm, 0.5);
                opt_row(ui, "Queue", " mm", &mut t.shank_mm, 0.05);
            });
            ui.add_space(8.0);
            let ok = !t.id.trim().is_empty() && !t.name.trim().is_empty() && t.diameter_mm > 0.0;
            if ui.add_enabled(ok, egui::Button::new("Enregistrer l'outil")).clicked() {
                save = true;
            }
        });
        if save {
            let mut t = self.new_tool.tool.clone();
            t.id = t.id.trim().to_owned();
            if let Some(db) = &self.db {
                self.status = match db.save_user_tool(&t) {
                    Ok(()) => format!("Outil {} enregistré", t.name),
                    Err(e) => e.to_string(),
                };
            }
            self.sel_tool = Some(t.id);
            self.reload_lists();
            open = false;
        }
        self.new_tool.open = open;
    }
}

fn opt_row(ui: &mut egui::Ui, label: &str, unit: &str, v: &mut Option<f64>, speed: f64) {
    ui.label(label);
    ui.horizontal(|ui| {
        let mut on = v.is_some();
        if ui.checkbox(&mut on, "").changed() {
            *v = on.then_some(v.unwrap_or(0.0));
        }
        if let Some(x) = v.as_mut() {
            ui.add(egui::DragValue::new(x).range(0.0..=1000.0).speed(speed).suffix(unit));
        }
    });
    ui.end_row();
}

fn blank_tool() -> Tool {
    Tool {
        id: "perso-".into(),
        name: String::new(),
        kind: "flat".into(),
        diameter_mm: 3.175,
        flutes: 2,
        angle_deg: None,
        tip_diameter_mm: None,
        flute_length_mm: None,
        shank_mm: Some(3.175),
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(err) = self.error.clone() {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.heading("Base d'outils inaccessible");
                ui.label(err);
                ui.label(format!("Dossier : {}", data_dir().display()));
            });
            return;
        }
        if self.dirty {
            self.refresh();
        }
        egui::Panel::top("haut").show(ui, |ui| {
            ui.add_space(4.0);
            self.top_bar(ui);
            ui.add_space(4.0);
        });
        egui::Panel::bottom("bas").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.status).color(PERSO));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("Calcul : Sorotec · Mes valeurs : {}", data_dir().join("user_tools.db").display())).small().weak());
                });
            });
        });
        egui::Panel::left("outils").default_size(270.0).show(ui, |ui| self.tool_list(ui));
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.details(ui));
        });
        self.new_tool_window(&ctx);
    }
}

fn main() -> eframe::Result {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Bibliothèque d'outils - MakerCraft")
        .with_inner_size([1100.0, 680.0])
        .with_min_inner_size([820.0, 480.0]);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(ICON_PNG) {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        "makercraft-outils",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(App::new()))
        }),
    )
}
