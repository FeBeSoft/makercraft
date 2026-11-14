//! Vue 3D de droite, en lecture seule : l'image vient du moteur de rendu de SolidCraft
//! (`makercraft_cao::view3d`). On peut tourner autour (glisser) et zoomer (molette) ; rien ne
//! s'y modifie. L'image n'est recalculée que si la scène, le point de vue ou la taille changent.

use eframe::egui::{self, Align2, ColorImage, CornerRadius, Rect, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, Ui, pos2, vec2};
use makercraft_cao::view3d::{Orbit, Scene3, render};

use crate::theme::{self, Palette};

#[derive(Default)]
pub struct View3d {
    pub orbit: Orbit,
    last: Option<(Scene3, Orbit, [usize; 2], bool)>,
    tex: Option<TextureHandle>,
}

impl View3d {
    pub fn home(&mut self) {
        self.orbit = Orbit::default();
    }

    pub fn show(&mut self, ui: &mut Ui, rect: Rect, pal: &Palette, scene: &Scene3, caption: &str) {
        let resp = ui.interact(rect, ui.scope_id().with("vue-3d"), Sense::click_and_drag());
        if resp.dragged() {
            let d = resp.drag_delta();
            self.orbit.yaw -= f64::from(d.x) * 0.01;
            self.orbit.pitch = (self.orbit.pitch + f64::from(d.y) * 0.01).clamp(-1.5, 1.5);
        }
        if resp.hovered() {
            let s = ui.input(|i| i.smooth_scroll_delta.y);
            if s != 0.0 {
                self.orbit.zoom = (self.orbit.zoom * (f64::from(s) / 300.0).exp()).clamp(0.2, 20.0);
            }
        }
        if resp.double_clicked() {
            self.home();
        }
        let ppp = ui.ctx().pixels_per_point();
        // pendant qu'on tourne, image à demi-résolution pour rester fluide
        let moving = resp.dragged();
        let k = if moving { 0.5 } else { 1.0 };
        let size = [(rect.width() * ppp * k).max(1.0) as usize, (rect.height() * ppp * k).max(1.0) as usize];
        let key = (scene.clone(), self.orbit, size, pal.dark);
        if self.last.as_ref() != Some(&key) {
            let bg = |c: egui::Color32| [c.r(), c.g(), c.b()];
            let rgba = render(scene, &self.orbit, size[0], size[1], (bg(pal.canvas), bg(theme::mix(pal.canvas, pal.panel, 0.6))));
            if rgba.len() == size[0] * size[1] * 4 {
                let img = ColorImage::from_rgba_unmultiplied(size, &rgba);
                match &mut self.tex {
                    Some(t) => t.set(img, TextureOptions::LINEAR),
                    None => self.tex = Some(ui.ctx().load_texture("vue-3d", img, TextureOptions::LINEAR)),
                }
            }
            self.last = Some(key);
        }
        let p = ui.painter_at(rect);
        p.rect_filled(rect, CornerRadius::same(10), pal.canvas);
        if let Some(t) = &self.tex {
            p.image(t.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);
        }
        p.text(rect.left_top() + vec2(12.0, 10.0), Align2::LEFT_TOP, "3D", theme::bold(12.0), pal.weak);
        p.text(rect.left_top() + vec2(36.0, 10.0), Align2::LEFT_TOP, caption, theme::regular(12.0), pal.dim);
        p.text(rect.right_bottom() - vec2(12.0, 10.0), Align2::RIGHT_BOTTOM, "glisser : tourner  ·  molette : zoom  ·  double-clic : vue de départ", theme::regular(11.0), pal.dim);
        p.rect_stroke(rect, CornerRadius::same(10), Stroke::new(1.0, pal.card_line), StrokeKind::Inside);
    }
}
