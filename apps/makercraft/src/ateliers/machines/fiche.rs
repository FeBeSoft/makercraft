//! Onglet Fiche, sur le modèle de l'image de référence du 16/11 : Identité (photo, contrôleur,
//! liaison) | Caractéristiques et Équipements (présent, état) | Calibrage et Outils de la machine.

use eframe::egui::{self, Align2, Color32, CornerRadius, DragValue, Rect, Response, RichText, ScrollArea, Sense, Stroke, StrokeKind, TextureHandle, Ui, pos2, vec2};

use super::equip::{Item, Plan, Reference};
use super::{Machines, Page, Request};
use crate::icons::{self, Icon};
use crate::theme::{self, Palette};
use crate::widgets::{self, Kind};

/// Nombre en mm sans zéros inutiles, virgule décimale.
pub(super) fn mm(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.replace('.', ",")
}

/// Titre de carte orange souligné (images de référence).
pub(super) fn heading(ui: &mut Ui, pal: &Palette, t: &str) {
    ui.label(RichText::new(t).font(theme::bold(19.0)).color(pal.orange));
    let r = ui.available_rect_before_wrap();
    ui.painter().hline(r.x_range(), r.top() + 1.0, Stroke::new(2.0, theme::alpha(pal.orange, 0.8)));
    ui.add_space(8.0);
}

/// Pastille d'état, cliquable si `click`.
pub(super) fn badge(ui: &mut Ui, text: &str, bg: Color32, fg: Color32, click: bool) -> Response {
    let font = theme::bold(12.0);
    let g = ui.painter().layout_no_wrap(text.to_owned(), font, fg);
    let (rect, resp) = ui.allocate_exact_size(g.size() + vec2(18.0, 8.0), if click { Sense::click() } else { Sense::hover() });
    let bg = if click && resp.hovered() { bg.gamma_multiply(1.25) } else { bg };
    ui.painter().rect_filled(rect, CornerRadius::same(255), bg);
    ui.painter().galley(rect.min + vec2(9.0, 4.0), g, fg);
    resp
}

/// Ligne « icône, titre en gras, valeur ».
fn feature(ui: &mut Ui, pal: &Palette, icon: Icon, title: &str, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new().fill(pal.panel).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
            icons::paint(ui.painter(), r, icon, pal.weak);
            ui.add_space(6.0);
            ui.allocate_ui_with_layout(vec2(110.0, 26.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_min_width(110.0);
                ui.label(RichText::new(title).font(theme::bold(15.0)).color(pal.text));
            });
            ui.vertical(add);
        });
    });
}

impl Machines {
    /// Photo de la machine en texture (chargée une fois, réduite à 800 px).
    fn photo(&mut self, ctx: &egui::Context) -> Option<TextureHandle> {
        let path = self.equipment.photo.clone()?;
        if let Some((p, t)) = &self.photo_tex
            && *p == path
        {
            return t.clone();
        }
        let tex = image::open(&path).ok().map(|img| {
            let img = img.thumbnail(800, 800).to_rgba8();
            let size = [img.width() as usize, img.height() as usize];
            ctx.load_texture("machine-photo", egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()), egui::TextureOptions::LINEAR)
        });
        self.photo_tex = Some((path, tex.clone()));
        tex
    }

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
        let photo = self.photo(ui.ctx());
        ScrollArea::vertical().id_salt("mach-fiche").auto_shrink([false, false]).show(ui, |ui| {
            let total = ui.available_width();
            let left_w = (total * 0.24).clamp(240.0, 320.0);
            let right_w = (total * 0.27).clamp(260.0, 360.0);
            let mid_w = (total - left_w - right_w - 32.0).max(380.0);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = vec2(16.0, 12.0);
                // Identité
                ui.vertical(|ui| {
                    ui.set_width(left_w);
                    widgets::card(ui, pal, None, |ui| {
                        heading(ui, pal, "Identité");
                        let w = ui.available_width();
                        let (r, _) = ui.allocate_exact_size(vec2(w, w * 0.82), Sense::hover());
                        match &photo {
                            Some(t) => {
                                let s = t.size_vec2();
                                let k = (r.width() / s.x).min(r.height() / s.y);
                                ui.painter().rect_filled(r, CornerRadius::same(8), Color32::WHITE);
                                ui.painter().image(t.id(), Rect::from_center_size(r.center(), s * k), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                            }
                            None => {
                                ui.painter().rect_filled(r, CornerRadius::same(8), pal.panel);
                                icons::paint(ui.painter(), Rect::from_center_size(r.center() - vec2(0.0, 12.0), vec2(64.0, 64.0)), Icon::Machines, pal.dim);
                                ui.painter().text(r.center() + vec2(0.0, 36.0), Align2::CENTER_CENTER, "pas de photo", theme::regular(12.0), pal.weak);
                            }
                        }
                        ui.add_space(4.0);
                        if widgets::button(ui, pal, Some(Icon::Open), "Changer la photo", Kind::Plain, edit).clicked()
                            && let Some(f) = rfd::FileDialog::new().add_filter("Image", &["jpg", "jpeg", "png", "webp", "bmp"]).pick_file()
                        {
                            self.equipment.photo = Some(f);
                        }
                        ui.add_space(8.0);
                        ui.label(RichText::new(&p.name).font(theme::bold(17.0)).color(pal.text));
                        ui.label(RichText::new(format!("Contrôleur : {} par {}", p.controller, p.connection)).size(13.5));
                        let link = if !self.connected() {
                            "Liaison : déconnectée".to_owned()
                        } else if self.emulator {
                            "Liaison : émulateur".to_owned()
                        } else {
                            format!("Port {} · {} bauds", self.port, self.baud)
                        };
                        ui.label(RichText::new(link).size(13.5));
                        if let Some(f) = &self.firmware {
                            ui.label(RichText::new(f).size(12.5).color(pal.weak));
                        }
                    });
                });
                // Caractéristiques et équipements
                ui.vertical(|ui| {
                    ui.set_width(mid_w);
                    widgets::card(ui, pal, None, |ui| {
                        heading(ui, pal, "Caractéristiques");
                        ui.spacing_mut().item_spacing.y = 8.0;
                        feature(ui, pal, Icon::Stock, "Mécanique", |ui| {
                            ui.label(RichText::new(format!("Courses X {} / Y {} / Z {} mm", mm(p.travel_mm[0]), mm(p.travel_mm[1]), mm(p.travel_mm[2]))).size(13.5));
                            ui.label(RichText::new(&p.table).size(13.5));
                            ui.add_enabled_ui(edit, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Planche martyr").size(13.5));
                                    ui.add(DragValue::new(&mut self.equipment.spoilboard_mm).range(0.0..=40.0).speed(0.1).suffix(" mm"));
                                });
                            });
                        });
                        feature(ui, pal, Icon::Drill, "Broche", |ui| {
                            ui.label(RichText::new(format!("{} - jusqu'à {} tr/min ({} W)", p.spindle, super::thousands(p.spindle_rpm_max), mm(p.spindle_power_w))).size(13.5));
                        });
                        feature(ui, pal, Icon::Clamp, "Pinces ER11", |ui| {
                            ui.horizontal(|ui| {
                                for c in &p.collets_mm {
                                    badge(ui, &format!("{} mm", mm(*c)), pal.line2, pal.text, false);
                                }
                            });
                        });
                        ui.add_space(10.0);
                        heading(ui, pal, "Équipements");
                        ui.add_enabled_ui(edit, |ui| self.equipment_table(ui, pal));
                        ui.add(egui::Label::new(RichText::new("Les fonctions de l'onglet Pilotage s'activent selon les équipements cochés ; un clic sur l'état d'un équipement absent change ce qui est prévu.").size(12.0).color(pal.weak)).wrap());
                    });
                });
                // Calibrage et outils
                ui.vertical(|ui| {
                    ui.set_width(right_w);
                    self.calibration_card(ui, pal, edit);
                    widgets::card(ui, pal, None, |ui| {
                        heading(ui, pal, "Outils de cette machine");
                        let mut collets: Vec<f64> = self.tools.iter().map(|t| t.collet_mm).collect();
                        collets.sort_by(f64::total_cmp);
                        collets.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
                        let list: Vec<String> = collets.iter().map(|c| mm(*c)).collect();
                        ui.label(RichText::new(format!("{} outils de l'atelier", self.tools.len())).size(14.0));
                        if !list.is_empty() {
                            ui.label(RichText::new(format!("Pinces ER11 {} mm", list.join(" et "))).size(14.0));
                        }
                        ui.add_space(4.0);
                        ui.add_enabled(false, egui::Link::new("Ouvrir la bibliothèque d'outils")).on_disabled_hover_text("La bibliothèque rejoindra Machines & Outils (t082c)");
                    });
                });
            });
        });
        if self.equipment != before {
            req.push(Request::Equipment(p.id.clone(), self.equipment.clone()));
        }
    }

    fn equipment_table(&mut self, ui: &mut Ui, pal: &Palette) {
        let names = ["Arrêt d'urgence (STOP)", "Fins de course X / Y / Z", "Sonde Z", "Laser croix", "Caméra", "Module laser 12V PWM"];
        let icon = [Icon::Stop, Icon::Home, Icon::Down, Icon::Zero, Icon::Info, Icon::Laser];
        egui::Frame::new().stroke(Stroke::new(1.0, pal.card_line)).corner_radius(CornerRadius::same(6)).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let w = ui.available_width();
            egui::Grid::new("mach-equip").num_columns(3).min_col_width(70.0).spacing(vec2(12.0, 8.0)).striped(true).show(ui, |ui| {
                ui.label(RichText::new("Équipement").strong());
                ui.label(RichText::new("Présent").strong());
                ui.label(RichText::new("État").strong());
                ui.end_row();
                for item in Item::ALL {
                    let i = item.index();
                    let name = if item == Item::Probe { format!("Sonde Z (Ø {} mm, h {} mm)", mm(self.equipment.probe_diameter_mm), mm(self.equipment.probe_height_mm)) } else { names[i].to_owned() };
                    ui.horizontal(|ui| {
                        ui.set_min_width((w * 0.55).max(200.0));
                        let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                        icons::paint(ui.painter(), r, icon[i], pal.weak);
                        ui.label(RichText::new(name).size(13.5));
                    });
                    let mut on = self.equipment.present(item);
                    if ui.checkbox(&mut on, "").changed() {
                        self.equipment.set_present(item, on);
                    }
                    if on {
                        let t = if item == Item::Camera { "caméra Windows" } else { "installé" };
                        badge(ui, t, pal.ok, Color32::BLACK, false);
                    } else {
                        let plan = self.equipment.plans[i];
                        let (bg, fg) = match plan {
                            Plan::ATester | Plan::AImprimer => (pal.orange, pal.on_orange),
                            Plan::Non | Plan::AVenir => (pal.line2, pal.text),
                        };
                        if badge(ui, plan.label(), bg, fg, true).on_hover_text("Cliquer pour changer ce qui est prévu").clicked() {
                            self.equipment.plans[i] = plan.next();
                        }
                    }
                    ui.end_row();
                }
            });
        });
        let e = &mut self.equipment;
        if e.probe {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Sonde : hauteur").size(13.0));
                ui.add(DragValue::new(&mut e.probe_height_mm).range(0.0..=60.0).speed(0.05).suffix(" mm"));
                ui.label(RichText::new("diamètre").size(13.0));
                ui.add(DragValue::new(&mut e.probe_diameter_mm).range(0.0..=100.0).speed(0.1).suffix(" mm"));
            });
        }
        if e.laser_cross {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Laser croix : décalage X").size(13.0));
                ui.add(DragValue::new(&mut e.laser_offset_mm[0]).range(-100.0..=100.0).speed(0.05).suffix(" mm"));
                ui.label(RichText::new("Y").size(13.0));
                ui.add(DragValue::new(&mut e.laser_offset_mm[1]).range(-100.0..=100.0).speed(0.05).suffix(" mm"));
            });
        }
    }

    fn calibration_card(&mut self, ui: &mut Ui, pal: &Palette, edit: bool) {
        let expert = self.rights().expert_tabs;
        widgets::card(ui, pal, None, |ui| {
            heading(ui, pal, "Calibrage");
            let (zero, zbg, zfg) = match self.reference {
                Reference::None => ("non valable", pal.bad, Color32::WHITE),
                Reference::TableZero => ("valable", pal.ok, Color32::BLACK),
                Reference::Homed => ("référencé", pal.ok, Color32::BLACK),
            };
            let rows = [
                ("Test des entrées", None),
                ("Décalage laser / outil", None),
                ("Zéro table", Some((zero, zbg, zfg))),
                ("Pas par mm", None),
                ("Équerrage X/Y", None),
                ("Planéité du martyr", None),
                ("Perpendicularité de la broche", None),
            ];
            for (name, state) in rows {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(name).size(13.5));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (t, bg, fg) = state.unwrap_or(("à faire", pal.line2, pal.text));
                        badge(ui, t, bg, fg, false);
                    });
                });
            }
            ui.add_space(6.0);
            ui.add_enabled_ui(edit, |ui| {
                ui.horizontal(|ui| {
                    widgets::switch(ui, pal, &mut self.equipment.corrections, "Corrections appliquées d'office");
                    ui.label(RichText::new("Corrections appliquées d'office").size(13.5));
                });
            });
            ui.add_space(6.0);
            let w = ui.available_width();
            let (r, resp) = ui.allocate_exact_size(vec2(w, 36.0), if expert { Sense::click() } else { Sense::hover() });
            let c = if expert { pal.blue } else { theme::alpha(pal.blue, 0.35) };
            ui.painter().rect_filled(r, CornerRadius::same(8), if expert && resp.hovered() { c.gamma_multiply(0.9) } else { c });
            ui.painter().rect_stroke(r, CornerRadius::same(8), Stroke::new(1.0, c), StrokeKind::Inside);
            ui.painter().text(r.center(), Align2::CENTER_CENTER, "Ouvrir le calibrage", theme::bold(14.0), pal.on_blue);
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, expert, "Ouvrir le calibrage"));
            if resp.clicked() {
                self.page = Page::Calibrage;
            }
        });
    }
}
