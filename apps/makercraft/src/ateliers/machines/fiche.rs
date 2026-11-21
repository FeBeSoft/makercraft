//! Onglet Fiche : identité de la machine (profil de la banque), mécanique, broche, pinces,
//! équipements déclarés, outils de l'atelier et résumé du calibrage.

use eframe::egui::{self, DragValue, RichText, ScrollArea, Ui};

use super::{Machines, Request};
use crate::icons::Icon;
use crate::theme::Palette;
use crate::widgets;

/// Ligne « libellé : valeur ».
fn kv(ui: &mut Ui, pal: &Palette, k: &str, v: &str) {
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(egui::vec2(150.0, 18.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_min_width(150.0);
            ui.label(RichText::new(k).size(13.0).color(pal.weak));
        });
        ui.add(egui::Label::new(RichText::new(v).size(13.5).color(pal.text)).wrap());
    });
}

fn mm(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.replace('.', ",")
}

impl Machines {
    pub(super) fn fiche_ui(&mut self, ui: &mut Ui, pal: &Palette, req: &mut Vec<Request>) {
        let Some(p) = self.profile().cloned() else {
            widgets::card(ui, pal, Some("Fiche machine"), |ui| {
                let why = self.db_error.clone().unwrap_or_else(|| "aucun profil de machine dans la banque".to_owned());
                widgets::status_line(ui, pal, Icon::Warn, pal.warn, &format!("Banque d'outils et de machines illisible : {why}"));
            });
            return;
        };
        let edit = self.rights().edit_fiche;
        let before = self.equipment.clone();
        ScrollArea::vertical().id_salt("mach-fiche").auto_shrink([false, false]).show(ui, |ui| {
            ui.columns(2, |cols| {
                let [left, right] = cols else { return };
                left.spacing_mut().item_spacing.y = 12.0;
                right.spacing_mut().item_spacing.y = 12.0;
                widgets::card(left, pal, Some("Identité"), |ui| {
                    ui.label(RichText::new(&p.name).font(crate::theme::bold(17.0)).color(pal.text));
                    ui.add_space(4.0);
                    kv(ui, pal, "Contrôleur", &p.controller);
                    kv(ui, pal, "Connexion", &p.connection);
                    let link = if !self.connected() {
                        "déconnectée".to_owned()
                    } else if self.emulator {
                        "émulateur".to_owned()
                    } else {
                        format!("{} à {} bauds ({})", self.port, self.baud, self.transport)
                    };
                    kv(ui, pal, "Liaison", &link);
                    kv(ui, pal, "Micrologiciel", self.firmware.as_deref().unwrap_or("lu à la connexion"));
                    kv(ui, pal, "Source", &p.source);
                });
                widgets::card(left, pal, Some("Mécanique"), |ui| {
                    kv(ui, pal, "Courses", &format!("X {} / Y {} / Z {} mm", mm(p.travel_mm[0]), mm(p.travel_mm[1]), mm(p.travel_mm[2])));
                    kv(ui, pal, "Table", &p.table);
                    ui.add_enabled_ui(edit, |ui| {
                        ui.horizontal(|ui| {
                            ui.allocate_ui_with_layout(egui::vec2(150.0, 18.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.set_min_width(150.0);
                                ui.label(RichText::new("Planche martyr").size(13.0).color(pal.weak));
                            });
                            ui.add(DragValue::new(&mut self.equipment.spoilboard_mm).range(0.0..=40.0).speed(0.1).suffix(" mm"));
                        });
                    });
                });
                widgets::card(left, pal, Some("Broche et pinces"), |ui| {
                    kv(ui, pal, "Broche", &format!("{}, {} W, {} tr/min max.", p.spindle, mm(p.spindle_power_w), super::thousands(p.spindle_rpm_max)));
                    let collets: Vec<String> = p.collets_mm.iter().map(|c| mm(*c)).collect();
                    kv(ui, pal, "Pinces ER11", &format!("{} mm", collets.join(" / ")));
                });
                widgets::card(right, pal, Some("Équipements installés"), |ui| {
                    ui.label(RichText::new("Cocher ce qui est monté : les commandes correspondantes s'activent dans le Pilotage.").size(12.5).color(pal.weak));
                    ui.add_space(4.0);
                    ui.add_enabled_ui(edit, |ui| self.equipment_ui(ui, pal));
                });
                widgets::card(right, pal, Some("Outils de l'atelier"), |ui| {
                    if self.tools.is_empty() {
                        ui.label(RichText::new("aucun outil numéroté pour cette machine").color(pal.weak));
                    }
                    for t in &self.tools {
                        kv(ui, pal, &format!("T{}  -  pince {}", t.number, mm(t.collet_mm)), &t.tool.name);
                    }
                    ui.add_space(4.0);
                    ui.label(RichText::new("La bibliothèque d'outils rejoindra cette page (Machines & Outils).").size(12.0).color(pal.weak));
                });
                widgets::card(right, pal, Some("Calibrage"), |ui| {
                    kv(ui, pal, "Position machine", self.reference.label());
                    kv(ui, pal, "Procédures", "à faire (onglet Calibrage)");
                    ui.add_enabled_ui(edit, |ui| {
                        ui.horizontal(|ui| {
                            widgets::switch(ui, pal, &mut self.equipment.corrections, "Corrections de calibrage");
                            ui.label(RichText::new("Appliquer les corrections de calibrage (équerrage, planéité)").size(13.0));
                        });
                    });
                });
            });
        });
        if self.equipment != before {
            req.push(Request::Equipment(p.id.clone(), self.equipment.clone()));
        }
    }

    fn equipment_ui(&mut self, ui: &mut Ui, pal: &Palette) {
        let e = &mut self.equipment;
        let row = |ui: &mut Ui, on: &mut bool, label: &str, help: &str| {
            ui.horizontal(|ui| {
                widgets::switch(ui, pal, on, label);
                ui.label(RichText::new(label).size(13.5));
                if !help.is_empty() {
                    ui.label(RichText::new(help).size(12.0).color(pal.weak));
                }
            });
        };
        row(ui, &mut e.estop, "Arrêt d'urgence", "sur le connecteur STOP de la carte");
        ui.horizontal(|ui| {
            ui.label(RichText::new("Fins de course").size(13.5));
            for (i, a) in ["X", "Y", "Z"].iter().enumerate() {
                ui.checkbox(&mut e.limits[i], *a);
            }
            let help = if e.can_home() { "prise d'origine $H possible" } else { "$H grisé tant que les 3 ne sont pas cochés" };
            ui.label(RichText::new(help).size(12.0).color(pal.weak));
        });
        row(ui, &mut e.probe, "Sonde de Z", "");
        if e.probe {
            ui.horizontal(|ui| {
                ui.add_space(46.0);
                ui.label("hauteur");
                ui.add(DragValue::new(&mut e.probe_height_mm).range(0.0..=60.0).speed(0.05).suffix(" mm"));
                ui.label("diamètre");
                ui.add(DragValue::new(&mut e.probe_diameter_mm).range(0.0..=100.0).speed(0.1).suffix(" mm"));
            });
        }
        row(ui, &mut e.laser_cross, "Pointeur laser croix", "repérage seulement");
        if e.laser_cross {
            ui.horizontal(|ui| {
                ui.add_space(46.0);
                ui.label("décalage X");
                ui.add(DragValue::new(&mut e.laser_offset_mm[0]).range(-100.0..=100.0).speed(0.05).suffix(" mm"));
                ui.label("Y");
                ui.add(DragValue::new(&mut e.laser_offset_mm[1]).range(-100.0..=100.0).speed(0.05).suffix(" mm"));
            });
        }
        row(ui, &mut e.laser_module, "Module laser 12 V PWM", "à la place de la broche");
    }
}
