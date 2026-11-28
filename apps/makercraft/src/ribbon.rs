//! Ruban, sur le modèle de SolidCraft et DraftSight : une rangée d'onglets, puis des groupes
//! nommés (nom en bas, séparateurs verticaux) de grands boutons à icône, libellé dessous, et de
//! petits boutons empilés par trois. Les onglets sont fournis par l'atelier actif ; un clic
//! renvoie l'identifiant de la commande, que l'application transmet à l'atelier.
//! Le ruban se replie (double-clic sur l'onglet actif ou chevron à droite).

use eframe::egui::{self, Align, Align2, Color32, CornerRadius, Rect, Sense, Stroke, Ui, pos2, vec2};

use crate::icons::{self, Icon};
use crate::theme::{self, Palette};

/// Hauteur de la rangée d'onglets.
pub const TAB_ROW_H: f32 = 30.0;
/// Hauteur d'un groupe (boutons).
pub const GROUP_H: f32 = 86.0;
/// Hauteur du nom de groupe, sous les boutons.
pub const SECTION_H: f32 = 20.0;
/// Hauteur du corps du ruban déplié.
pub const BODY_H: f32 = GROUP_H + SECTION_H;

const LARGE_MIN_W: f32 = 54.0;
const LARGE_MAX_W: f32 = 76.0;
const SMALL_H: f32 = 25.0;
/// Largeur des bandes fléchées de défilement.
const ARROW_W: f32 = 16.0;

/// Couleur d'une commande : conception en bleu, composants et production en orange.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Plain,
    Design,
    Component,
    Danger,
}

pub struct Item {
    pub id: &'static str,
    pub label: &'static str,
    pub icon: Icon,
    pub large: bool,
    pub enabled: bool,
    /// Bascule : `Some(état)`.
    pub checked: Option<bool>,
    pub tone: Tone,
    pub tip: String,
    /// Liste déroulante (rangée de petits boutons) : (libellé, actif) de chaque choix ; vide pour
    /// un bouton. Le choix fait arrive par `Ribbon::picked`.
    pub choices: Vec<(String, bool)>,
    /// Rang du choix affiché.
    pub selected: usize,
    /// Largeur de la liste déroulante (px).
    pub width: f32,
    /// Rangée dans les fonctions masquées du groupe (flèche à droite du nom du groupe).
    pub more: bool,
}

impl Item {
    pub fn large(id: &'static str, label: &'static str, icon: Icon) -> Self {
        Self { id, label, icon, large: true, enabled: true, checked: None, tone: Tone::Plain, tip: String::new(), choices: Vec::new(), selected: 0, width: 0.0, more: false }
    }

    /// Liste déroulante dans une colonne de petits boutons.
    pub fn choice(id: &'static str, label: &'static str, icon: Icon, choices: Vec<(String, bool)>, selected: usize, width: f32) -> Self {
        Self { large: false, choices, selected, width, ..Self::large(id, label, icon) }
    }

    pub fn small(id: &'static str, label: &'static str, icon: Icon) -> Self {
        Self { large: false, ..Self::large(id, label, icon) }
    }

    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    pub fn checked(mut self, on: bool) -> Self {
        self.checked = Some(on);
        self
    }

    pub fn tone(mut self, t: Tone) -> Self {
        self.tone = t;
        self
    }

    /// Range la commande dans les fonctions masquées du groupe.
    pub fn more(mut self) -> Self {
        self.more = true;
        self
    }

    pub fn tip(mut self, t: impl Into<String>) -> Self {
        self.tip = t.into();
        self
    }
}

pub struct Group {
    pub label: &'static str,
    pub items: Vec<Item>,
}

pub struct Tab {
    pub label: &'static str,
    pub groups: Vec<Group>,
    /// Texte affiché quand l'onglet n'a aucun groupe.
    pub empty_note: &'static str,
}

impl Tab {
    pub fn new(label: &'static str, groups: Vec<Group>) -> Self {
        Self { label, groups, empty_note: "" }
    }
}

#[derive(Default)]
pub struct Ribbon {
    pub tab: usize,
    pub collapsed: bool,
    /// Dernier choix fait dans une liste déroulante : (identifiant, rang).
    pub picked: Option<(&'static str, usize)>,
    /// Décalage du défilement horizontal des groupes (px).
    pub scroll: f32,
    /// Largeur occupée par les groupes au dernier affichage (px).
    content_w: f32,
    /// Groupe dont les fonctions masquées sont ouvertes : (onglet, groupe).
    open: Option<(usize, usize)>,
}

fn tone_color(pal: &Palette, t: Tone) -> Color32 {
    match t {
        Tone::Plain => pal.text,
        Tone::Design => pal.blue,
        Tone::Component => pal.orange,
        Tone::Danger => pal.red,
    }
}

impl Ribbon {
    /// Rangée d'onglets dans `r` ; `accent` est la couleur de l'atelier actif.
    pub fn tabs_ui(&mut self, ui: &mut Ui, r: Rect, pal: &Palette, tabs: &[Tab], accent: Color32) {
        if self.tab >= tabs.len() {
            self.tab = 0;
        }
        let p = ui.painter().clone();
        let font = theme::regular(13.5);
        let mut x = r.left() + 6.0;
        for (i, t) in tabs.iter().enumerate() {
            let w = p.layout_no_wrap(t.label.to_owned(), font.clone(), pal.text).size().x + 26.0;
            let tr = Rect::from_min_size(pos2(x, r.top()), vec2(w, r.height()));
            let resp = ui.interact(tr, ui.scope_id().with(("onglet-ruban", i)), Sense::click());
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, i == self.tab, t.label));
            let active = i == self.tab;
            if resp.hovered() && !active {
                p.rect_filled(tr.shrink2(vec2(2.0, 4.0)), CornerRadius::same(6), pal.hover);
            }
            p.text(tr.center(), Align2::CENTER_CENTER, t.label, if active { theme::bold(13.5) } else { font.clone() }, if active { pal.text } else { pal.weak });
            if active && !self.collapsed {
                p.rect_filled(Rect::from_min_max(pos2(tr.left() + 8.0, tr.bottom() - 3.0), pos2(tr.right() - 8.0, tr.bottom())), CornerRadius::same(2), accent);
            }
            if resp.double_clicked() && active {
                self.collapsed = !self.collapsed;
            } else if resp.clicked() {
                if active && self.collapsed {
                    self.collapsed = false;
                }
                self.tab = i;
                if !active {
                    self.collapsed = false;
                }
            }
            x = tr.right() + 2.0;
        }
        // chevron : replier / déplier
        let cr = Rect::from_center_size(pos2(r.right() - 18.0, r.center().y), vec2(26.0, 22.0));
        let label = if self.collapsed { "Déplier le ruban" } else { "Replier le ruban" };
        let resp = ui.interact(cr, ui.scope_id().with("replier-ruban"), Sense::click()).on_hover_text(label);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
        if resp.hovered() {
            p.rect_filled(cr, CornerRadius::same(6), pal.hover);
        }
        icons::paint(&p, Rect::from_center_size(cr.center(), vec2(14.0, 14.0)), if self.collapsed { Icon::Expand } else { Icon::Collapse }, pal.weak);
        if resp.clicked() {
            self.collapsed = !self.collapsed;
        }
    }

    /// Corps du ruban (groupes de l'onglet actif) dans `r`. Renvoie la commande cliquée.
    ///
    /// Les commandes marquées `more()` ne s'affichent pas dans le groupe : la flèche à droite du
    /// nom du groupe les ouvre dans un petit ruban. Quand les groupes ne tiennent pas dans la
    /// largeur, une bande fléchée apparaît à l'extrémité qui cache encore des groupes, et
    /// seulement là.
    pub fn body_ui(&mut self, ui: &mut Ui, r: Rect, pal: &Palette, tabs: &[Tab]) -> Option<&'static str> {
        let tab = tabs.get(self.tab)?;
        if tab.groups.is_empty() {
            ui.painter().text(pos2(r.left() + 18.0, r.center().y), Align2::LEFT_CENTER, tab.empty_note, theme::regular(13.0), pal.weak);
            return None;
        }
        if self.open.is_some_and(|(t, _)| t != self.tab) {
            self.open = None;
        }
        let mut clicked = None;
        // ------------------------------------------------------------ défilement
        let over = self.content_w > r.width() + 0.5;
        let max_scroll = if over { self.content_w - r.width() + 2.0 * ARROW_W } else { 0.0 };
        self.scroll = self.scroll.clamp(0.0, max_scroll);
        let show_left = over && self.scroll > 0.5;
        let show_right = over && self.scroll < max_scroll - 0.5;
        let inner = Rect::from_min_max(
            pos2(r.left() + if show_left { ARROW_W } else { 0.0 }, r.top()),
            pos2(r.right() - if show_right { ARROW_W } else { 0.0 }, r.bottom()),
        );
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).id_salt("ruban-corps"));
        child.set_clip_rect(inner.intersect(ui.clip_rect()));
        let p = child.painter().clone();
        let origin = inner.left() + 6.0 - self.scroll;
        let mut x = origin;
        let top = r.top() + 4.0;
        let mut anchors: Vec<(usize, egui::Pos2)> = Vec::new();
        let mut arrow_clicked = false;
        for (gi, g) in tab.groups.iter().enumerate() {
            let start = x;
            let vis: Vec<&Item> = g.items.iter().filter(|it| !it.more).collect();
            let has_more = g.items.iter().any(|it| it.more);
            let mut i = 0;
            while i < vis.len() {
                let it = vis[i];
                if it.large {
                    let w = large_width(&p, it.label);
                    let br = Rect::from_min_size(pos2(x, top), vec2(w, GROUP_H - 8.0));
                    if button(&child, &p, pal, br, it, true) {
                        clicked = Some(it.id);
                    }
                    x = br.right() + 2.0;
                    i += 1;
                } else {
                    // colonne de petits boutons (trois au plus)
                    let col: Vec<&Item> = vis[i..].iter().copied().take_while(|it| !it.large).take(3).collect();
                    let font = theme::regular(12.5);
                    let w = col
                        .iter()
                        .map(|it| if it.choices.is_empty() { p.layout_no_wrap(it.label.to_owned(), font.clone(), pal.text).size().x + 36.0 } else { it.width })
                        .fold(0.0, f32::max);
                    let ch = col.len() as f32 * SMALL_H + (col.len().saturating_sub(1)) as f32 * 2.0;
                    let mut y = top + ((GROUP_H - 8.0) - ch) / 2.0;
                    for it in &col {
                        let br = Rect::from_min_size(pos2(x, y), vec2(w, SMALL_H));
                        if !it.choices.is_empty() {
                            if let Some(k) = choice(&mut child, &p, pal, br, it) {
                                self.picked = Some((it.id, k));
                            }
                        } else if button(&child, &p, pal, br, it, false) {
                            clicked = Some(it.id);
                        }
                        y += SMALL_H + 2.0;
                    }
                    x += w + 2.0;
                    i += col.len();
                }
            }
            let end = x.max(start + 40.0);
            let gr = Rect::from_min_max(pos2(start, r.top() + GROUP_H), pos2(end, r.bottom()));
            let label_w = p.layout_no_wrap(g.label.to_owned(), theme::regular(11.5), pal.dim).size().x;
            let total = label_w + if has_more { 17.0 } else { 0.0 };
            let lx = gr.center().x - total / 2.0;
            p.text(pos2(lx, gr.top() + 2.0), Align2::LEFT_TOP, g.label, theme::regular(11.5), pal.dim);
            if has_more {
                let ar = Rect::from_min_size(pos2(lx + label_w + 4.0, gr.top() + 1.0), vec2(13.0, 13.0));
                let open = self.open == Some((self.tab, gi));
                let name = format!("Autres fonctions du groupe {}", g.label);
                let resp = child.interact(ar, child.scope_id().with(("plus-groupe", gi)), Sense::click()).on_hover_text(&name);
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
                if open || resp.hovered() {
                    p.rect_filled(ar.expand(1.0), CornerRadius::same(3), if open { theme::mix(pal.bar, pal.blue, 0.35) } else { pal.hover });
                }
                p.rect_stroke(ar, CornerRadius::same(3), Stroke::new(1.0, if open { pal.blue } else { pal.bar_line }), egui::StrokeKind::Inside);
                icons::paint(&p, ar.shrink(2.5), Icon::Expand, if open { pal.text } else { pal.weak });
                if resp.clicked() {
                    self.open = if open { None } else { Some((self.tab, gi)) };
                    arrow_clicked = true;
                }
                anchors.push((gi, pos2(start, r.bottom() + 2.0)));
            }
            x = end + 8.0;
            p.vline(x - 4.0, (r.top() + 8.0)..=(r.bottom() - 8.0), Stroke::new(1.0, pal.bar_line));
            x += 4.0;
        }
        self.content_w = x - origin + 6.0;
        // ------------------------------------------------------------ flèches utiles seulement
        let step = (inner.width() * 0.6).max(80.0);
        if show_left {
            let ar = Rect::from_min_max(r.min, pos2(r.left() + ARROW_W, r.bottom()));
            if scroll_arrow(ui, pal, ar, false) {
                self.scroll -= step;
            }
        }
        if show_right {
            let ar = Rect::from_min_max(pos2(r.right() - ARROW_W, r.top()), r.max);
            if scroll_arrow(ui, pal, ar, true) {
                self.scroll += step;
            }
        }
        // ------------------------------------------------------------ fonctions masquées d'un groupe
        if let Some((_, gi)) = self.open
            && let (Some(g), Some(&(_, at))) = (tab.groups.get(gi), anchors.iter().find(|(k, _)| *k == gi))
        {
            let mut picked = None;
            let area = egui::Area::new(ui.scope_id().with(("ruban-plus", self.tab, gi))).order(egui::Order::Foreground).fixed_pos(at).show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for it in g.items.iter().filter(|it| it.more) {
                            let w = large_width(ui.painter(), it.label);
                            let (br, _) = ui.allocate_exact_size(vec2(w, GROUP_H - 8.0), Sense::hover());
                            let painter = ui.painter().clone();
                            if button(ui, &painter, pal, br, it, true) {
                                picked = Some(it.id);
                            }
                        }
                    });
                    ui.label(egui::RichText::new(format!("{}, autres fonctions", g.label)).size(11.0).color(pal.dim));
                });
            });
            if picked.is_some() {
                clicked = picked;
                self.open = None;
            } else if !arrow_clicked && (area.response.clicked_elsewhere() || ui.input(|i| i.key_pressed(egui::Key::Escape))) {
                self.open = None;
            }
        }
        clicked
    }
}

/// Bande fléchée d'une extrémité du ruban ; renvoie vrai si elle a été cliquée.
fn scroll_arrow(ui: &mut Ui, pal: &Palette, r: Rect, right: bool) -> bool {
    let label = if right { "Groupes suivants" } else { "Groupes précédents" };
    let resp = ui.interact(r, ui.scope_id().with(("ruban-defile", right)), Sense::click()).on_hover_text(label);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    let p = ui.painter();
    p.rect_filled(r, CornerRadius::same(4), if resp.hovered() { pal.hover } else { theme::mix(pal.bar, pal.blue, 0.12) });
    icons::paint(p, Rect::from_center_size(r.center(), vec2(12.0, 12.0)), if right { Icon::Right } else { Icon::Left }, pal.text);
    resp.clicked()
}

fn large_width(p: &egui::Painter, label: &str) -> f32 {
    let w = p.layout_no_wrap(label.to_owned(), theme::regular(12.5), Color32::WHITE).size().x + 16.0;
    w.clamp(LARGE_MIN_W, LARGE_MAX_W)
}

/// Liste déroulante du ruban (icône puis liste) ; renvoie le rang choisi.
fn choice(ui: &mut Ui, p: &egui::Painter, pal: &Palette, r: Rect, it: &Item) -> Option<usize> {
    let icon_c = if it.enabled { tone_color(pal, it.tone) } else { theme::alpha(pal.dim, 0.7) };
    icons::paint(p, Rect::from_center_size(pos2(r.left() + 12.0, r.center().y), vec2(16.0, 16.0)), it.icon, icon_c);
    let area = Rect::from_min_max(pos2(r.left() + 26.0, r.top()), r.max);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area).layout(egui::Layout::left_to_right(Align::Center)));
    child.spacing_mut().interact_size.y = SMALL_H - 2.0;
    let mut picked = None;
    let shown = it.choices.get(it.selected).map_or("", |c| c.0.as_str());
    child.add_enabled_ui(it.enabled, |ui| {
        let r = egui::ComboBox::from_id_salt(("ruban-liste", it.id)).width(area.width() - 8.0).selected_text(shown).show_ui(ui, |ui| {
            for (k, (label, on)) in it.choices.iter().enumerate() {
                if ui.add_enabled(*on, egui::Button::selectable(k == it.selected, label.as_str())).clicked() {
                    picked = Some(k);
                }
            }
        });
        let tip = if it.tip.is_empty() { it.label.to_owned() } else { format!("{} : {}", it.label, it.tip) };
        r.response.on_hover_text(tip).widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, it.enabled, it.label));
    });
    picked
}

/// Dessine un bouton du ruban ; renvoie vrai s'il a été cliqué.
fn button(ui: &Ui, p: &egui::Painter, pal: &Palette, r: Rect, it: &Item, large: bool) -> bool {
    let resp = ui.interact(r, ui.scope_id().with(("ruban", it.id)), if it.enabled { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| match it.checked {
        Some(on) => egui::WidgetInfo::selected(egui::Role::CheckBox, it.enabled, on, it.label),
        None => egui::WidgetInfo::labeled(egui::Role::Button, it.enabled, it.label),
    });
    let accent = tone_color(pal, it.tone);
    let on = it.checked == Some(true);
    if on {
        p.rect_filled(r, CornerRadius::same(7), theme::mix(pal.bar, pal.blue, 0.2));
        p.rect_stroke(r, CornerRadius::same(7), Stroke::new(1.0, theme::mix(pal.bar, pal.blue, 0.5)), egui::StrokeKind::Inside);
    } else if it.enabled && resp.hovered() {
        p.rect_filled(r, CornerRadius::same(7), pal.hover);
    }
    let (icon_c, text_c) = if !it.enabled {
        (theme::alpha(pal.dim, 0.7), pal.dim)
    } else if on {
        (pal.blue, pal.text)
    } else {
        (accent, pal.text)
    };
    if large {
        icons::paint(p, Rect::from_center_size(pos2(r.center().x, r.top() + 24.0), vec2(28.0, 28.0)), it.icon, icon_c);
        let mut job = egui::text::LayoutJob::simple(it.label.to_owned(), theme::regular(12.5), text_c, r.width() - 6.0);
        job.halign = Align::Center;
        job.wrap.max_rows = 2;
        p.galley(pos2(r.center().x, r.top() + 44.0), p.layout_job(job), text_c);
    } else {
        icons::paint(p, Rect::from_center_size(pos2(r.left() + 14.0, r.center().y), vec2(16.0, 16.0)), it.icon, icon_c);
        p.text(pos2(r.left() + 28.0, r.center().y), Align2::LEFT_CENTER, it.label, theme::regular(12.5), text_c);
    }
    let resp = if it.tip.is_empty() { resp } else { resp.on_hover_text(&it.tip) };
    it.enabled && resp.clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builders_set_fields() {
        let it = Item::small("a.b", "Libellé", Icon::Fit).enabled(false).checked(true).tone(Tone::Component).tip("aide");
        assert!(!it.large && !it.enabled);
        assert_eq!(it.checked, Some(true));
        assert_eq!(it.tone, Tone::Component);
        assert_eq!(it.tip, "aide");
        assert!(Item::large("a.c", "X", Icon::Open).large);
    }

    #[test]
    fn heights_follow_solidcraft() {
        assert!((BODY_H - 106.0).abs() < f32::EPSILON);
        assert!((TAB_ROW_H - 30.0).abs() < f32::EPSILON);
    }
}
