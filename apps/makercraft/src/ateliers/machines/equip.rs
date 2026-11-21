//! Équipements déclarés d'une machine, état de référence des axes, garde des déplacements
//! manuels et droits selon le mode d'utilisation (maker, prof, élève).
//!
//! Tout est sans interface pour être testé : la page Machines ne fait que l'afficher.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::prefs::Mode;

/// Équipements installés sur une machine, déclarés par l'utilisateur dans l'onglet Fiche.
/// Les commandes qui en dépendent (prise d'origine, palpage...) restent grisées tant qu'ils
/// ne sont pas cochés.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Equipment {
    /// Arrêt d'urgence branché sur le connecteur STOP de la carte.
    pub estop: bool,
    /// Fins de course X, Y, Z.
    pub limits: [bool; 3],
    /// Sonde de Z (plaque ou cylindre de palpage).
    pub probe: bool,
    /// Hauteur de la sonde (mm), face de contact comprise.
    pub probe_height_mm: f64,
    /// Diamètre de la sonde (mm).
    pub probe_diameter_mm: f64,
    /// Pointeur laser croix (repérage seulement).
    pub laser_cross: bool,
    /// Décalage du pointeur par rapport à l'axe de la broche, X et Y (mm).
    pub laser_offset_mm: [f64; 2],
    /// Module laser 12 V PWM à la place de la broche.
    pub laser_module: bool,
    /// Caméra de surveillance (caméra système vue par Windows).
    pub camera: bool,
    /// Ce qui est prévu pour chaque équipement absent (dans l'ordre de `Item::ALL`).
    pub plans: [Plan; 6],
    /// Photo de la machine (fichier de l'utilisateur).
    pub photo: Option<PathBuf>,
    /// Épaisseur de la planche martyr (mm).
    pub spoilboard_mm: f64,
    /// Corrections de calibrage (équerrage, planéité) appliquées d'office.
    pub corrections: bool,
}

impl Default for Equipment {
    fn default() -> Self {
        Self {
            estop: false,
            limits: [false; 3],
            probe: false,
            probe_height_mm: 19.5,
            probe_diameter_mm: 35.0,
            laser_cross: false,
            laser_offset_mm: [0.0, 0.0],
            laser_module: false,
            camera: false,
            plans: [Plan::Non; 6],
            photo: None,
            spoilboard_mm: 8.0,
            corrections: true,
        }
    }
}

/// Ce qui est prévu pour un équipement absent ; un clic sur l'état passe au suivant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Plan {
    #[default]
    Non,
    AVenir,
    ATester,
    AImprimer,
}

impl Plan {
    pub fn label(self) -> &'static str {
        match self {
            Self::Non => "non",
            Self::AVenir => "à venir",
            Self::ATester => "à tester",
            Self::AImprimer => "support à imprimer",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Non => Self::AVenir,
            Self::AVenir => Self::ATester,
            Self::ATester => Self::AImprimer,
            Self::AImprimer => Self::Non,
        }
    }
}

/// Équipements de la fiche, dans l'ordre d'affichage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    Estop,
    Limits,
    Probe,
    LaserCross,
    Camera,
    LaserModule,
}

impl Item {
    pub const ALL: [Self; 6] = [Self::Estop, Self::Limits, Self::Probe, Self::LaserCross, Self::Camera, Self::LaserModule];

    pub fn index(self) -> usize {
        self as usize
    }
}

impl Equipment {
    /// Équipement présent (fins de course : les trois axes).
    pub fn present(&self, i: Item) -> bool {
        match i {
            Item::Estop => self.estop,
            Item::Limits => self.can_home(),
            Item::Probe => self.probe,
            Item::LaserCross => self.laser_cross,
            Item::Camera => self.camera,
            Item::LaserModule => self.laser_module,
        }
    }

    pub fn set_present(&mut self, i: Item, on: bool) {
        match i {
            Item::Estop => self.estop = on,
            Item::Limits => self.limits = [on; 3],
            Item::Probe => self.probe = on,
            Item::LaserCross => self.laser_cross = on,
            Item::Camera => self.camera = on,
            Item::LaserModule => self.laser_module = on,
        }
    }

    /// Prise d'origine `$H` possible : fins de course sur les trois axes.
    pub fn can_home(&self) -> bool {
        self.limits.iter().all(|l| *l)
    }
}

/// Ce que vaut la position machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Reference {
    /// Position inconnue (mise sous tension, alarme, arrêt, reconnexion).
    #[default]
    None,
    /// Zéro table pris à la main (coin avant gauche), en attendant les fins de course.
    TableZero,
    /// Prise d'origine faite par les fins de course.
    Homed,
}

impl Reference {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "non référencé",
            Self::TableZero => "zéro table",
            Self::Homed => "référencé",
        }
    }

    pub fn valid(self) -> bool {
        self != Self::None
    }
}

/// Déplacement libre sans zéro valable (mm).
pub const FREE_MM: f64 = 1.0;
/// Déplacement accepté après une première validation (mm).
pub const FIRST_MM: f64 = 5.0;

/// Nombre de validations nécessaires pour un déplacement manuel de `dist` mm : aucune avec un
/// zéro valable ou sur l'émulateur ; sinon aucune sous 1 mm, une jusqu'à 5 mm, deux au-delà.
pub fn jog_validations(reference: Reference, emulator: bool, dist: f64) -> u8 {
    if emulator || reference.valid() || dist < FREE_MM {
        0
    } else if dist <= FIRST_MM {
        1
    } else {
        2
    }
}

/// Ce que le mode d'utilisation permet dans la page Machines.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rights {
    /// Modifier la fiche (équipements, martyr, corrections).
    pub edit_fiche: bool,
    /// Onglets Calibrage, Réglages Grbl, Entretien et journal.
    pub expert_tabs: bool,
    /// Plus grand pas de déplacement manuel (mm).
    pub max_step_mm: f64,
    /// Vitesse de déplacement manuel la plus haute (mm/min).
    pub max_jog_feed: f64,
    /// Mettre la broche en marche à la main.
    pub spindle: bool,
    /// Déverrouiller (`$X`) et réinitialiser Grbl.
    pub unlock: bool,
    /// Saisie libre dans la console.
    pub console_input: bool,
    /// Démarrer un programme (en mode élève : après validation du prof, à venir).
    pub run_program: bool,
}

impl Rights {
    pub fn of(mode: Mode) -> Self {
        match mode {
            Mode::Maker | Mode::Prof => Self {
                edit_fiche: true,
                expert_tabs: true,
                max_step_mm: f64::INFINITY,
                max_jog_feed: 2_000.0,
                spindle: true,
                unlock: true,
                console_input: true,
                run_program: true,
            },
            Mode::Eleve => Self {
                edit_fiche: false,
                expert_tabs: false,
                max_step_mm: 5.0,
                max_jog_feed: 1_000.0,
                spindle: false,
                unlock: false,
                console_input: false,
                run_program: false,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn homing_needs_three_limit_switches() {
        let mut e = Equipment::default();
        assert!(!e.can_home());
        e.limits = [true, true, false];
        assert!(!e.can_home());
        e.limits = [true; 3];
        assert!(e.can_home());
    }

    #[test]
    fn presence_and_plans() {
        let mut e = Equipment::default();
        e.set_present(Item::Limits, true);
        assert!(e.can_home() && e.present(Item::Limits));
        e.set_present(Item::Camera, true);
        assert!(e.camera);
        assert_eq!(Plan::AImprimer.next(), Plan::Non);
        assert_eq!(Item::ALL.iter().map(|i| i.index()).collect::<Vec<_>>(), vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn defaults_match_the_3018_probe() {
        let e = Equipment::default();
        assert!((e.probe_height_mm - 19.5).abs() < 1e-9);
        assert!((e.probe_diameter_mm - 35.0).abs() < 1e-9);
        assert!(e.corrections, "corrections appliquées d'office");
    }

    #[test]
    fn jog_guard_without_valid_zero() {
        let r = Reference::None;
        assert_eq!(jog_validations(r, false, 0.1), 0);
        assert_eq!(jog_validations(r, false, 1.0), 1);
        assert_eq!(jog_validations(r, false, 5.0), 1);
        assert_eq!(jog_validations(r, false, 10.0), 2);
        assert_eq!(jog_validations(r, true, 50.0), 0, "émulateur : rien à valider");
        assert_eq!(jog_validations(Reference::TableZero, false, 50.0), 0);
        assert_eq!(jog_validations(Reference::Homed, false, 50.0), 0);
    }

    #[test]
    fn pupil_mode_is_restricted() {
        let e = Rights::of(Mode::Eleve);
        assert!(!e.edit_fiche && !e.expert_tabs && !e.spindle && !e.unlock && !e.console_input && !e.run_program);
        assert!(e.max_step_mm <= 5.0 && e.max_jog_feed <= 1_000.0);
        let m = Rights::of(Mode::Maker);
        assert_eq!(m, Rights::of(Mode::Prof));
        assert!(m.edit_fiche && m.spindle && m.console_input);
    }

    #[test]
    fn equipment_json_fills_missing_fields() {
        let e: Equipment = serde_json::from_str(r#"{"probe":true}"#).unwrap_or_default();
        assert!(e.probe);
        assert!((e.probe_height_mm - 19.5).abs() < 1e-9);
    }
}
