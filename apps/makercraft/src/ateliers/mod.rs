//! Ateliers de MakerCraft (menu « Atelier », comme l'« espace de travail » de SolidCraft).
//!
//! - **CNC** : fraisage 2,5D (de la machine à l'usinage : machine, brut, dessin, parcours,
//!   simulation, usinage), usinage 3D, gravure d'images ;
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
    pub fn icon(self) -> Icon {
        match self {
            Self::Cnc => Icon::Cnc,
            Self::Laser => Icon::Laser,
            Self::Pcb => Icon::Pcb,
            Self::Machines => Icon::Machines,
        }
    }
}

/// Atelier affiché.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Atelier {
    Cnc,
    CncUsinage3d,
    CncGravure,
    Laser,
    Pcb,
    Machines,
}

impl Family {
    /// Couleur d'accent de la famille (tuile du menu des ateliers, barre d'état).
    pub fn accent(self, pal: &crate::theme::Palette) -> eframe::egui::Color32 {
        match self {
            Self::Cnc => pal.blue,
            Self::Laser => pal.red,
            Self::Pcb => pal.green,
            Self::Machines => pal.orange,
        }
    }
}

impl Atelier {
    /// Tous les ateliers, dans l'ordre du menu.
    pub const ALL: [Atelier; 6] =
        [Atelier::Cnc, Atelier::CncUsinage3d, Atelier::CncGravure, Atelier::Laser, Atelier::Pcb, Atelier::Machines];

    /// Identifiant stable (préférences, ligne de commande).
    pub fn id(self) -> &'static str {
        match self {
            Self::Cnc => "cnc",
            Self::CncUsinage3d => "cnc.usinage3d",
            Self::CncGravure => "cnc.gravure",
            Self::Laser => "laser",
            Self::Pcb => "pcb",
            Self::Machines => "machines",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        // anciens identifiants (v0.1 : DAO/CAO et FAO séparés)
        let id = if matches!(id, "cnc.dao" | "cnc.fao") { "cnc" } else { id };
        Self::ALL.into_iter().find(|a| a.id() == id)
    }

    /// Une ligne : à quoi sert l'atelier (menu des ateliers).
    pub fn description(self) -> &'static str {
        match self {
            Self::Cnc => "De la machine à l'usinage : brut, dessin 2D, parcours, simulation 3D.",
            Self::CncUsinage3d => "Reliefs et pièces 3D à partir des modèles SolidCraft.",
            Self::CncGravure => "Gravure d'images en niveaux de gris.",
            Self::Laser => "Découpe et gravure au laser, vectorielle et ligne à ligne.",
            Self::Pcb => "Circuits imprimés à la fraiseuse : isolation et perçage.",
            Self::Machines => "Piloter les machines et leurs périphériques.",
        }
    }

    /// Ce que contient l'atelier (menu des ateliers) ; vide pour un atelier à venir.
    pub fn contents(self) -> &'static str {
        match self {
            Self::Cnc => "Machine · Brut · Dessin · Parcours · Simulation · Usinage",
            Self::Machines => "Fiche · Pilotage · Calibrage · Réglages Grbl · Entretien et journal",
            Self::CncUsinage3d | Self::CncGravure | Self::Laser | Self::Pcb => "",
        }
    }

    pub fn family(self) -> Family {
        match self {
            Self::Cnc | Self::CncUsinage3d | Self::CncGravure => Family::Cnc,
            Self::Laser => Family::Laser,
            Self::Pcb => Family::Pcb,
            Self::Machines => Family::Machines,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Cnc => "Fraisage 2,5D",
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
        matches!(self, Self::Cnc | Self::Machines)
    }

    /// Ce que l'atelier apportera (écran « à venir »).
    fn plan(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::CncUsinage3d => (
                "Usinage de reliefs et de pièces 3D à partir des modèles SolidCraft.",
                &["Import STEP par le noyau SolidCraft", "Ébauche par niveaux et finition en balayage", "Simulation de l'enlèvement de matière"],
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
            Self::Cnc | Self::Machines => ("", &[]),
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
        for a in [Atelier::Cnc, Atelier::CncUsinage3d, Atelier::CncGravure] {
            assert_eq!(a.family(), Family::Cnc);
        }
        assert_eq!(Atelier::Cnc.title(), "CNC - Fraisage 2,5D");
        assert_eq!(Atelier::from_id("cnc.fao"), Some(Atelier::Cnc));
    }

    #[test]
    fn ids_roundtrip_and_available_ateliers_describe_their_contents() {
        for a in Atelier::ALL {
            assert_eq!(Atelier::from_id(a.id()), Some(a));
            assert!(!a.description().is_empty());
            assert_eq!(a.available(), !a.contents().is_empty(), "{a:?}");
        }
        assert_eq!(Atelier::from_id("inconnu"), None);
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
