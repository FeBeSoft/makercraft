//! Ateliers de MakerCraft (menu « Atelier », comme l'« espace de travail » de SolidCraft).
//!
//! - **CNC** : DAO/CAO, FAO fraiseuse, usinage 3D, gravure d'images ;
//! - **Laser** ;
//! - **PCB** (complément CNC) ;
//! - **Machines** : infrastructure générique (laser, fraiseuse, tour...) : pilotage et périphériques.
//!
//! Un atelier pas encore réalisé affiche un écran « à venir » propre, sans fausse fonction.

pub mod cnc;
pub mod machines;

use eframe::egui::{self, Align2, CornerRadius, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::icons::{self, Icon};
use crate::theme::{self, Palette};

/// Grandes familles (onglets de la barre des ateliers).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Cnc,
    Laser,
    Pcb,
    Machines,
}

impl Family {
    pub const ALL: [Family; 4] = [Family::Cnc, Family::Laser, Family::Pcb, Family::Machines];

    pub fn label(self) -> &'static str {
        match self {
            Self::Cnc => "CNC",
            Self::Laser => "Laser",
            Self::Pcb => "PCB",
            Self::Machines => "Machines",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Self::Cnc => Icon::Cnc,
            Self::Laser => Icon::Laser,
            Self::Pcb => Icon::Pcb,
            Self::Machines => Icon::Machines,
        }
    }

    /// Atelier ouvert quand on choisit la famille.
    pub fn default_atelier(self) -> Atelier {
        match self {
            Self::Cnc => Atelier::CncDao,
            Self::Laser => Atelier::Laser,
            Self::Pcb => Atelier::Pcb,
            Self::Machines => Atelier::Machines,
        }
    }
}

/// Atelier affiché.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Atelier {
    CncDao,
    CncFao,
    CncUsinage3d,
    CncGravure,
    Laser,
    Pcb,
    Machines,
}

impl Atelier {
    pub const CNC: [Atelier; 4] = [Atelier::CncDao, Atelier::CncFao, Atelier::CncUsinage3d, Atelier::CncGravure];

    pub fn family(self) -> Family {
        match self {
            Self::CncDao | Self::CncFao | Self::CncUsinage3d | Self::CncGravure => Family::Cnc,
            Self::Laser => Family::Laser,
            Self::Pcb => Family::Pcb,
            Self::Machines => Family::Machines,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::CncDao => "DAO / CAO",
            Self::CncFao => "FAO fraiseuse",
            Self::CncUsinage3d => "Usinage 3D",
            Self::CncGravure => "Gravure d'images",
            Self::Laser => "Laser",
            Self::Pcb => "PCB",
            Self::Machines => "Machines",
        }
    }

    /// Nom complet, pour la barre d'état et les titres.
    pub fn title(self) -> String {
        match self.family() {
            Family::Cnc => format!("CNC - {}", self.label()),
            _ => self.label().to_owned(),
        }
    }

    pub fn available(self) -> bool {
        matches!(self, Self::CncDao | Self::CncFao | Self::Machines)
    }

    /// Ce que l'atelier apportera (écran « à venir »).
    fn plan(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::CncUsinage3d => (
                "Usinage de reliefs et de pièces 3D à partir des modèles SolidCraft.",
                &["Import STEP par le noyau SolidCraft (déjà lu dans DAO / CAO)", "Ébauche par niveaux et finition en balayage", "Simulation de l'enlèvement de matière"],
            ),
            Self::CncGravure => (
                "Gravure d'images en niveaux de gris à la fraise en V ou à la pointe.",
                &["Import d'images (PNG, JPG)", "Profondeur selon la luminosité", "Aperçu du relief obtenu"],
            ),
            Self::Laser => (
                "Découpe et gravure au laser, vectorielle et ligne à ligne.",
                &["Puissance et vitesse par calque, passes multiples", "Gravure d'images (tramage)", "Cadrage du travail avant le tir"],
            ),
            Self::Pcb => (
                "Fabrication de circuits imprimés à la fraiseuse, en complément de l'atelier CNC.",
                &["Import Gerber et Excellon", "Isolation des pistes et perçage", "Palpage de la planéité (carte de hauteur)"],
            ),
            Self::CncDao | Self::CncFao | Self::Machines => ("", &[]),
        }
    }
}

/// Écran « à venir » : ce que l'atelier fera, sans aucun bouton factice.
pub fn soon_screen(ui: &mut Ui, pal: &Palette, at: Atelier) {
    let (intro, items) = at.plan();
    let full = ui.available_rect_before_wrap();
    let w = 560.0_f32.min(full.width() - 40.0);
    let h = 300.0 + 30.0 * items.len() as f32;
    let rect = Rect::from_center_size(full.center(), vec2(w, h.min(full.height() - 20.0)));
    ui.allocate_rect(full, Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(16), pal.card);
    p.rect_stroke(rect, CornerRadius::same(16), Stroke::new(1.0, pal.card_line), StrokeKind::Inside);
    let ic = Rect::from_center_size(pos2(rect.center().x, rect.top() + 62.0), vec2(64.0, 64.0));
    p.circle_filled(ic.center(), 44.0, pal.orange_soft);
    icons::paint(p, ic.shrink(14.0), at.family().icon(), pal.orange);
    p.text(pos2(rect.center().x, rect.top() + 130.0), Align2::CENTER_CENTER, at.title(), theme::bold(22.0), pal.text);
    let badge = Rect::from_center_size(pos2(rect.center().x, rect.top() + 164.0), vec2(170.0, 28.0));
    p.rect_filled(badge, CornerRadius::same(255), pal.blue_soft);
    icons::paint(p, Rect::from_min_size(pos2(badge.left() + 14.0, badge.center().y - 8.0), vec2(16.0, 16.0)), Icon::Soon, pal.blue);
    p.text(pos2(badge.left() + 38.0, badge.center().y), Align2::LEFT_CENTER, "Atelier à venir", theme::bold(13.0), pal.blue);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(pos2(rect.left() + 40.0, rect.top() + 196.0), rect.max - vec2(40.0, 20.0))));
    child.label(RichText::new(intro).size(14.5).color(pal.text));
    child.add_space(6.0);
    for it in items {
        child.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(14.0, 18.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 3.5, pal.orange);
            ui.label(RichText::new(*it).size(14.0).color(pal.weak));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_atelier_belongs_to_its_family() {
        for f in Family::ALL {
            assert_eq!(f.default_atelier().family(), f);
        }
        for a in Atelier::CNC {
            assert_eq!(a.family(), Family::Cnc);
        }
        assert_eq!(Atelier::CncFao.title(), "CNC - FAO fraiseuse");
    }

    #[test]
    fn unfinished_ateliers_describe_their_plan() {
        for a in [Atelier::CncUsinage3d, Atelier::CncGravure, Atelier::Laser, Atelier::Pcb] {
            assert!(!a.available());
            let (intro, items) = a.plan();
            assert!(!intro.is_empty() && !items.is_empty(), "{a:?}");
        }
    }
}
