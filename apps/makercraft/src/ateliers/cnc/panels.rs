//! Panneau de gauche de l'atelier CNC : les réglages de l'étape en cours.

use eframe::egui::{self, DragValue, RichText, Sense, Ui, vec2};
use makercraft_dao::LayerInfo;
use makercraft_mkc::{PathKind, PathTool, XyOrigin, ZOrigin};

use super::{Cnc, Request, Step, toolpaths};
use crate::icons::Icon;
use crate::prefs::Units;
use crate::theme::{self, Palette};
use crate::widgets::{self, Kind};

pub fn show(c: &mut Cnc, ui: &mut Ui, pal: &Palette, units: Units, req: &mut Vec<Request>) {
    // où l'on en est dans la chronologie, avec l'étape précédente et la suivante
    let i = c.step.index();
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("ÉTAPE {} / {}", i + 1, Step::ALL.len())).font(theme::bold(11.0)).color(pal.dim));
        ui.label(RichText::new(c.step.label()).font(theme::bold(15.0)).color(pal.blue));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add_enabled(i + 1 < Step::ALL.len(), egui::Button::new(">").small()).on_hover_text("Étape suivante").clicked() {
                c.step = Step::from_index(i + 1);
            }
            if ui.add_enabled(i > 0, egui::Button::new("<").small()).on_hover_text("Étape précédente").clicked() {
                c.step = Step::from_index(i - 1);
            }
        });
    });
    let _ = units;
    match c.step {
        Step::Machine => machine(c, ui, pal),
        Step::Brut => stock(c, ui, pal),
        Step::Dessin => drawing(c, ui, pal, req),
        Step::Parcours => paths(c, ui, pal),
        Step::Simulation => simulation(c, ui, pal),
        Step::Usinage => machining(c, ui, pal, req),
    }
}

fn num(ui: &mut Ui, label: &str, v: &mut f64, range: std::ops::RangeInclusive<f64>, suffix: &str) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(110.0, 20.0), Sense::hover());
        ui.painter().text(r.left_center(), egui::Align2::LEFT_CENTER, label, theme::regular(13.0), ui.visuals().weak_text_color());
        ui.add(DragValue::new(v).range(range).speed(0.5).max_decimals(3).suffix(suffix));
    });
}

// ---------------------------------------------------------------------- 1 Machine

fn machine(c: &mut Cnc, ui: &mut Ui, pal: &Palette) {
    let machines = c.tooldb().and_then(|db| db.machines().map_err(|e| e.to_string()));
    widgets::card(ui, pal, Some("Machine"), |ui| {
        match &machines {
            Ok(list) => {
                let cur = c.project.machine.name.clone();
                egui::ComboBox::from_id_salt("cnc-machine").width(ui.available_width()).selected_text(cur).show_ui(ui, |ui| {
                    for m in list {
                        if ui.selectable_label(c.project.machine.tooldb_id == m.id, &m.name).clicked() {
                            c.project.machine.tooldb_id = m.id.clone();
                            c.project.machine.name = m.name.clone();
                        }
                    }
                });
                if let Some(m) = list.iter().find(|m| m.id == c.project.machine.tooldb_id) {
                    widgets::caption(ui, pal, "Broche");
                    ui.label(format!("{:.0} à {:.0} tr/min, outils jusqu'à {:.1} mm", m.rpm_min, m.rpm_max, m.max_diameter_mm));
                }
            }
            Err(e) => widgets::status_line(ui, pal, Icon::Warn, pal.warn, &format!("Banque d'outils : {e}")),
        }
        ui.horizontal(|ui| {
            widgets::caption(ui, pal, "Nom");
            ui.text_edit_singleline(&mut c.project.machine.name);
        });
    });
    widgets::card(ui, pal, Some("Courses"), |ui| {
        let t = &mut c.project.machine.travel;
        num(ui, "X", &mut t[0], 10.0..=5000.0, " mm");
        num(ui, "Y", &mut t[1], 10.0..=5000.0, " mm");
        num(ui, "Z", &mut t[2], 5.0..=1000.0, " mm");
        let s = c.project.stock.size;
        let fits = s[0] <= t[0] && s[1] <= t[1];
        if fits {
            widgets::status_line(ui, pal, Icon::Ok, pal.ok, "Le brut tient dans la course");
        } else {
            widgets::status_line(ui, pal, Icon::Warn, pal.warn, "Le brut dépasse la course de la machine");
        }
    });
    widgets::card(ui, pal, Some("Contrôleur"), |ui| {
        ui.label(&c.project.machine.controller);
        ui.label(RichText::new("Le pilotage (connexion, envoi du programme) se fait dans l'atelier Machines.").size(12.5).color(pal.weak));
    });
}

// ---------------------------------------------------------------------- 2 Brut

fn stock(c: &mut Cnc, ui: &mut Ui, pal: &Palette) {
    let materials = c.tooldb().and_then(|db| db.materials().map_err(|e| e.to_string())).unwrap_or_default();
    widgets::card(ui, pal, Some("Brut"), |ui| {
        let s = &mut c.project.stock;
        num(ui, "Largeur (X)", &mut s.size[0], 1.0..=5000.0, " mm");
        num(ui, "Profondeur (Y)", &mut s.size[1], 1.0..=5000.0, " mm");
        num(ui, "Épaisseur (Z)", &mut s.size[2], 0.1..=500.0, " mm");
        let label = materials.iter().find(|(k, _)| *k == s.material).map_or_else(|| if s.material.is_empty() { "à choisir".to_owned() } else { s.material.clone() }, |(_, l)| l.clone());
        ui.horizontal(|ui| {
            widgets::caption(ui, pal, "Matière");
            egui::ComboBox::from_id_salt("cnc-matiere").width(180.0).selected_text(label).show_ui(ui, |ui| {
                for (k, l) in &materials {
                    ui.selectable_value(&mut s.material, k.clone(), l);
                }
            });
        });
    });
    widgets::card(ui, pal, Some("Zéro pièce"), |ui| {
        let s = &mut c.project.stock;
        for o in XyOrigin::ALL {
            ui.radio_value(&mut s.xy_origin, o, format!("X0 Y0 : {}", o.label()));
        }
        ui.radio_value(&mut s.z_origin, ZOrigin::Dessus, format!("Z0 : {}", ZOrigin::Dessus.label()));
        ui.add_enabled(false, egui::RadioButton::new(s.z_origin == ZOrigin::Dessous, format!("Z0 : {} (bientôt)", ZOrigin::Dessous.label())));
        num(ui, "Hauteur de sécurité", &mut c.project.safe_z, 1.0..=100.0, " mm");
        let cl = c.project.clearance();
        if cl > c.project.safe_z {
            ui.label(RichText::new(format!("Relevée à {cl:.1} mm pour passer au-dessus des brides.")).size(12.5).color(pal.weak));
        }
    });
    let mut remove = None;
    widgets::card(ui, pal, Some("Zones de bridage"), |ui| {
        if c.project.clamps.is_empty() {
            ui.label(RichText::new("Aucune bride. Ruban > Ajouter une bride : l'outil ne descendra pas dans ces zones.").size(13.0).color(pal.weak));
        }
        for (i, cl) in c.project.clamps.iter_mut().enumerate() {
            let sel = c.selected_clamp == Some(i);
            ui.horizontal(|ui| {
                if ui.selectable_label(sel, RichText::new(&cl.name).strong()).clicked() {
                    c.selected_clamp = if sel { None } else { Some(i) };
                }
                if ui.small_button("x").on_hover_text("Supprimer cette bride").clicked() {
                    remove = Some(i);
                }
            });
            if sel {
                ui.text_edit_singleline(&mut cl.name);
                num(ui, "X", &mut cl.x, -2000.0..=5000.0, " mm");
                num(ui, "Y", &mut cl.y, -2000.0..=5000.0, " mm");
                num(ui, "Largeur", &mut cl.w, 1.0..=1000.0, " mm");
                num(ui, "Profondeur", &mut cl.h, 1.0..=1000.0, " mm");
                num(ui, "Hauteur", &mut cl.height, 0.0..=200.0, " mm");
            }
        }
    });
    if let Some(i) = remove {
        c.project.clamps.remove(i);
        c.selected_clamp = None;
    }
}

// ---------------------------------------------------------------------- 3 Dessin

fn drawing(c: &mut Cnc, ui: &mut Ui, pal: &Palette, req: &mut Vec<Request>) {
    let layers: Vec<LayerInfo> = c.sketch.layers().into_iter().filter(|l| l.name != "Defpoints" && (l.name != "0" || l.objects > 0)).collect();
    let current = c.sketch.current_layer();
    let mut msg = None;

    // barre d'outils de l'arbre
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let solo = c.solo.is_some();
        if tool(ui, pal, Icon::Solo, "Contour courant seul (cliquer de nouveau pour rétablir)", solo) {
            match c.solo.take() {
                Some(saved) => {
                    for (name, on) in saved {
                        keep(&mut msg, set_layer(c, &name, "on", on));
                    }
                }
                None => {
                    c.solo = Some(layers.iter().map(|l| (l.name.clone(), l.visible)).collect());
                    for l in &layers {
                        keep(&mut msg, set_layer(c, &l.name, "on", l.name == current));
                    }
                }
            }
        }
        if tool(ui, pal, Icon::EyeOff, "Masquer les autres contours", false) {
            c.solo = None;
            for l in layers.iter().filter(|l| l.name != current) {
                keep(&mut msg, set_layer(c, &l.name, "on", false));
            }
        }
        if tool(ui, pal, Icon::Eye, "Tout afficher", false) {
            c.solo = None;
            for l in &layers {
                keep(&mut msg, set_layer(c, &l.name, "on", true));
            }
        }
        if tool(ui, pal, Icon::Unlock, "Tout déverrouiller", false) {
            for l in layers.iter().filter(|l| l.locked) {
                keep(&mut msg, set_layer(c, &l.name, "locked", false));
            }
        }
        let all_open = !layers.is_empty() && layers.iter().all(|l| c.expanded.contains(&l.name));
        if tool(ui, pal, if all_open { Icon::Collapse } else { Icon::Expand }, if all_open { "Tout replier" } else { "Tout déplier" }, false) {
            if all_open {
                c.expanded.clear();
            } else {
                c.expanded.extend(layers.iter().map(|l| l.name.clone()));
            }
        }
        if tool(ui, pal, Icon::New, "Nouveau contour", false) {
            c.command("cnc.contour_nouveau", req);
        }
    });

    // arbre des contours : un calque = un contour
    widgets::card(ui, pal, Some("Contours"), |ui| {
        if layers.is_empty() {
            ui.label(RichText::new("Aucun contour.").color(pal.weak));
        }
        for l in &layers {
            let paths = c.sketch.contours(&l.name, 0.05);
            let open = c.expanded.contains(&l.name);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if tool(ui, pal, if open { Icon::Collapse } else { Icon::Expand }, if open { "Replier" } else { "Déplier" }, false) {
                    if open {
                        c.expanded.remove(&l.name);
                    } else {
                        c.expanded.insert(l.name.clone());
                    }
                }
                let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                ui.painter().rect_filled(r, egui::CornerRadius::same(3), egui::Color32::from_rgb(l.rgb[0], l.rgb[1], l.rgb[2]));
                let cur = l.name == current;
                let editing = c.renaming.as_ref().is_some_and(|(old, _)| *old == l.name);
                if editing {
                    // renommage sur place : Entrée valide, Échap annule
                    let mut commit = false;
                    if let Some((_, buf)) = c.renaming.as_mut() {
                        let r = ui.add(egui::TextEdit::singleline(buf).desired_width(150.0));
                        r.request_focus();
                        commit = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        c.renaming = None;
                    } else if commit && let Some((old, new)) = c.renaming.take() {
                        keep(&mut msg, rename_contour(c, &old, new.trim()));
                    }
                } else {
                    let name = RichText::new(&l.name).strong().color(if l.visible { ui.visuals().text_color() } else { pal.dim });
                    let r = ui.radio(cur, name).on_hover_text("Contour courant : ce qu'on dessine va dessus ; double-clic ou F2 : renommer ; clic droit : menu");
                    if r.clicked() {
                        c.sketch.set_current_layer(&l.name);
                    }
                    if r.double_clicked() || (cur && ui.input(|i| i.key_pressed(egui::Key::F2)) && !ui.ctx().egui_wants_keyboard_input()) {
                        c.renaming = Some((l.name.clone(), l.name.clone()));
                    }
                    r.context_menu(|ui| {
                        if ui.button("Renommer (F2)").clicked() {
                            c.renaming = Some((l.name.clone(), l.name.clone()));
                        }
                        if ui.button("Sélectionner ses objets").clicked() {
                            let hs = c.sketch.objects_on(&l.name);
                            c.sketch.session.set_selection(hs);
                        }
                        ui.separator();
                        if ui.button("Supprimer le contour...").clicked() {
                            c.delete_contour = Some(l.name.clone());
                        }
                    });
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if tool(ui, pal, if l.locked { Icon::Lock } else { Icon::Unlock }, if l.locked { "Verrouillé : cliquer pour déverrouiller" } else { "Verrouiller (plus de modification possible)" }, l.locked) {
                        keep(&mut msg, set_layer(c, &l.name, "locked", !l.locked));
                    }
                    if tool(ui, pal, if l.visible { Icon::Eye } else { Icon::EyeOff }, if l.visible { "Visible : cliquer pour masquer" } else { "Masqué : cliquer pour afficher" }, !l.visible) {
                        c.solo = None;
                        keep(&mut msg, set_layer(c, &l.name, "on", !l.visible));
                    }
                    let used: Vec<&str> = c.project.toolpaths.iter().filter(|t| t.layers.contains(&l.name)).map(|t| t.name.as_str()).collect();
                    if !used.is_empty() {
                        let (r, resp) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                        crate::icons::paint(ui.painter(), r, Icon::Link, pal.orange);
                        resp.on_hover_text(format!("Utilisé par : {}", used.join(", ")));
                    }
                    if c.project.comments.contains_key(&l.name) {
                        let (r, resp) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                        crate::icons::paint(ui.painter(), r, Icon::Comment, pal.weak);
                        resp.on_hover_text("Ce contour a un commentaire (panneau de droite, onglet Commentaires)");
                    }
                });
            });
            if open {
                ui.indent(("contour", &l.name), |ui| {
                    if paths.is_empty() {
                        ui.label(RichText::new("vide").size(12.0).color(pal.weak));
                    }
                    for (i, p) in paths.iter().enumerate() {
                        let kind = if p.closed { "fermé" } else { "ouvert" };
                        ui.label(RichText::new(format!("Tracé {} : {kind}, {} points", i + 1, p.points.len())).size(12.0).color(if p.closed { pal.weak } else { pal.warn }));
                    }
                    ui.label(RichText::new(format!("{} objet(s)", l.objects)).size(12.0).color(pal.dim));
                });
            }
        }
    });
    delete_window(c, ui, &mut msg);
    if let Some(m) = msg {
        req.push(Request::Status(m));
    }
    widgets::card(ui, pal, Some("Dessiner"), |ui| {
        ui.label(RichText::new("Les commandes du ruban (ou tapées : ligne, cercle, rect...) demandent leurs points : clic dans la vue, ou valeurs dans la ligne de commande (x,y ; @dx,dy ; longueur). Clic droit : menu de la commande ou de la sélection ; clic molette : menu des vues.").size(12.5).color(pal.weak));
        ui.label(RichText::new("Sélection : clic, ou rectangle glissé. Le cadre orange déplace, met à l'échelle et tourne (Maj : depuis le centre, pas de 15°) ; Entrée ouvre les cotes.").size(12.5).color(pal.weak));
    });
}

/// Renomme un contour (calque) ; les parcours et commentaires qui le visent suivent.
fn rename_contour(c: &mut Cnc, old: &str, new: &str) -> Option<String> {
    if new.is_empty() || new == old {
        return None;
    }
    if c.sketch.layers().iter().any(|l| l.name == new) {
        return Some(format!("Un contour s'appelle déjà « {new} »"));
    }
    if let Err(e) = c.sketch.session.execute("layer.set", &serde_json::json!({ "name": old, "newName": new })) {
        return Some(e.to_string());
    }
    for tp in &mut c.project.toolpaths {
        for l in &mut tp.layers {
            if l == old {
                *l = new.to_owned();
            }
        }
    }
    if let Some(t) = c.project.comments.remove(old) {
        c.project.comments.insert(new.to_owned(), t);
    }
    c.expanded.remove(old);
    None
}

/// Confirmation avant de supprimer un contour et tous ses objets (annulable par Ctrl+Z).
fn delete_window(c: &mut Cnc, ui: &mut Ui, msg: &mut Option<String>) {
    let Some(name) = c.delete_contour.clone() else { return };
    let n = c.sketch.objects_on(&name).len();
    let used: Vec<String> = c.project.toolpaths.iter().filter(|t| t.layers.contains(&name)).map(|t| t.name.clone()).collect();
    let mut choice = None;
    egui::Modal::new(egui::Id::unique("supprimer-contour")).show(ui.ctx(), |ui| {
        ui.set_width(380.0);
        ui.label(RichText::new(format!("Supprimer le contour « {name} » ?")).strong().size(15.0));
        ui.label(format!("{n} objet(s) seront effacés (Ctrl+Z pour revenir en arrière)."));
        if !used.is_empty() {
            ui.label(RichText::new(format!("Parcours qui l'utilisent : {}", used.join(", "))).color(pal_warn(ui)));
        }
        ui.horizontal(|ui| {
            if ui.button("Supprimer").clicked() {
                choice = Some(true);
            }
            if ui.button("Annuler").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                choice = Some(false);
            }
        });
    });
    match choice {
        Some(true) => {
            c.delete_contour = None;
            keep(msg, delete_contour(c, &name));
        }
        Some(false) => c.delete_contour = None,
        None => {}
    }
}

fn pal_warn(ui: &Ui) -> egui::Color32 {
    ui.visuals().warn_fg_color
}

fn delete_contour(c: &mut Cnc, name: &str) -> Option<String> {
    let hs: Vec<String> = c.sketch.objects_on(name).iter().map(|h| h.hex()).collect();
    if !hs.is_empty()
        && let Err(e) = c.sketch.session.execute("erase", &serde_json::json!({ "handles": hs }))
    {
        return Some(e.to_string());
    }
    if c.sketch.current_layer() == name
        && let Some(other) = c.sketch.layers().into_iter().map(|l| l.name).find(|l| l != name && l != "Defpoints")
    {
        c.sketch.set_current_layer(&other);
    }
    if name != "0"
        && let Err(e) = c.sketch.session.execute("layer.delete", &serde_json::json!({ "name": name }))
    {
        return Some(e.to_string());
    }
    c.project.comments.remove(name);
    None
}

/// Garde le premier message d'erreur.
fn keep(msg: &mut Option<String>, e: Option<String>) {
    if msg.is_none() {
        *msg = e;
    }
}

/// Change une propriété (`on`, `locked`) du calque `name` ; renvoie l'erreur éventuelle.
fn set_layer(c: &mut Cnc, name: &str, key: &str, value: bool) -> Option<String> {
    let mut m = serde_json::Map::new();
    m.insert("name".into(), name.into());
    m.insert(key.into(), value.into());
    let r = c.sketch.session.execute("layer.set", &serde_json::Value::Object(m));
    // les objets cachés ou verrouillés quittent la sélection
    let sel = c.sketch.editable(&c.sketch.session.selection());
    c.sketch.session.set_selection(sel);
    r.err().map(|e| e.to_string())
}

/// Petit bouton icône de l'arbre des contours ; `on` : état actif (surligné).
fn tool(ui: &mut Ui, pal: &Palette, icon: Icon, tip: &str, on: bool) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, egui::CornerRadius::same(5), pal.hover);
    }
    crate::icons::paint(ui.painter(), r.shrink(4.0), icon, if on { pal.blue } else { ui.visuals().text_color() });
    resp.on_hover_text(tip).clicked()
}

// ---------------------------------------------------------------------- 4 Parcours

fn paths(c: &mut Cnc, ui: &mut Ui, pal: &Palette) {
    let ops: Vec<(String, Vec<String>)> = c.job().groups.iter().flat_map(|g| g.ops.iter()).map(|o| (o.name.clone(), o.warnings.clone())).collect();
    widgets::card(ui, pal, Some("Parcours"), |ui| {
        if c.project.toolpaths.is_empty() {
            ui.label(RichText::new("Aucun parcours. Ruban > Ajouter un parcours : il prend le calque courant.").size(13.0).color(pal.weak));
        }
        for (i, tp) in c.project.toolpaths.iter_mut().enumerate() {
            let sel = c.selected_path == Some(i);
            ui.horizontal(|ui| {
                ui.checkbox(&mut tp.enabled, "").on_hover_text("Usiner ce parcours");
                if ui.selectable_label(sel, RichText::new(&tp.name).strong()).clicked() {
                    c.selected_path = if sel { None } else { Some(i) };
                }
                ui.label(RichText::new(format!("Ø{:.2}", tp.tool.diameter)).size(12.0).color(pal.orange));
            });
        }
    });
    let Some(i) = c.selected_path.filter(|i| *i < c.project.toolpaths.len()) else { return };
    let layers: Vec<String> = c.sketch.layers().into_iter().filter(|l| l.objects > 0 && !makercraft_dao::sketch::is_reference_layer(&l.name)).map(|l| l.name).collect();
    let tools = c.tooldb().and_then(|db| db.tools().map_err(|e| e.to_string())).unwrap_or_default();
    let tp = &mut c.project.toolpaths[i];
    widgets::card(ui, pal, Some("Réglages du parcours"), |ui| {
        ui.text_edit_singleline(&mut tp.name);
        ui.horizontal(|ui| {
            widgets::caption(ui, pal, "Type");
            egui::ComboBox::from_id_salt("cnc-type-parcours").selected_text(tp.kind.label()).show_ui(ui, |ui| {
                for k in PathKind::ALL {
                    ui.selectable_value(&mut tp.kind, k, k.label());
                }
            });
        });
        widgets::caption(ui, pal, "Calques usinés");
        if layers.is_empty() {
            ui.label(RichText::new("Le dessin est vide.").color(pal.weak));
        }
        for l in &layers {
            let mut on = tp.layers.contains(l);
            if ui.checkbox(&mut on, l.as_str()).changed() {
                if on {
                    tp.layers.push(l.clone());
                } else {
                    tp.layers.retain(|x| x != l);
                }
            }
        }
    });
    widgets::card(ui, pal, Some("Outil"), |ui| {
        egui::ComboBox::from_id_salt("cnc-outil").width(ui.available_width()).selected_text(tp.tool.name.clone()).show_ui(ui, |ui| {
            for t in &tools {
                if ui.selectable_label(tp.tool.id == t.id, format!("{}  (Ø{:.2})", t.name, t.diameter_mm)).clicked() {
                    tp.tool = PathTool { id: t.id.clone(), name: t.name.clone(), kind: t.kind.clone(), diameter: t.diameter_mm, angle: t.angle_deg.unwrap_or(0.0), flutes: t.flutes };
                }
            }
        });
        num(ui, "Diamètre", &mut tp.tool.diameter, 0.1..=50.0, " mm");
    });
    widgets::card(ui, pal, Some("Coupe"), |ui| {
        num(ui, "Profondeur", &mut tp.depth, 0.05..=200.0, " mm");
        num(ui, "Passe", &mut tp.stepdown, 0.05..=50.0, " mm");
        if tp.kind == PathKind::Poche {
            num(ui, "Recouvrement", &mut tp.stepover, 0.05..=50.0, " mm");
        }
        num(ui, "Avance", &mut tp.feed, 10.0..=20_000.0, " mm/min");
        num(ui, "Plongée", &mut tp.plunge, 10.0..=10_000.0, " mm/min");
        num(ui, "Rotation", &mut tp.rpm, 0.0..=60_000.0, " tr/min");
    });
    if let Some((_, w)) = ops.iter().find(|(n, _)| *n == c.project.toolpaths[i].name) {
        for m in w {
            widgets::status_line(ui, pal, Icon::Warn, pal.warn, m);
        }
    }
}

/// Avance, plongée, rotation et passes proposées par la banque d'outils pour le parcours `i`.
pub fn suggest_speeds(c: &mut Cnc, i: usize) -> String {
    let Some(tp) = c.project.toolpaths.get(i).cloned() else { return String::new() };
    if tp.tool.id.is_empty() {
        return "Choisir d'abord un outil de la banque".into();
    }
    if c.project.stock.material.is_empty() {
        return "Choisir d'abord la matière du brut (étape Brut)".into();
    }
    let (mat, mach) = (c.project.stock.material.clone(), c.project.machine.tooldb_id.clone());
    let r = c.tooldb().and_then(|db| db.suggest(&tp.tool.id, &mat, &mach).map_err(|e| e.to_string()));
    match r {
        Ok(Some(p)) => {
            if let Some(t) = c.project.toolpaths.get_mut(i) {
                t.feed = p.feed_mm_min.round();
                t.plunge = p.plunge_mm_min.round();
                t.rpm = p.rpm.round();
                t.stepdown = (p.stepdown_mm * 100.0).round() / 100.0;
                if let Some(so) = p.stepover_mm {
                    t.stepover = (so * 100.0).round() / 100.0;
                }
            }
            format!("Vitesses proposées : {:.0} mm/min, {:.0} tr/min, passe {:.2} mm", p.feed_mm_min, p.rpm, p.stepdown_mm)
        }
        Ok(None) => "Pas de référence de coupe pour cet outil et cette matière".into(),
        Err(e) => format!("Banque d'outils : {e}"),
    }
}

// ---------------------------------------------------------------------- 5 Simulation

fn simulation(c: &mut Cnc, ui: &mut Ui, pal: &Palette) {
    let project = c.project.clone();
    let job = c.job().clone();
    let hits = toolpaths::clamp_hits(&project, &job);
    widgets::card(ui, pal, Some("Vérifications"), |ui| {
        if job.groups.is_empty() {
            widgets::status_line(ui, pal, Icon::Info, pal.blue, "Aucun parcours actif");
        }
        for (op, clamp) in &hits {
            widgets::status_line(ui, pal, Icon::Error, pal.bad, &format!("{op} passe dans la zone de {clamp}"));
        }
        let s = project.stock.size;
        let t = project.machine.travel;
        if s[0] > t[0] || s[1] > t[1] {
            widgets::status_line(ui, pal, Icon::Warn, pal.warn, "Le brut dépasse la course de la machine");
        }
        let deepest = job.groups.iter().flat_map(|g| g.ops.iter()).flat_map(|o| o.moves()).map(|m| m.target().z).fold(0.0, f64::min);
        if -deepest > s[2] + 1e-6 {
            widgets::status_line(ui, pal, Icon::Warn, pal.warn, &format!("Usinage à {:.2} mm sous le dessus : plus que l'épaisseur du brut", -deepest));
        }
        for g in &job.groups {
            for o in &g.ops {
                for w in &o.warnings {
                    widgets::status_line(ui, pal, Icon::Warn, pal.warn, &format!("{} : {w}", o.name));
                }
            }
        }
        if hits.is_empty() && !job.groups.is_empty() {
            widgets::status_line(ui, pal, Icon::Ok, pal.ok, "Aucun passage dans les brides");
        }
    });
    widgets::card(ui, pal, Some("Durée estimée"), |ui| {
        let mut total = 0.0;
        for g in &job.groups {
            let st = g.stats(job.heights);
            total += st.seconds;
            ui.label(RichText::new(format!("{}  -  {:.0} mm usinés, {}", makercraft_cam::job::tool_label(&g.tool), st.cut_length, duration(st.seconds))).size(13.0));
        }
        ui.label(RichText::new(format!("Total : {}", duration(total))).font(theme::bold(14.0)));
        ui.label(RichText::new("La simulation de l'enlèvement de matière viendra dans une prochaine étape.").size(12.5).color(pal.weak));
    });
}

fn duration(s: f64) -> String {
    let m = (s / 60.0).round() as i64;
    if m >= 60 { format!("{} h {:02} min", m / 60, m % 60) } else { format!("{m} min") }
}

// ---------------------------------------------------------------------- 6 Usinage

fn machining(c: &mut Cnc, ui: &mut Ui, pal: &Palette, req: &mut Vec<Request>) {
    let job = c.job().clone();
    widgets::card(ui, pal, Some("Programme"), |ui| {
        ui.label(format!("{} outil(s), {} parcours", job.groups.len(), job.groups.iter().map(|g| g.ops.len()).sum::<usize>()));
        for g in &job.groups {
            ui.horizontal(|ui| {
                widgets::pill(ui, &format!("T{}", g.tool.number), pal.orange_soft, pal.orange);
                ui.add(egui::Label::new(RichText::new(format!("{}  -  Ø{:.2} mm", g.tool.name, g.tool.diameter)).size(13.0)).truncate());
            });
        }
        ui.label(RichText::new("Un seul fichier Grbl : pause M0 à chaque changement d'outil.").size(12.5).color(pal.weak));
        ui.horizontal(|ui| {
            let on = !job.groups.is_empty();
            if widgets::button(ui, pal, Some(Icon::Export), "Écrire le .nc...", Kind::Component, on).clicked() {
                write_gcode(c, req, false);
            }
            if widgets::button(ui, pal, Some(Icon::Play), "Vers Machines", Kind::Plain, on).clicked() {
                write_gcode(c, req, true);
            }
        });
        match &c.written {
            Some(Ok(p)) => widgets::status_line(ui, pal, Icon::Ok, pal.ok, &format!("Écrit : {}", p.display())),
            Some(Err(e)) => widgets::status_line(ui, pal, Icon::Error, pal.bad, e),
            None => {}
        }
    });
    widgets::card(ui, pal, Some("Sûreté"), |ui| {
        ui.label(RichText::new("Avant d'usiner : brut bridé comme prévu, zéro pièce pris, passage à vide conseillé. L'envoi à une vraie machine demande une confirmation dans l'atelier Machines.").size(12.5).color(pal.weak));
    });
}

/// Écrit le G-code ; `send` : dans un fichier temporaire, puis l'ouvre dans l'atelier Machines.
pub fn write_gcode(c: &mut Cnc, req: &mut Vec<Request>, send: bool) {
    let project = c.project.clone();
    let job = c.job().clone();
    let hits = toolpaths::clamp_hits(&project, &job);
    if !hits.is_empty() {
        req.push(Request::Status(format!("G-code non écrit : {} passe dans une zone de bridage (voir Simulation)", hits[0].0)));
        c.step = Step::Simulation;
        return;
    }
    let code = makercraft_cam::gcode::write_all(&job);
    let stem = c.path.as_ref().and_then(|p| p.file_stem()).map_or_else(|| "projet".to_owned(), |s| s.to_string_lossy().into_owned());
    let path = if send {
        Some(std::env::temp_dir().join(format!("{}.nc", makercraft_cam::ext::file_stem_safe(&stem))))
    } else {
        let mut d = rfd::FileDialog::new().set_title("Écrire le G-code").add_filter("G-code", &["nc"]).set_file_name(format!("{stem}.nc"));
        if let Some(dir) = c.path.as_ref().and_then(|p| p.parent()) {
            d = d.set_directory(dir);
        }
        d.save_file()
    };
    let Some(path) = path else { return };
    let r = std::fs::write(&path, code).map(|()| path.clone()).map_err(|e| format!("{} : {e}", path.display()));
    match &r {
        Ok(p) if send => req.push(Request::SendProgram(p.clone())),
        Ok(p) => req.push(Request::Status(format!("G-code écrit : {}", p.display()))),
        Err(e) => req.push(Request::Status(format!("Échec : {e}"))),
    }
    c.written = Some(r);
}
