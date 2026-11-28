//! Vue 2D du dessin, pilotée par le moteur de CadCraft : on clique les points des commandes
//! (accroches sur les extrémités, milieux, centres, intersections), on tape des valeurs ou des
//! commandes dans la ligne de commande, Entrée valide, Échap annule, Suppr efface la sélection.
//! Molette : zoom autour du curseur ; bouton du milieu (ou clic gauche hors commande) : déplacer.

use std::sync::Arc;

use eframe::egui::{self, Align2, Color32, CornerRadius, Key, Painter, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, UiBuilder, pos2, vec2};
use makercraft_dao::sketch::engine::{Input, Session, View, snap};
use makercraft_dao::sketch::{cgeom, doc, render};
use makercraft_dao::Sketch;

use super::select_frame;
use crate::theme::{self, Palette};

/// Accroches actives : extrémité, milieu, centre, intersection, quadrant.
const OSMODE: u32 = 1 | 2 | 4 | 16 | 32;
/// Rayon de capture des accroches et des objets (pixels).
const APERTURE_PX: f64 = 10.0;
const CMDLINE_H: f32 = 34.0;

type WVec = cgeom::Vec2;

/// Transformation monde (mm) <-> écran pour la vue courante.
#[derive(Clone, Copy)]
pub struct Xf {
    rect: Rect,
    center: WVec,
    /// pixels par mm
    scale: f64,
}

impl Xf {
    pub fn to_screen(self, p: WVec) -> Pos2 {
        let c = self.rect.center();
        pos2(c.x + ((p.x - self.center.x) * self.scale) as f32, c.y - ((p.y - self.center.y) * self.scale) as f32)
    }

    pub fn to_world(self, s: Pos2) -> WVec {
        let c = self.rect.center();
        WVec::new(self.center.x + f64::from(s.x - c.x) / self.scale, self.center.y - f64::from(s.y - c.y) / self.scale)
    }

}

/// Ce que la vue rapporte à l'atelier.
#[derive(Default)]
pub struct Outcome {
    /// Le dessin a changé.
    pub changed: bool,
    pub status: Option<String>,
    /// Ouvrir la boîte des cotes de la sélection (Entrée).
    pub open_dims: bool,
}

#[derive(Default)]
pub struct SketchView {
    pub cmdline: String,
    /// Liste d'affichage en cache : dessin (pointeur) et finesse.
    cache: Option<(usize, f64, Arc<render::DisplayList>)>,
    snap: Option<snap::SnapHit>,
    cursor: Option<WVec>,
    /// Une vue a été choisie (sinon : cadrer au premier affichage).
    framed: bool,
    /// Le dessin se modifie (étape Dessin) ; sinon on ne fait que regarder.
    pub editable: bool,
    /// Ne pas marquer l'origine (zéro pièce) dans la vue.
    pub hide_origin: bool,
    /// Proportions déliées (Propriétés) : les poignées étirent sur un axe ; Ctrl inverse.
    pub free_scale: bool,
    /// Commandes de CadCraft refusées dans la ligne de commande (Options) : seules les valeurs
    /// demandées par une commande en cours y passent.
    pub no_commands: bool,
    /// Geste en cours sur le cadre de sélection.
    drag: Option<select_frame::Drag>,
    /// Coin de départ (écran) d'un rectangle de sélection.
    window: Option<Pos2>,
    /// Menu des vues : position et passe d'ouverture.
    zoom_menu: Option<(Pos2, u64)>,
    /// Finesse et transformation du dernier affichage.
    tol: f64,
    xf: Option<Xf>,
}

impl SketchView {
    /// Recadre la vue sur `b` (xmin, ymin, xmax, ymax) au prochain affichage.
    pub fn frame(&mut self, sketch: &mut Sketch, b: [f64; 4], rect_aspect: f64) {
        let w = (b[2] - b[0]).max(1.0);
        let h = (b[3] - b[1]).max(1.0);
        let height = (h.max(w / rect_aspect.max(0.1))) * 1.15;
        if let Ok(st) = sketch.session.state_mut() {
            st.set_view_quiet(View { center: WVec::new((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0), height, twist: 0.0 });
        }
        self.framed = true;
    }

    pub fn needs_frame(&self) -> bool {
        !self.framed
    }

    pub fn reset(&mut self) {
        self.cache = None;
        self.framed = false;
        self.cmdline.clear();
    }

    fn list(&mut self, sketch: &Sketch, tol: f64) -> Arc<render::DisplayList> {
        let key = sketch.session.state().map_or(0, |s| Arc::as_ptr(&s.doc) as usize);
        if let Some((k, t, l)) = &self.cache
            && *k == key
            && (*t / tol - 1.0).abs() < 0.5
        {
            return l.clone();
        }
        let l = Arc::new(sketch.display(tol));
        self.cache = Some((key, tol, l.clone()));
        l
    }

    /// Affiche la vue dans `rect`. `under` dessine par-dessous le dessin (brut, brides) et
    /// `over` par-dessus (trajectoires).
    pub fn show(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        pal: &Palette,
        sketch: &mut Sketch,
        under: impl FnOnce(&Painter, &Xf),
        over: impl FnOnce(&Painter, &Xf),
    ) -> Outcome {
        let mut out = Outcome::default();
        let editable = self.editable;
        let canvas = Rect::from_min_max(rect.min, pos2(rect.right(), rect.bottom() - if editable { CMDLINE_H + 6.0 } else { 0.0 }));
        let resp = ui.interact(canvas, ui.scope_id().with("dessin-2d"), Sense::click_and_drag());
        sketch.session.viewport_px = (f64::from(canvas.width()), f64::from(canvas.height()));
        let view = sketch.session.state().map(|st| st.view()).unwrap_or_default();
        let xf = Xf { rect: canvas, center: view.center, scale: f64::from(canvas.height()) / view.height.max(1e-9) };
        self.xf = Some(xf);
        let before = sketch.session.state().map_or(0, |st| Arc::as_ptr(&st.doc) as usize);
        let tol = 0.5 / xf.scale;
        self.tol = tol;
        let list = self.list(sketch, tol);
        let running = sketch.session.running.is_some();
        // cadre de sélection : objets choisis, modifiables, hors commande
        let frame_sel = if editable && !running { sketch.editable(&sketch.session.selection()) } else { Vec::new() };
        let frame = if frame_sel.is_empty() { None } else { select_frame::bbox(&list, &frame_sel) };
        let (shift, ctrl, pointer) = ui.input(|i| (i.modifiers.shift, i.modifiers.command, i.pointer.interact_pos()));
        let free = self.free_scale != ctrl;
        let cmd_id = ui.scope_id().with("ligne-de-commande");
        let typing = ui.memory(|m| m.has_focus(cmd_id));

        // ----------------------------------------------------------------- souris
        let s = &mut sketch.session;
        if let Some(hp) = resp.hover_pos() {
            let w = xf.to_world(hp);
            s.cursor = w;
            self.cursor = Some(w);
            self.snap = None;
            if editable && running && !s.picking_objects()
                && let Ok(d) = s.doc()
            {
                let base = s.current_prompt().and_then(|p| p.base);
                self.snap = snap::osnap(d, &doc::Space::Model, w, APERTURE_PX / xf.scale, OSMODE, base, false);
            }
            if self.drag.is_none()
                && let Some(b) = frame
                && let Some(g) = select_frame::hit(&xf, b, hp)
            {
                ui.ctx().set_cursor_icon(g.cursor());
            }
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                zoom_at(s, view, w, (-f64::from(scroll) / 300.0).exp());
            }
        } else {
            self.cursor = None;
            self.snap = None;
        }
        if editable && !running && resp.drag_started_by(egui::PointerButton::Primary)
            && let Some(o) = ui.input(|i| i.pointer.press_origin())
        {
            match frame.and_then(|b| select_frame::hit(&xf, b, o).map(|g| (b, g))) {
                Some((bbox, grip)) => self.drag = Some(select_frame::Drag { grip, start: xf.to_world(o), bbox }),
                None => self.window = Some(o),
            }
        }
        let pan = resp.dragged_by(egui::PointerButton::Middle) || (resp.dragged_by(egui::PointerButton::Primary) && !editable);
        if pan {
            let d = resp.drag_delta();
            let v = View { center: view.center - WVec::new(f64::from(d.x), -f64::from(d.y)) / xf.scale, ..view };
            if let Ok(st) = s.state_mut() {
                st.set_view_quiet(v);
            }
        }
        let active = self.drag.zip(pointer).map(|(d, p)| d.xform(xf.to_world(p), shift, free));
        if resp.drag_stopped() {
            if let (Some(x), Some(_)) = (active, self.drag.take()) {
                if let Err(e) = x.execute(sketch, &frame_sel) {
                    out.status = Some(e);
                }
            } else if let (Some(a), Some(b)) = (self.window.take(), pointer) {
                let mut pick = sketch.editable(&select_frame::window_pick(&list, xf.to_world(a), xf.to_world(b)));
                if shift {
                    for h in sketch.session.selection() {
                        if !pick.contains(&h) {
                            pick.push(h);
                        }
                    }
                }
                sketch.session.set_selection(pick);
            }
        }
        let s = &mut sketch.session;
        if editable && resp.clicked()
            && let Some(hp) = resp.interact_pointer_pos()
        {
            let w = xf.to_world(hp);
            let r = if s.running.is_some() {
                if s.picking_objects() {
                    match s.pick_object(&doc::Space::Model, w, APERTURE_PX / xf.scale) {
                        Ok(Some(h)) => s.input_object(h, Some(w)),
                        _ => Ok(()),
                    }
                } else if let Some(hit) = self.snap {
                    s.input_snapped(hit)
                } else {
                    s.input(Input::Point(w))
                }
            } else {
                // hors commande : un clic choisit l'objet sous le curseur (et lui seul), dans le
                // vide il vide la sélection ; Maj ajoute ou retire l'objet (Carbide Create, Galaad)
                match s.pick_object(&doc::Space::Model, w, APERTURE_PX / xf.scale) {
                    Ok(hit) => {
                        let mut sel = if shift { s.selection() } else { Vec::new() };
                        if let Some(h) = hit {
                            match sel.iter().position(|x| *x == h) {
                                Some(i) => {
                                    sel.remove(i);
                                }
                                None => sel.push(h),
                            }
                        }
                        s.set_selection(sel);
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            };
            if let Err(e) = r {
                out.status = Some(e.to_string());
            }
        }
        // bouton du milieu : menu des vues (le glissement, lui, déplace la vue)
        if resp.clicked_by(egui::PointerButton::Middle) {
            self.zoom_menu = resp.interact_pointer_pos().map(|p| (p, ui.ctx().cumulative_pass_nr()));
        }

        // ----------------------------------------------------------------- clavier
        if editable && (resp.hovered() || typing) {
            // Échap : d'abord le geste en cours, sinon la commande et la sélection
            if ui.input(|i| i.key_pressed(Key::Escape)) && self.drag.take().is_none() && self.window.take().is_none() {
                s.cancel();
                s.set_selection(Vec::new());
                self.cmdline.clear();
            }
            if !typing && ui.input(|i| i.key_pressed(Key::Delete)) && !s.selection().is_empty() && s.running.is_none() {
                out.status = s.cmdline("erase").err().map(|e| e.to_string());
                if s.running.is_some() {
                    let _ = s.input(Input::Enter);
                }
            }
            if !typing && frame.is_some() && ui.input(|i| i.key_pressed(Key::Enter)) {
                out.open_dims = true;
            }
        }

        // ----------------------------------------------------------------- dessin
        let p = ui.painter_at(canvas);
        p.rect_filled(canvas, CornerRadius::same(10), pal.canvas);
        grid(&p, &xf, pal, !self.hide_origin);
        under(&p, &xf);
        let list = self.list(sketch, tol);
        let sel = sketch.session.selection();
        paint_list(&p, &xf, &list, pal, &sel);
        over(&p, &xf);
        let frame_sel = if editable && sketch.session.running.is_none() { sketch.editable(&sel) } else { Vec::new() };
        if let Some(b) = select_frame::bbox(&list, &frame_sel) {
            select_frame::paint(&p, &xf, b, pal.orange, pal.canvas, &list, &frame_sel, active);
        }
        if let (Some(a), Some(b)) = (self.window, pointer) {
            select_frame::paint_window(&p, a, b, pal.blue);
        }
        let s = &mut sketch.session;
        if editable
            && s.running.is_some()
            && let Some(c) = self.cursor
            && let Ok(d) = s.doc()
        {
            let preview = s.preview(self.snap.as_ref().map_or(c, |h| h.point));
            if !preview.is_empty() {
                let opts = render::Options { tolerance: tol, ..render::Options::default() };
                let pl = render::build_entities(d, preview.iter(), &opts);
                paint_rubber(&p, &xf, &pl, pal.orange);
            }
        }
        if let Some(h) = &self.snap {
            let c = xf.to_screen(h.point);
            p.rect_stroke(Rect::from_center_size(c, vec2(11.0, 11.0)), CornerRadius::ZERO, Stroke::new(1.6, pal.orange), StrokeKind::Middle);
            p.text(c + vec2(10.0, -10.0), Align2::LEFT_BOTTOM, snap_label(h.name), theme::regular(11.5), pal.orange);
        }
        if let Some(c) = self.cursor {
            let pos = self.snap.as_ref().map_or(c, |h| h.point);
            p.text(canvas.right_bottom() - vec2(12.0, 10.0), Align2::RIGHT_BOTTOM, format!("X {:.2}   Y {:.2}", pos.x, pos.y), theme::mono(12.0), pal.weak);
        }
        p.rect_stroke(canvas, CornerRadius::same(10), Stroke::new(1.0, pal.card_line), StrokeKind::Inside);

        // ----------------------------------------------------------------- menus
        if editable {
            resp.context_menu(|ui| context_menu(ui, sketch, &list, &mut out));
        }
        self.zoom_popup(ui, sketch, &list, view);

        // ----------------------------------------------------------------- ligne de commande
        if editable {
            let cr = Rect::from_min_max(pos2(rect.left(), canvas.bottom() + 6.0), rect.max);
            self.command_line(ui, cr, pal, sketch, cmd_id, &mut out);
        }
        let after = sketch.session.state().map_or(0, |st| Arc::as_ptr(&st.doc) as usize);
        out.changed = before != after;
        out
    }

    /// Boîte englobante des objets `hs` (mm), d'après le dernier affichage.
    pub fn bbox_of(&mut self, sketch: &Sketch, hs: &[doc::Handle]) -> Option<select_frame::BBox> {
        let list = self.list(sketch, self.tol.max(1e-6));
        select_frame::bbox(&list, hs)
    }

    /// Recadre la vue sur `b` (mm), avec une marge.
    pub fn zoom_to(&mut self, sketch: &mut Sketch, b: [f64; 4]) {
        let aspect = self.xf.map_or(1.5, |x| f64::from(x.rect.width() / x.rect.height().max(1.0)));
        self.frame(sketch, b, aspect);
    }

    /// Menu des vues ouvert par un clic du bouton du milieu.
    fn zoom_popup(&mut self, ui: &mut Ui, sketch: &mut Sketch, list: &render::DisplayList, view: View) {
        let Some((at, opened)) = self.zoom_menu else { return };
        let mut pick = None;
        let r = egui::Area::new(ui.scope_id().with("menu-vues")).order(egui::Order::Foreground).fixed_pos(at).show(ui.ctx(), |ui| {
            egui::Frame::menu(ui.style()).show(ui, |ui| {
                ui.set_min_width(170.0);
                for (k, label) in [(0, "Tout voir"), (1, "Cadrer la sélection"), (2, "Cadrer le brut et le dessin"), (3, "Zoom avant"), (4, "Zoom arrière")] {
                    if ui.button(label).clicked() {
                        pick = Some(k);
                    }
                }
            });
        });
        let center = view.center;
        match pick {
            Some(0) => {
                let b = sketch.bounds();
                if !b.is_empty() {
                    self.zoom_to(sketch, [b.min.x, b.min.y, b.max.x, b.max.y]);
                }
            }
            Some(1) => {
                let sel = sketch.session.selection();
                if let Some(b) = select_frame::bbox(list, &sel) {
                    self.zoom_to(sketch, b);
                }
            }
            Some(2) => self.framed = false,
            Some(3) => zoom_at(&mut sketch.session, view, center, 0.5),
            Some(4) => zoom_at(&mut sketch.session, view, center, 2.0),
            _ => {}
        }
        let new_click = ui.ctx().cumulative_pass_nr() > opened && r.response.clicked_elsewhere();
        if pick.is_some() || new_click || ui.input(|i| i.key_pressed(Key::Escape)) {
            self.zoom_menu = None;
        }
    }

    fn command_line(&mut self, ui: &mut Ui, r: Rect, pal: &Palette, sketch: &mut Sketch, id: egui::Id, out: &mut Outcome) {
        let s = &mut sketch.session;
        let mut c = ui.new_child(UiBuilder::new().max_rect(r).layout(egui::Layout::left_to_right(egui::Align::Center)));
        let prompt = s.current_prompt();
        let msg = prompt.as_ref().map_or_else(|| "Commande".to_owned(), |p| translate(&p.message));
        c.label(RichText::new(msg).font(theme::bold(13.0)).color(if prompt.is_some() { pal.orange } else { pal.weak }));
        let mut kw_clicked = None;
        if let Some(p) = &prompt {
            for k in &p.keywords {
                if c.small_button(k).on_hover_text("Option de la commande").clicked() {
                    kw_clicked = Some(k.clone());
                }
            }
        }
        let te = egui::TextEdit::singleline(&mut self.cmdline)
            .id(id)
            .font(theme::mono(13.0))
            .hint_text(if prompt.is_some() {
                "valeur, x,y ou @dx,dy, Entrée"
            } else if self.no_commands {
                "commandes du ruban (commandes tapées : voir Options)"
            } else {
                "commande (ligne, cercle, rect...)"
            })
            .desired_width(c.available_width());
        let resp = c.add(te);
        if let Some(k) = kw_clicked {
            out.status = s.input(Input::Keyword(k)).err().map(|e| e.to_string());
        }
        if resp.lost_focus() && c.input(|i| i.key_pressed(Key::Enter)) {
            let text = self.cmdline.trim().to_owned();
            self.cmdline.clear();
            let r = if text.is_empty() {
                if s.running.is_some() {
                    s.input(Input::Enter).map_err(|e| e.to_string())
                } else {
                    // Entrée sur une ligne vide, hors commande : cotes de la sélection
                    out.open_dims = !s.selection().is_empty();
                    Ok(())
                }
            } else if s.running.is_none() && self.no_commands {
                Err("commandes de CadCraft désactivées (Options > Général > Dessin) : utiliser le ruban".to_owned())
            } else {
                s.cmdline(&command_alias(&text)).map_err(|e| e.to_string())
            };
            out.status = r.err();
            resp.request_focus();
        }
    }
}

/// Noms de commandes français acceptés dans la ligne de commande.
fn command_alias(t: &str) -> String {
    let lower = t.to_lowercase();
    let cmd = match lower.as_str() {
        "ligne" | "l" => "line",
        "polyligne" | "pl" => "pline",
        "rectangle" | "rect" => "rectang",
        "cercle" | "c" => "circle",
        "arc" | "a" => "arc",
        "effacer" | "supprimer" | "eff" => "erase",
        "deplacer" | "déplacer" | "dep" => "move",
        "copier" | "cp" => "copy",
        "rotation" | "ro" => "rotate",
        "miroir" | "mi" => "mirror",
        "decaler" | "décaler" | "dec" => "offset",
        "ajuster" | "aj" => "trim",
        "raccord" | "rac" => "fillet",
        "ellipse" | "el" => "ellipse",
        "polygone" | "pol" => "polygon",
        "echelle" | "échelle" | "ech" => "scale",
        "couper" | "cou" => "break",
        "prolonger" | "pro" => "extend",
        "joindre" | "jo" => "join",
        "decomposer" | "décomposer" | "dcp" => "explode",
        "chanfrein" | "cha" => "chamfer",
        "reseau" | "réseau" | "res" => "arrayrect",
        "annuler" | "u" => "undo",
        "retablir" | "rétablir" => "redo",
        _ => return t.to_owned(),
    };
    cmd.to_owned()
}

/// Invites de CadCraft en français (les autres restent en anglais).
fn translate(msg: &str) -> String {
    const T: &[(&str, &str)] = &[
        ("Specify first point", "Premier point"),
        ("Specify next point", "Point suivant"),
        ("Specify start point of arc", "Point de départ de l'arc"),
        ("Specify second point of arc", "Deuxième point de l'arc"),
        ("Specify end point of arc (hold Ctrl to switch direction)", "Extrémité de l'arc (Ctrl : inverser le sens)"),
        ("Specify end point of arc", "Extrémité de l'arc"),
        ("Specify endpoint of arc", "Extrémité de l'arc"),
        ("Specify center point of arc", "Centre de l'arc"),
        ("Specify radius of arc", "Rayon de l'arc"),
        ("Specify included angle", "Angle de l'arc"),
        ("Specify center point for circle", "Centre du cercle"),
        ("Specify radius of circle", "Rayon du cercle"),
        ("Specify diameter of circle", "Diamètre du cercle"),
        ("Specify first point on circle", "Premier point du cercle"),
        ("Specify second point on circle", "Deuxième point du cercle"),
        ("Specify third point on circle", "Troisième point du cercle"),
        ("Specify first corner point", "Premier coin"),
        ("Specify other corner point", "Coin opposé"),
        ("Specify center point for rectangle", "Centre du rectangle"),
        ("Specify fillet radius for rectangles", "Rayon des congés"),
        ("Specify rotation angle", "Angle de rotation"),
        ("Enter rectangle length", "Longueur du rectangle"),
        ("Enter rectangle width", "Largeur du rectangle"),
        ("Specify start point", "Point de départ"),
        ("Specify second point", "Deuxième point"),
        ("Specify base point", "Point de base"),
        ("Select objects", "Choisir les objets"),
        ("Specify offset distance", "Distance de décalage"),
    ];
    let m = msg.trim_end_matches(':').trim();
    if let Some((_, fr)) = T.iter().find(|(en, _)| m.starts_with(en)) {
        return (*fr).to_owned();
    }
    let cat = catalogue();
    if let Some(fr) = cat.get(m).or_else(|| cat.get(format!("{m}:").as_str())) {
        return fr.trim_end_matches(':').trim().to_owned();
    }
    // « Specify ... or [Undo] » : on traduit la partie avant les options
    for sep in [" or [", " [", " <"] {
        if let Some((head, tail)) = m.split_once(sep)
            && let Some(fr) = cat.get(head.trim())
        {
            return format!("{}{sep}{tail}", fr.trim_end_matches(':').trim());
        }
    }
    m.to_owned()
}

/// Catalogue français de CadCraft (`crates/ui-egui/src/i18n/fr.tsv` de CadCraft 0.20.1,
/// MIT OR Apache-2.0, traductions originales de @pechnikov2026-star), copié dans
/// `assets/cadcraft-fr.tsv` : anglais -> français, lignes sans contexte seulement.
fn catalogue() -> &'static std::collections::HashMap<&'static str, &'static str> {
    static CAT: std::sync::OnceLock<std::collections::HashMap<&'static str, &'static str>> = std::sync::OnceLock::new();
    CAT.get_or_init(|| {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/cadcraft-fr.tsv"))
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| {
                let mut it = l.split('\t');
                let (ctx, en, fr) = (it.next()?, it.next()?, it.next()?);
                (ctx.is_empty() && !fr.is_empty()).then_some((en.trim(), fr))
            })
            .collect()
    })
}

/// Zoom de `k` (moins de 1 : avant) autour du point `w` (mm).
fn zoom_at(s: &mut Session, view: View, w: WVec, k: f64) {
    let v = View { center: w + (view.center - w) * k, height: (view.height * k).clamp(0.01, 1e7), twist: 0.0 };
    if let Ok(st) = s.state_mut() {
        st.set_view_quiet(v);
    }
}

/// Menu du clic droit : celui de la commande en cours, sinon celui de la sélection, sinon les
/// tracés.
fn context_menu(ui: &mut Ui, sketch: &mut Sketch, list: &render::DisplayList, out: &mut Outcome) {
    ui.set_min_width(190.0);
    if sketch.session.running.is_some() {
        let s = &mut sketch.session;
        if ui.button("Valider (Entrée)").clicked() {
            out.status = status(s.input(Input::Enter));
        }
        let keywords = s.current_prompt().map(|p| p.keywords).unwrap_or_default();
        if !keywords.is_empty() {
            ui.separator();
        }
        for k in keywords {
            if ui.button(&k).clicked() {
                out.status = status(s.input(Input::Keyword(k)));
            }
        }
        ui.separator();
        if ui.button("Annuler la commande (Échap)").clicked() {
            s.cancel();
        }
        return;
    }
    let sel = sketch.editable(&sketch.session.selection());

    if sel.is_empty() {
        for (label, cmd) in [("Ligne", "line"), ("Polyligne", "pline"), ("Rectangle", "rectang"), ("Cercle", "circle"), ("Arc", "arc")] {
            if ui.button(label).clicked() {
                run_cmd(sketch, cmd, out);
            }
        }
        ui.separator();
        if ui.button("Tout sélectionner").clicked() {
            let mut all: Vec<doc::Handle> = Vec::new();
            for p in &list.prims {
                if !all.contains(&p.handle) {
                    all.push(p.handle);
                }
            }
            let all = sketch.editable(&all);
            sketch.session.set_selection(all);
        }
        ui.separator();
        if ui.button("Annuler").clicked() {
            run_cmd(sketch, "undo", out);
        }
        if ui.button("Rétablir").clicked() {
            run_cmd(sketch, "redo", out);
        }
        return;
    }
    if ui.button("Cotes de la sélection... (Entrée)").clicked() {
        out.open_dims = true;
    }
    ui.separator();
    for (label, cmd) in [
        ("Déplacer", "move"),
        ("Copier", "copy"),
        ("Rotation", "rotate"),
        ("Miroir", "mirror"),
        ("Échelle", "scale"),
        ("Décaler", "offset"),
        ("Décomposer", "explode"),
    ] {
        if ui.button(label).clicked() {
            run_cmd(sketch, cmd, out);
        }
    }
    ui.separator();
    if ui.button("Supprimer (Suppr)").clicked() {
        run_cmd(sketch, "erase", out);
    }
    if ui.button("Tout désélectionner (Échap)").clicked() {
        sketch.session.set_selection(Vec::new());
    }
}

/// Lance la commande `cmd` (sur la sélection) ; Supprimer s'applique tout de suite.
fn run_cmd(sketch: &mut Sketch, cmd: &str, out: &mut Outcome) {
    let s = &mut sketch.session;
    out.status = status(s.cmdline(cmd));
    if cmd == "erase" && s.running.is_some() {
        out.status = status(s.input(Input::Enter));
    }
}

fn status<T, E: std::fmt::Display>(r: Result<T, E>) -> Option<String> {
    r.err().map(|e| e.to_string())
}

fn snap_label(name: &str) -> &str {
    match name {
        "Endpoint" | "endpoint" => "extrémité",
        "Midpoint" | "midpoint" => "milieu",
        "Center" | "center" => "centre",
        "Intersection" | "intersection" => "intersection",
        "Quadrant" | "quadrant" => "quadrant",
        other => other,
    }
}

fn grid(p: &Painter, xf: &Xf, pal: &Palette, origin: bool) {
    let r = p.clip_rect();
    // pas de grille : environ 24 px, multiple de 1, 2, 5
    let target = 24.0 / xf.scale;
    let mag = 10f64.powf(target.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0].iter().map(|k| k * mag).find(|s| *s >= target).unwrap_or(mag * 10.0);
    let a = xf.to_world(r.left_bottom());
    let b = xf.to_world(r.right_top());
    let mut x = (a.x / step).floor() * step;
    let mut n = 0;
    while x <= b.x && n < 400 {
        let major = ((x / step).round() as i64) % 5 == 0;
        let sx = xf.to_screen(WVec::new(x, 0.0)).x;
        p.vline(sx, r.y_range(), Stroke::new(1.0, if major { pal.grid_major } else { pal.grid_minor }));
        x += step;
        n += 1;
    }
    let mut y = (a.y / step).floor() * step;
    n = 0;
    while y <= b.y && n < 400 {
        let major = ((y / step).round() as i64) % 5 == 0;
        let sy = xf.to_screen(WVec::new(0.0, y)).y;
        p.hline(r.x_range(), sy, Stroke::new(1.0, if major { pal.grid_major } else { pal.grid_minor }));
        y += step;
        n += 1;
    }
    if !origin {
        return;
    }
    // axes du zéro pièce
    let o = xf.to_screen(WVec::new(0.0, 0.0));
    p.line_segment([o, o + vec2(36.0, 0.0)], Stroke::new(1.6, pal.red));
    p.line_segment([o, o - vec2(0.0, 36.0)], Stroke::new(1.6, pal.green));
}

fn color_of(pal: &Palette, prim: &render::DPrim) -> Color32 {
    if prim.aci7 { pal.drawing } else { Color32::from_rgb(prim.color.0, prim.color.1, prim.color.2) }
}

fn paint_list(p: &Painter, xf: &Xf, list: &render::DisplayList, pal: &Palette, sel: &[doc::Handle]) {
    for prim in &list.prims {
        let selected = sel.contains(&prim.handle);
        let col = if selected { pal.blue } else { color_of(pal, prim) };
        let w = if selected { 2.4 } else { 1.4 };
        let (s, n) = (prim.start as usize, prim.len as usize);
        match prim.kind {
            render::Kind::Polyline => {
                if let Some(pts) = list.verts.get(s..s + n) {
                    let pts: Vec<Pos2> = pts.iter().map(|v| xf.to_screen(*v)).collect();
                    p.add(egui::Shape::line(pts, Stroke::new(w, col)));
                }
            }
            render::Kind::Tris => {
                if let Some(t) = list.tris.get(s..s + n) {
                    for tri in t.as_chunks::<3>().0 {
                        p.add(egui::Shape::convex_polygon(tri.iter().map(|v| xf.to_screen(*v)).collect(), col, Stroke::NONE));
                    }
                }
            }
            render::Kind::Point => {
                if let Some(v) = list.verts.get(s) {
                    p.circle_filled(xf.to_screen(*v), 2.0, col);
                }
            }
            _ => {}
        }
    }
}

fn paint_rubber(p: &Painter, xf: &Xf, list: &render::DisplayList, col: Color32) {
    for prim in &list.prims {
        if prim.kind != render::Kind::Polyline {
            continue;
        }
        let (s, n) = (prim.start as usize, prim.len as usize);
        if let Some(pts) = list.verts.get(s..s + n) {
            let pts: Vec<Pos2> = pts.iter().map(|v| xf.to_screen(*v)).collect();
            p.extend(egui::Shape::dashed_line(&pts, Stroke::new(1.4, col), 6.0, 4.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_roundtrips() {
        let xf = Xf { rect: Rect::from_min_size(pos2(10.0, 20.0), vec2(400.0, 300.0)), center: WVec::new(50.0, 25.0), scale: 3.0 };
        let w = WVec::new(12.5, -4.0);
        let back = xf.to_world(xf.to_screen(w));
        assert!((back.x - w.x).abs() < 1e-3 && (back.y - w.y).abs() < 1e-3);
    }

    #[test]
    fn french_aliases_and_prompts() {
        assert_eq!(command_alias("Cercle"), "circle");
        assert_eq!(command_alias("Décomposer"), "explode");
        // invite reprise du catalogue français de CadCraft
        assert_eq!(translate("Specify a point:"), "Spécifier un point");
        assert_eq!(translate("Specify axis endpoint of ellipse"), "Spécifier l'extrémité de l'axe de l'ellipse");
        assert_eq!(command_alias("10,20"), "10,20");
        assert_eq!(translate("Specify next point"), "Point suivant");
        assert_eq!(translate("Something else:"), "Something else");
    }
}
