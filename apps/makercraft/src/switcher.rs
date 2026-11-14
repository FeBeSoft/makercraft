//! Sélecteur d'ateliers, sur le principe de l'« espace de travail » de SolidCraft (et des
//! logiciels de CAO comme CATIA ou SolidWorks) : un bouton en haut à gauche montre l'atelier
//! actif ; il déplie la liste des ateliers, chacun avec son icône, son état (actif, disponible,
//! à venir), ce qu'il fait et ce qu'il contient. Ce qui n'est pas disponible est grisé.

use eframe::egui::{self, Align2, Color32, CornerRadius, Key, Margin, Order, Rect, Sense, Shape, Stroke, Ui, pos2, vec2};

use crate::ateliers::Atelier;
use crate::icons;
use crate::theme::{self, Palette};

/// Largeur de la liste dépliée.
pub const MENU_W: f32 = 470.0;

#[derive(Default)]
pub struct Switcher {
    pub open: bool,
}

impl Switcher {
    /// Dessine le bouton dans `rect` et, s'il est ouvert, la liste. Renvoie l'atelier choisi.
    pub fn ui(&mut self, ui: &mut Ui, rect: Rect, pal: &Palette, current: Atelier) -> Option<Atelier> {
        let resp = ui.interact(rect, ui.scope_id().with("ateliers"), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Ateliers"));
        let p = ui.painter();
        if self.open || resp.hovered() {
            p.rect_filled(rect, CornerRadius::same(9), pal.hover);
        }
        let accent = current.family().accent(pal);
        let tile = Rect::from_min_size(pos2(rect.left() + 6.0, rect.center().y - 17.0), vec2(34.0, 34.0));
        p.rect_filled(tile, CornerRadius::same(8), theme::mix(pal.card, accent, 0.18));
        p.rect_stroke(tile, CornerRadius::same(8), Stroke::new(1.0, theme::mix(pal.card, accent, 0.45)), egui::StrokeKind::Inside);
        icons::paint(p, tile.shrink(8.0), current.family().icon(), accent);
        let x = tile.right() + 11.0;
        p.text(pos2(x, rect.center().y - 9.0), Align2::LEFT_CENTER, "ATELIER", theme::bold(10.5), pal.dim);
        let t = p.text(pos2(x, rect.center().y + 8.0), Align2::LEFT_CENTER, current.title(), theme::bold(14.5), pal.text);
        // chevron
        let c = pos2(t.right() + 12.0, rect.center().y + 8.0);
        let d = if self.open { -3.5 } else { 3.5 };
        p.add(Shape::line(vec![c + vec2(-4.5, -d * 0.6), c + vec2(0.0, d * 0.6), c + vec2(4.5, -d * 0.6)], Stroke::new(1.6, pal.weak)));
        if resp.clicked() {
            self.open = !self.open;
        }
        if !self.open {
            return None;
        }
        let ctx = ui.ctx().clone();
        let mut picked = None;
        let area = egui::Area::new(ui.scope_id().with("menu-ateliers")).order(Order::Foreground).fixed_pos(rect.left_bottom() + vec2(0.0, 6.0)).show(&ctx, |ui| {
            egui::Frame::new()
                .fill(pal.card)
                .stroke(Stroke::new(1.0, pal.line2))
                .corner_radius(CornerRadius::same(12))
                .shadow(egui::Shadow { offset: [0, 12], blur: 32, spread: 0, color: Color32::from_black_alpha(if pal.dark { 160 } else { 50 }) })
                .inner_margin(Margin::same(10))
                .show(ui, |ui| {
                    ui.set_width(MENU_W);
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new("ATELIERS").font(theme::bold(11.0)).color(pal.dim).extra_letter_spacing(0.9));
                    });
                    ui.add_space(6.0);
                    for at in Atelier::ALL {
                        if row(ui, pal, at, at == current) {
                            picked = Some(at);
                        }
                    }
                    ui.add_space(6.0);
                    let r = ui.available_rect_before_wrap();
                    ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, pal.card_line));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.add_space(6.0);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new("Les ateliers grisés arrivent dans une prochaine version. Chaque atelier pourra aussi fonctionner seul, en application autonome.")
                                    .size(12.0)
                                    .color(pal.weak),
                            )
                            .wrap(),
                        );
                    });
                    ui.add_space(2.0);
                });
        });
        // fermeture : choix fait, Échap, ou clic en dehors du bouton et de la liste
        let outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p) && !rect.contains(p)));
        if picked.is_some() || outside || ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.open = false;
        }
        picked.filter(|a| *a != current)
    }
}

/// Une ligne du menu ; renvoie vrai si elle a été choisie.
fn row(ui: &mut Ui, pal: &Palette, at: Atelier, current: bool) -> bool {
    let available = at.available();
    let h = if available { 70.0 } else { 56.0 };
    let (r, resp) = ui.allocate_exact_size(vec2(MENU_W, h), if available { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::MenuItem, available, at.title()));
    let p = ui.painter();
    let accent = if available { at.family().accent(pal) } else { pal.dim };
    let soft = theme::mix(pal.card, accent, 0.14);
    if current || (available && resp.hovered()) {
        p.rect_filled(r, CornerRadius::same(8), soft);
    }
    // filet d'accent à gauche
    p.rect_filled(Rect::from_min_max(pos2(r.left() + 2.0, r.top() + 8.0), pos2(r.left() + 5.0, r.bottom() - 8.0)), CornerRadius::same(2), accent);
    // tuile de l'icône
    let tile = Rect::from_center_size(pos2(r.left() + 38.0, r.center().y), vec2(42.0, 42.0));
    p.rect_filled(tile, CornerRadius::same(10), if available { theme::mix(pal.card, accent, 0.2) } else { pal.panel });
    icons::paint(p, tile.shrink(10.0), at.family().icon(), if available { accent } else { theme::alpha(pal.dim, 0.7) });
    // nom et badge
    let text = if available { pal.text } else { pal.dim };
    let x = r.left() + 72.0;
    let top = if available { r.top() + 16.0 } else { r.top() + 18.0 };
    let name = p.text(pos2(x, top), Align2::LEFT_CENTER, at.title(), theme::bold(14.5), text);
    let (badge, bg, fg) = if current {
        ("actif", accent, if pal.dark { pal.window } else { Color32::WHITE })
    } else if available {
        ("disponible", soft, accent)
    } else {
        ("à venir", pal.panel, pal.dim)
    };
    let g = p.layout_no_wrap(badge.to_owned(), theme::regular(11.5), fg);
    let br = Rect::from_min_size(pos2(name.right() + 9.0, top - 9.0), g.size() + vec2(14.0, 4.0));
    p.rect_filled(br, CornerRadius::same(255), bg);
    p.galley(br.min + vec2(7.0, 2.0), g, fg);
    // description et contenu, une ligne chacun
    let line = |t: &str, y: f32, c: Color32| {
        let mut job = egui::text::LayoutJob::simple_singleline(t.to_owned(), theme::regular(12.5), c);
        job.wrap.max_width = r.right() - x - 8.0;
        job.wrap.max_rows = 1;
        p.galley(pos2(x, y), p.layout_job(job), c);
    };
    line(at.description(), top + 11.0, if available { pal.weak } else { pal.dim });
    if available {
        line(at.contents(), top + 29.0, theme::mix(pal.weak, accent, 0.35));
    }
    available && resp.clicked()
}
