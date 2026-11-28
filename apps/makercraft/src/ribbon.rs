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

/// Variante d'une commande (clic droit sur son bouton) : identifiant, libellé du menu, libellé
/// court affiché sur le bouton quand c'est la dernière utilisée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Variant {
    pub id: &'static str,
    pub label: &'static str,
    pub short: &'static str,
}

impl Variant {
    pub const fn new(id: &'static str, label: &'static str, short: &'static str) -> Self {
        Self { id, label, short }
    }
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
    /// Variantes (clic droit) ; la dernière utilisée est affichée et lancée par un clic.
    pub variants: Vec<Variant>,
}

impl Item {
    pub fn large(id: &'static str, label: &'static str, icon: Icon) -> Self {
        Self { id, label, icon, large: true, enabled: true, checked: None, tone: Tone::Plain, tip: String::new(), choices: Vec::new(), selected: 0, width: 0.0, more: false, variants: Vec::new() }
    }

    /// Liste déroulante dans une colonne de petits boutons.
    pub fn choice(id: &'static str, label: &'static str, icon: Icon, choices: Vec<(String, bool)>, selected: usize, width: f32) -> Self {
        Self { large: false, choices, selected, width, ..Self::large(id, label, icon) }
    }

    pub fn small(id: &'static str, label: &'static str, icon: Icon) -> Self {
        Self { large: false, ..Self::large(id, label, icon) }
    }

    /// Variantes au clic droit ; la première est celle par défaut.
    pub fn variants(mut self, v: Vec<Variant>) -> Self {
        self.variants = v;
        self
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
    /// Dernière variante utilisée de chaque commande : identifiant du bouton -> de la variante.
    pub chosen: std::collections::BTreeMap<String, String>,
    /// `chosen` a changé (à enregistrer dans les préférences).
    pub chosen_changed: bool,
    /// Barre des variantes ouverte : bouton, position, passe d'ouverture.
    flyout: Option<(&'static str, egui::Pos2, u64)>,
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
    /// Rangée d'onglets dans `r` ; `accent` est la couleur de l'atelier actif. `dot` : pastille
    /// cliquable après le titre d'un onglet (rang, couleur, info-bulle) ; renvoie vrai si on l'a
    /// cliquée.
    pub fn tabs_ui(&mut self, ui: &mut Ui, r: Rect, pal: &Palette, tabs: &[Tab], accent: Color32, dot: Option<(usize, Color32, &str)>) -> bool {
        let mut dot_clicked = false;
        if self.tab >= tabs.len() {
            self.tab = 0;
        }
        let p = ui.painter().clone();
        let font = theme::regular(13.5);
        let mut x = r.left() + 6.0;
        for (i, t) in tabs.iter().enumerate() {
            let this_dot = dot.filter(|(k, _, _)| *k == i);
            let extra = if this_dot.is_some() { 16.0 } else { 0.0 };
            let w = p.layout_no_wrap(t.label.to_owned(), font.clone(), pal.text).size().x + 26.0 + extra;
            let tr = Rect::from_min_size(pos2(x, r.top()), vec2(w, r.height()));
            let resp = ui.interact(tr, ui.scope_id().with(("onglet-ruban", i)), Sense::click());
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, i == self.tab, t.label));
            let active = i == self.tab;
            if resp.hovered() && !active {
                p.rect_filled(tr.shrink2(vec2(2.0, 4.0)), CornerRadius::same(6), pal.hover);
            }
            let label_c = pos2(tr.center().x - extra / 2.0, tr.center().y);
            let lr = p.text(label_c, Align2::CENTER_CENTER, t.label, if active { theme::bold(13.5) } else { font.clone() }, if active { pal.text } else { pal.weak });
            if let Some((_, col, tip)) = this_dot {
                let c = pos2(lr.right() + 9.0, tr.center().y);
                let dr = Rect::from_center_size(c, vec2(14.0, 14.0));
                let d = ui.interact(dr, ui.scope_id().with(("pastille-onglet", i)), Sense::click()).on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand);
                d.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tip));
                p.circle_filled(c, if d.hovered() { 5.0 } else { 4.0 }, col);
                dot_clicked |= d.clicked();
            }
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
        dot_clicked
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
                    let (c, menu) = button(&child, &p, pal, br, it, true, self.shown(it));
                    if c {
                        clicked = Some(self.shown(it).0);
                    }
                    if menu {
                        self.flyout = Some((it.id, br.left_bottom() + vec2(0.0, 2.0), ui.ctx().cumulative_pass_nr()));
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
                        } else {
                            let (c, menu) = button(&child, &p, pal, br, it, false, self.shown(it));
                            if c {
                                clicked = Some(self.shown(it).0);
                            }
                            if menu {
                                self.flyout = Some((it.id, br.left_bottom() + vec2(0.0, 2.0), ui.ctx().cumulative_pass_nr()));
                            }
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
            let mut flyout_req = None;
            let area = egui::Area::new(ui.scope_id().with(("ruban-plus", self.tab, gi))).order(egui::Order::Foreground).fixed_pos(at).show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for it in g.items.iter().filter(|it| it.more) {
                            let w = large_width(ui.painter(), it.label);
                            let (br, _) = ui.allocate_exact_size(vec2(w, GROUP_H - 8.0), Sense::hover());
                            let painter = ui.painter().clone();
                            let (c, menu) = button(ui, &painter, pal, br, it, true, self.shown(it));
                            if c {
                                picked = Some(self.shown(it).0);
                            }
                            if menu {
                                flyout_req = Some((it.id, br.left_bottom() + vec2(0.0, 2.0)));
                            }
                        }
                    });
                    ui.label(egui::RichText::new(format!("{}, autres fonctions", g.label)).size(11.0).color(pal.dim));
                });
            });
            if let Some((id, at)) = flyout_req {
                self.flyout = Some((id, at, ui.ctx().cumulative_pass_nr()));
            }
            if picked.is_some() {
                clicked = picked;
                self.open = None;
            } else if !arrow_clicked && self.flyout.is_none() && (area.response.clicked_elsewhere() || ui.input(|i| i.key_pressed(egui::Key::Escape))) {
                self.open = None;
            }
        }
        // ------------------------------------------------------------ variantes (clic droit)
        if let Some((id, at, opened)) = self.flyout
            && let Some(it) = tab.groups.iter().flat_map(|g| g.items.iter()).find(|it| it.id == id)
        {
            let cur = self.shown(it).0;
            let mut pick = None;
            let area = egui::Area::new(ui.scope_id().with(("ruban-variantes", id))).order(egui::Order::Foreground).fixed_pos(at).show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(200.0);
                    ui.label(egui::RichText::new(it.label).size(11.0).color(pal.dim));
                    for v in &it.variants {
                        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width().max(200.0), 26.0), Sense::click());
                        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, it.enabled, v.label));
                        if v.id == cur || resp.hovered() {
                            ui.painter().rect_filled(r, CornerRadius::same(5), if resp.hovered() { pal.hover } else { theme::mix(pal.bar, pal.blue, 0.2) });
                        }
                        icons::paint(ui.painter(), Rect::from_center_size(pos2(r.left() + 14.0, r.center().y), vec2(16.0, 16.0)), it.icon, tone_color(pal, it.tone));
                        ui.painter().text(pos2(r.left() + 30.0, r.center().y), Align2::LEFT_CENTER, v.label, theme::regular(12.5), pal.text);
                        if v.id == cur {
                            ui.painter().text(pos2(r.right() - 8.0, r.center().y), Align2::RIGHT_CENTER, "dernière", theme::regular(10.5), pal.dim);
                        }
                        if resp.clicked() && it.enabled {
                            pick = Some(v.id);
                        }
                    }
                });
            });
            if let Some(v) = pick {
                self.chosen.insert(id.to_owned(), v.to_owned());
                self.chosen_changed = true;
                self.flyout = None;
                self.open = None;
                clicked = Some(v);
            } else if (ui.ctx().cumulative_pass_nr() > opened && area.response.clicked_elsewhere()) || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.flyout = None;
            }
        } else {
            self.flyout = None;
        }
        clicked
    }

    /// Ce que montre le bouton `it` : identifiant lancé, libellé (variante choisie s'il en a).
    fn shown(&self, it: &Item) -> (&'static str, &'static str) {
        if it.variants.is_empty() {
            return (it.id, it.label);
        }
        let v = self.chosen.get(it.id).and_then(|c| it.variants.iter().find(|v| v.id == c)).or(it.variants.first());
        v.map_or((it.id, it.label), |v| (v.id, v.short))
    }
}

/// Bande fléchée d'une extrémité du ruban ; renvoie vrai si elle a été cliquée.
fn scroll_arrow(ui: &mut Ui, pal: &Palette, r: Rect, right: bool) -> bool {
    let label = if right { "Groupes suivants" } else { "Groupes précédents" };
    let resp = ui.interact(r, ui.scope_id().with(("ruban-defile", right)), Sense::click()).on_hover_text(label);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    let p = ui.painter();
    p.rect_filled(r, CornerRadius::same(4), if resp.hovered() { pal.hover } else { theme::mix(pal.bar, pal.blue, 0.12) });
    // chevron étroit, plus haut que large : « < » ou « > »
    let (c, h, w) = (r.center(), (r.height() * 0.32).min(14.0), (r.width() * 0.22).min(4.0));
    let s = if right { 1.0 } else { -1.0 };
    p.add(egui::Shape::line(vec![pos2(c.x - s * w, c.y - h), pos2(c.x + s * w, c.y), pos2(c.x - s * w, c.y + h)], egui::Stroke::new(1.8, pal.text)));
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
/// Bouton du ruban, montrant `shown` (identifiant, libellé) ; renvoie (clic, clic droit sur un
/// bouton à variantes).
fn button(ui: &Ui, p: &egui::Painter, pal: &Palette, r: Rect, it: &Item, large: bool, shown: (&'static str, &'static str)) -> (bool, bool) {
    let label = shown.1;
    let resp = ui.interact(r, ui.scope_id().with(("ruban", it.id)), if it.enabled { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| match it.checked {
        Some(on) => egui::WidgetInfo::selected(egui::Role::CheckBox, it.enabled, on, label),
        None => egui::WidgetInfo::labeled(egui::Role::Button, it.enabled, label),
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
        let mut job = egui::text::LayoutJob::simple(label.to_owned(), theme::regular(12.5), text_c, r.width() - 6.0);
        job.halign = Align::Center;
        job.wrap.max_rows = 2;
        p.galley(pos2(r.center().x, r.top() + 44.0), p.layout_job(job), text_c);
    } else {
        icons::paint(p, Rect::from_center_size(pos2(r.left() + 14.0, r.center().y), vec2(16.0, 16.0)), it.icon, icon_c);
        p.text(pos2(r.left() + 28.0, r.center().y), Align2::LEFT_CENTER, label, theme::regular(12.5), text_c);
    }
    let has_variants = !it.variants.is_empty();
    if has_variants {
        // petit coin en bas à droite : variantes au clic droit
        let c = if large { pos2(r.right() - 4.0, r.top() + 38.0) } else { pos2(r.right() - 3.0, r.bottom() - 3.0) };
        p.add(egui::Shape::convex_polygon(vec![c, c - vec2(6.0, 0.0), c - vec2(0.0, 6.0)], pal.weak, Stroke::NONE));
    }
    let resp = match (it.tip.is_empty(), has_variants) {
        (_, true) => resp.on_hover_text(if it.tip.is_empty() { "Clic droit : variantes".to_owned() } else { format!("{} ; clic droit : variantes", it.tip) }),
        (true, false) => resp,
        (false, false) => resp.on_hover_text(&it.tip),
    };
    (it.enabled && resp.clicked(), it.enabled && has_variants && resp.secondary_clicked())
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
