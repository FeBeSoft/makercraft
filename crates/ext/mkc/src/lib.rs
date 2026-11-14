//! Extension `mkc` : le projet MakerCraft.
//!
//! Un fichier `.mkc` est une archive zip qui contient :
//! - `makercraft.json` : les données MakerCraft ([`Project`] : machine, brut, brides, parcours) ;
//! - `dessin.dxf` ou `dessin.dwg` : le dessin 2D, au format natif de CadCraft ;
//! - `modele.sc` : le modèle 3D, au format natif de SolidCraft (facultatif).
//!
//! Les deux fichiers natifs restent tels quels : on peut les extraire et les ouvrir dans
//! CadCraft ou SolidCraft.

use std::io::{Cursor, Read, Write};
use std::path::Path;

use makercraft_ext_api::{CommandSpec, ExtError, Extension, Registry, Value, json};
use serde::{Deserialize, Serialize};

/// Extension des fichiers projet.
pub const EXT: &str = "mkc";
/// Version du format de `makercraft.json`.
pub const FORMAT: u32 = 1;
pub const PROJECT_ENTRY: &str = "makercraft.json";
pub const MODEL_ENTRY: &str = "modele.sc";

/// Machine choisie (étape 1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MachineSetup {
    /// Identifiant de la machine dans la banque d'outils (classe de machine, vitesses).
    pub tooldb_id: String,
    pub name: String,
    /// Courses X, Y, Z (mm).
    pub travel: [f64; 3],
    pub controller: String,
}

impl Default for MachineSetup {
    fn default() -> Self {
        Self { tooldb_id: String::new(), name: "CNC 3018 Pro".into(), travel: [300.0, 180.0, 45.0], controller: "Grbl 1.1".into() }
    }
}

/// Où se trouve le zéro pièce X0 Y0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum XyOrigin {
    #[default]
    AvantGauche,
    Centre,
}

impl XyOrigin {
    pub const ALL: [Self; 2] = [Self::AvantGauche, Self::Centre];
    pub fn label(self) -> &'static str {
        match self {
            Self::AvantGauche => "coin avant gauche",
            Self::Centre => "centre du brut",
        }
    }
}

/// Où se trouve le zéro pièce Z0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ZOrigin {
    #[default]
    Dessus,
    Dessous,
}

impl ZOrigin {
    pub const ALL: [Self; 2] = [Self::Dessus, Self::Dessous];
    pub fn label(self) -> &'static str {
        match self {
            Self::Dessus => "dessus du brut",
            Self::Dessous => "dessous du brut (martyr)",
        }
    }
}

/// Brut à usiner (étape 2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Stock {
    /// Largeur (X), profondeur (Y), épaisseur (Z), mm.
    pub size: [f64; 3],
    /// Matière (clé de la banque d'outils, ex. « bois tendre »).
    pub material: String,
    pub xy_origin: XyOrigin,
    pub z_origin: ZOrigin,
}

impl Default for Stock {
    fn default() -> Self {
        Self { size: [200.0, 150.0, 10.0], material: String::new(), xy_origin: XyOrigin::AvantGauche, z_origin: ZOrigin::Dessus }
    }
}

/// Zone de bridage : rectangle interdit à l'outil, avec la hauteur de la bride au-dessus du brut.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Clamp {
    pub name: String,
    /// Coin bas gauche et dimensions (mm, repère du brut : X0 Y0 au coin avant gauche).
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// Hauteur au-dessus du dessus du brut (mm).
    pub height: f64,
}

impl Default for Clamp {
    fn default() -> Self {
        Self { name: "Bride".into(), x: 0.0, y: 0.0, w: 20.0, h: 20.0, height: 15.0 }
    }
}

/// Type de parcours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PathKind {
    #[default]
    ContourExterieur,
    ContourInterieur,
    SurLeTrait,
    Poche,
    Percage,
    Gravure,
}

impl PathKind {
    pub const ALL: [Self; 6] = [Self::ContourExterieur, Self::ContourInterieur, Self::SurLeTrait, Self::Poche, Self::Percage, Self::Gravure];
    pub fn label(self) -> &'static str {
        match self {
            Self::ContourExterieur => "Contour extérieur",
            Self::ContourInterieur => "Contour intérieur",
            Self::SurLeTrait => "Sur le trait",
            Self::Poche => "Poche",
            Self::Percage => "Perçage",
            Self::Gravure => "Gravure",
        }
    }
}

/// Outil d'un parcours (copie des valeurs utiles, pour que le projet reste lisible sans la banque).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PathTool {
    /// Identifiant dans la banque d'outils (vide : outil saisi à la main).
    pub id: String,
    pub name: String,
    pub kind: String,
    pub diameter: f64,
    /// Angle (fraise en V), degrés.
    pub angle: f64,
    pub flutes: u32,
}

impl Default for PathTool {
    fn default() -> Self {
        Self { id: String::new(), name: "Fraise droite 3,175 mm".into(), kind: "flat".into(), diameter: 3.175, angle: 0.0, flutes: 2 }
    }
}

/// Parcours d'usinage (étape 4) : un outil qui suit les contours d'un ou plusieurs calques.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Toolpath {
    pub name: String,
    pub enabled: bool,
    pub kind: PathKind,
    /// Calques visés (un calque = un contour).
    pub layers: Vec<String>,
    pub tool: PathTool,
    /// Profondeur finale (mm, positive) et passe.
    pub depth: f64,
    pub stepdown: f64,
    /// Recouvrement des poches (mm).
    pub stepover: f64,
    /// Avance, plongée (mm/min), rotation (tr/min).
    pub feed: f64,
    pub plunge: f64,
    pub rpm: f64,
}

impl Default for Toolpath {
    fn default() -> Self {
        Self {
            name: "Parcours".into(),
            enabled: true,
            kind: PathKind::ContourExterieur,
            layers: Vec::new(),
            tool: PathTool::default(),
            depth: 3.0,
            stepdown: 1.0,
            stepover: 1.5,
            feed: 600.0,
            plunge: 200.0,
            rpm: 10_000.0,
        }
    }
}

/// Données MakerCraft d'un projet, dans l'ordre des étapes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Project {
    pub format: u32,
    pub machine: MachineSetup,
    pub stock: Stock,
    pub clamps: Vec<Clamp>,
    pub toolpaths: Vec<Toolpath>,
    /// Hauteur de sécurité au-dessus du brut (mm) ; relevée au-dessus des brides si besoin.
    pub safe_z: f64,
}

impl Default for Project {
    fn default() -> Self {
        Self { format: FORMAT, machine: MachineSetup::default(), stock: Stock::default(), clamps: Vec::new(), toolpaths: Vec::new(), safe_z: 5.0 }
    }
}

impl Project {
    /// Hauteur de dégagement réelle : au-dessus des brides les plus hautes.
    pub fn clearance(&self) -> f64 {
        self.clamps.iter().map(|c| c.height + 2.0).fold(self.safe_z, f64::max)
    }
}

/// Contenu d'un fichier `.mkc`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MkcFile {
    pub project: Project,
    /// Dessin CadCraft : nom dans l'archive (`dessin.dxf` ou `dessin.dwg`) et octets.
    pub drawing: Option<(String, Vec<u8>)>,
    /// Modèle SolidCraft (`modele.sc`).
    pub model: Option<Vec<u8>>,
}

fn zerr(e: impl std::fmt::Display) -> String {
    format!("archive .mkc : {e}")
}

impl MkcFile {
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let json = serde_json::to_vec_pretty(&self.project).map_err(zerr)?;
        z.start_file(PROJECT_ENTRY, opts).map_err(zerr)?;
        z.write_all(&json).map_err(zerr)?;
        if let Some((name, bytes)) = &self.drawing {
            z.start_file(name.as_str(), opts).map_err(zerr)?;
            z.write_all(bytes).map_err(zerr)?;
        }
        if let Some(bytes) = &self.model {
            z.start_file(MODEL_ENTRY, opts).map_err(zerr)?;
            z.write_all(bytes).map_err(zerr)?;
        }
        Ok(z.finish().map_err(zerr)?.into_inner())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let mut a = zip::ZipArchive::new(Cursor::new(bytes)).map_err(zerr)?;
        let read = |a: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str| -> Result<Option<Vec<u8>>, String> {
            match a.by_name(name) {
                Ok(mut f) => {
                    let mut v = Vec::new();
                    f.read_to_end(&mut v).map_err(zerr)?;
                    Ok(Some(v))
                }
                Err(zip::result::ZipError::FileNotFound) => Ok(None),
                Err(e) => Err(zerr(e)),
            }
        };
        let json = read(&mut a, PROJECT_ENTRY)?.ok_or_else(|| format!("{PROJECT_ENTRY} absent : ce n'est pas un projet MakerCraft"))?;
        let project: Project = serde_json::from_slice(&json).map_err(|e| format!("{PROJECT_ENTRY} : {e}"))?;
        if project.format > FORMAT {
            return Err(format!("projet écrit par une version plus récente de MakerCraft (format {})", project.format));
        }
        let mut drawing = None;
        for name in ["dessin.dwg", "dessin.dxf"] {
            if let Some(b) = read(&mut a, name)? {
                drawing = Some((name.to_owned(), b));
                break;
            }
        }
        let model = read(&mut a, MODEL_ENTRY)?;
        Ok(Self { project, drawing, model })
    }

    pub fn read(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{} : {e}", path.display()))?;
        Self::from_bytes(&bytes)
    }

    /// Écrit le projet (fichier temporaire puis renommage, pour ne jamais laisser un .mkc à moitié écrit).
    pub fn write(&self, path: &Path) -> Result<(), String> {
        let bytes = self.to_bytes()?;
        let tmp = path.with_extension("mkc.tmp");
        std::fs::write(&tmp, bytes).map_err(|e| format!("{} : {e}", tmp.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| format!("{} : {e}", path.display()))
    }
}

/// Extension accessible par commande.
pub struct MkcExtension;

impl Extension for MkcExtension {
    fn id(&self) -> &'static str {
        "mkc"
    }

    fn label(&self) -> &'static str {
        "Projets MakerCraft (.mkc)"
    }

    fn register(&self, r: &mut Registry) -> Result<(), ExtError> {
        r.add_command(CommandSpec { id: "mkc.info", label: "Contenu d'un projet MakerCraft", menu: "Projet", run: info })
    }
}

fn info(p: &Value) -> Result<Value, ExtError> {
    let path = p.get("path").and_then(Value::as_str).ok_or_else(|| ExtError::BadParams("attendu {\"path\": \"projet.mkc\"}".into()))?;
    let f = MkcFile::read(Path::new(path)).map_err(ExtError::Failed)?;
    Ok(json!({
        "machine": f.project.machine.name,
        "stock": f.project.stock.size,
        "clamps": f.project.clamps.len(),
        "toolpaths": f.project.toolpaths.iter().map(|t| json!({ "name": t.name, "kind": t.kind.label(), "layers": t.layers, "tool": t.tool.name })).collect::<Vec<_>>(),
        "drawing": f.drawing.as_ref().map(|d| d.0.clone()),
        "model": f.model.is_some(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keeps_everything() {
        let mut project = Project::default();
        project.clamps.push(Clamp { name: "B1".into(), x: -10.0, ..Clamp::default() });
        project.toolpaths.push(Toolpath { name: "Découpe".into(), layers: vec!["Contour 1".into()], ..Toolpath::default() });
        let f = MkcFile { project, drawing: Some(("dessin.dxf".into(), b"0\nEOF\n".to_vec())), model: Some(b"{}".to_vec()) };
        let bytes = match f.to_bytes() {
            Ok(b) => b,
            Err(e) => panic!("{e}"),
        };
        match MkcFile::from_bytes(&bytes) {
            Ok(back) => assert_eq!(back, f),
            Err(e) => panic!("{e}"),
        }
    }

    #[test]
    fn rejects_foreign_zip_and_newer_format() {
        let empty = MkcFile::default();
        let Ok(mut bytes) = empty.to_bytes() else { panic!("écriture") };
        assert!(MkcFile::from_bytes(&bytes).is_ok());
        bytes.truncate(10);
        assert!(MkcFile::from_bytes(&bytes).is_err());
        let newer = MkcFile { project: Project { format: FORMAT + 1, ..Project::default() }, ..MkcFile::default() };
        let Ok(b) = newer.to_bytes() else { panic!("écriture") };
        assert!(MkcFile::from_bytes(&b).is_err());
    }

    #[test]
    fn clearance_rises_above_clamps() {
        let mut p = Project::default();
        assert!((p.clearance() - 5.0).abs() < 1e-9);
        p.clamps.push(Clamp { height: 12.0, ..Clamp::default() });
        assert!((p.clearance() - 14.0).abs() < 1e-9);
    }
}
