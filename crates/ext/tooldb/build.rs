//! Compile la banque TOML (`<racine>/banque`) en une base SQLite `tools.db` dans OUT_DIR,
//! intégrée ensuite à l'exécutable par `include_bytes!`.
//! `banque/imported/` (données de logiciels tiers) n'est jamais lu ici.

use std::collections::hash_map::DefaultHasher;
use std::error::Error;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};
use serde::Deserialize;

type Res<T> = Result<T, Box<dyn Error>>;

#[derive(Deserialize)]
struct Materials {
    material: Vec<Material>,
}
#[derive(Deserialize)]
struct Material {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct Machines {
    machine: Vec<Machine>,
}
#[derive(Deserialize)]
struct Machine {
    id: String,
    name: String,
    rpm_min: f64,
    rpm_max: f64,
    fz_factor: f64,
    depth_factor: f64,
    max_diameter_mm: f64,
}

#[derive(Deserialize)]
struct Tools {
    tool: Vec<Tool>,
}
#[derive(Deserialize)]
struct Tool {
    id: String,
    name: String,
    #[serde(rename = "type")]
    kind: String,
    diameter_mm: f64,
    flutes: u32,
    angle_deg: Option<f64>,
    tip_diameter_mm: Option<f64>,
    flute_length_mm: Option<f64>,
    shank_mm: Option<f64>,
}

#[derive(Deserialize)]
struct Sorotec {
    url: String,
    version: String,
    diameters_mm: Vec<f64>,
    row: Vec<RefRow>,
    #[serde(default)]
    map: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    depth_override_x_d: std::collections::BTreeMap<String, f64>,
}
#[derive(Deserialize)]
struct RefRow {
    key: String,
    label: String,
    vc: f64,
    fz: Vec<f64>,
    depth_x_d: f64,
}

/// Profil de machine réelle (`banque/profils/*.toml`).
#[derive(Deserialize)]
struct Profile {
    id: String,
    name: String,
    class: String,
    source: String,
    travel_mm: [f64; 3],
    controller: String,
    connection: String,
    table: String,
    spindle: Spindle,
    collets_mm: Vec<f64>,
    tabs: Tabs,
    #[serde(default)]
    models: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    tool: Vec<ProfileTool>,
    #[serde(default)]
    cut: Vec<ProfileCut>,
}
#[derive(Deserialize)]
struct Spindle {
    name: String,
    power_w: f64,
    rpm_max: f64,
}
#[derive(Deserialize)]
struct Tabs {
    height_mm: f64,
    spacing_mm: f64,
}
#[derive(Deserialize)]
struct ProfileTool {
    number: u32,
    tool: String,
    usage: String,
}
#[derive(Deserialize)]
struct ProfileCut {
    material: String,
    tools: Vec<u32>,
    thickness_mm: Option<f64>,
    stepdown_mm: Option<[f64; 2]>,
    rpm: Option<[f64; 2]>,
    feed_mm_min: Option<[f64; 2]>,
    plunge_mm_min: Option<[f64; 2]>,
    note: Option<String>,
}

fn read(path: &Path, hasher: &mut DefaultHasher) -> Res<String> {
    println!("cargo:rerun-if-changed={}", path.display());
    let s = fs::read_to_string(path).map_err(|e| format!("{} : {e}", path.display()))?;
    s.hash(hasher);
    Ok(s)
}

fn main() -> Res<()> {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let banque = manifest.join("../../../banque");
    let out = PathBuf::from(std::env::var("OUT_DIR")?).join("tools.db");
    println!("cargo:rerun-if-changed={}", banque.join("tools").display());

    let mut h = DefaultHasher::new();
    let materials: Materials = toml::from_str(&read(&banque.join("materials.toml"), &mut h)?)?;
    let machines: Machines = toml::from_str(&read(&banque.join("machines.toml"), &mut h)?)?;
    let sorotec: Sorotec = toml::from_str(&read(&banque.join("references/sorotec.toml"), &mut h)?)?;
    let mut tool_files: Vec<PathBuf> = fs::read_dir(banque.join("tools"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    tool_files.sort();
    let mut tools = Vec::new();
    for f in &tool_files {
        let t: Tools = toml::from_str(&read(f, &mut h)?)?;
        let src = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        tools.extend(t.tool.into_iter().map(|t| (src.clone(), t)));
    }
    println!("cargo:rerun-if-changed={}", banque.join("profils").display());
    let mut profile_files: Vec<PathBuf> = fs::read_dir(banque.join("profils"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    profile_files.sort();
    let mut profiles = Vec::new();
    for f in &profile_files {
        let p: Profile = toml::from_str(&read(f, &mut h)?).map_err(|e| format!("{} : {e}", f.display()))?;
        check_profile(&p, &machines, &materials, &tools)?;
        profiles.push(p);
    }

    if out.exists() {
        fs::remove_file(&out)?;
    }
    let mut db = Connection::open(&out)?;
    db.execute_batch(
        "PRAGMA page_size=4096;
         CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
         CREATE TABLE material(id TEXT PRIMARY KEY, name TEXT NOT NULL);
         CREATE TABLE machine(id TEXT PRIMARY KEY, name TEXT NOT NULL, rpm_min REAL, rpm_max REAL,
                              fz_factor REAL, depth_factor REAL, max_diameter_mm REAL);
         CREATE TABLE tool(id TEXT PRIMARY KEY, name TEXT NOT NULL, type TEXT NOT NULL,
                           diameter_mm REAL NOT NULL, flutes INTEGER NOT NULL, angle_deg REAL,
                           tip_diameter_mm REAL, flute_length_mm REAL, shank_mm REAL, catalog TEXT);
         CREATE TABLE ref_row(key TEXT PRIMARY KEY, label TEXT, vc REAL NOT NULL, depth_x_d REAL NOT NULL);
         CREATE TABLE ref_fz(key TEXT NOT NULL, diameter_mm REAL NOT NULL, fz REAL NOT NULL);
         CREATE TABLE ref_map(material TEXT PRIMARY KEY, ref_key TEXT NOT NULL);
         CREATE TABLE depth_override(material TEXT PRIMARY KEY, x_d REAL NOT NULL);
         CREATE TABLE profile(id TEXT PRIMARY KEY, name TEXT NOT NULL, class TEXT NOT NULL,
                              source TEXT, travel_x REAL, travel_y REAL, travel_z REAL,
                              controller TEXT, connection TEXT, table_desc TEXT,
                              spindle TEXT, spindle_power_w REAL, spindle_rpm_max REAL,
                              tab_height_mm REAL, tab_spacing_mm REAL);
         CREATE TABLE profile_collet(profile TEXT NOT NULL, diameter_mm REAL NOT NULL);
         CREATE TABLE profile_model(profile TEXT NOT NULL, role TEXT NOT NULL, file TEXT NOT NULL);
         CREATE TABLE profile_tool(profile TEXT NOT NULL, number INTEGER NOT NULL, tool_id TEXT NOT NULL,
                                   collet_mm REAL NOT NULL, usage TEXT,
                                   PRIMARY KEY(profile, number));
         CREATE TABLE profile_cut(profile TEXT NOT NULL, material TEXT NOT NULL, tool_id TEXT NOT NULL,
                                  thickness_mm REAL, stepdown_min REAL, stepdown_max REAL,
                                  rpm_min REAL, rpm_max REAL, feed_min REAL, feed_max REAL,
                                  plunge_min REAL, plunge_max REAL, note TEXT,
                                  PRIMARY KEY(profile, material, tool_id));",
    )?;
    let tx = db.transaction()?;
    tx.execute("INSERT INTO meta VALUES('hash', ?1)", [format!("{:016x}", h.finish())])?;
    tx.execute("INSERT INTO meta VALUES('ref_source', ?1)", [format!("Sorotec {} - {}", sorotec.version, sorotec.url)])?;
    for m in &materials.material {
        tx.execute("INSERT INTO material VALUES(?1, ?2)", params![m.id, m.name])?;
    }
    for m in &machines.machine {
        tx.execute(
            "INSERT INTO machine VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![m.id, m.name, m.rpm_min, m.rpm_max, m.fz_factor, m.depth_factor, m.max_diameter_mm],
        )?;
    }
    for (src, t) in &tools {
        tx.execute(
            "INSERT INTO tool VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![t.id, t.name, t.kind, t.diameter_mm, t.flutes, t.angle_deg,
                    t.tip_diameter_mm, t.flute_length_mm, t.shank_mm, src],
        )?;
    }
    for r in &sorotec.row {
        if r.fz.len() != sorotec.diameters_mm.len() {
            return Err(format!("sorotec.toml : ligne {} incomplète", r.key).into());
        }
        tx.execute("INSERT INTO ref_row VALUES(?1,?2,?3,?4)", params![r.key, r.label, r.vc, r.depth_x_d])?;
        for (d, fz) in sorotec.diameters_mm.iter().zip(&r.fz) {
            tx.execute("INSERT INTO ref_fz VALUES(?1,?2,?3)", params![r.key, d, fz])?;
        }
        tx.execute("INSERT OR IGNORE INTO ref_map VALUES(?1,?1)", [&r.key])?;
    }
    for (mat, key) in &sorotec.map {
        tx.execute("INSERT OR REPLACE INTO ref_map VALUES(?1,?2)", params![mat, key])?;
    }
    for (mat, x) in &sorotec.depth_override_x_d {
        tx.execute("INSERT INTO depth_override VALUES(?1,?2)", params![mat, x])?;
    }
    for p in &profiles {
        insert_profile(&tx, p, &tools)?;
    }
    tx.commit()?;
    db.execute_batch("VACUUM;")?;
    Ok(())
}

fn lo(r: Option<[f64; 2]>) -> Option<f64> {
    r.map(|r| r[0])
}
fn hi(r: Option<[f64; 2]>) -> Option<f64> {
    r.map(|r| r[1])
}

/// Contrôles de cohérence : une erreur ici arrête la compilation, avec le fichier en cause.
fn check_profile(p: &Profile, machines: &Machines, materials: &Materials, tools: &[(String, Tool)]) -> Res<()> {
    let ctx = |m: String| format!("profil {} : {m}", p.id);
    if !machines.machine.iter().any(|m| m.id == p.class) {
        return Err(ctx(format!("classe de machine « {} » absente de machines.toml", p.class)).into());
    }
    let mut numbers = std::collections::BTreeSet::new();
    for t in &p.tool {
        if !numbers.insert(t.number) {
            return Err(ctx(format!("numéro d'outil {} en double", t.number)).into());
        }
        let Some((_, tool)) = tools.iter().find(|(_, x)| x.id == t.tool) else {
            return Err(ctx(format!("outil « {} » absent de tools/", t.tool)).into());
        };
        let shank = tool.shank_mm.unwrap_or(tool.diameter_mm);
        if !p.collets_mm.iter().any(|c| (c - shank).abs() < 1e-6) {
            return Err(ctx(format!("pas de pince pour la queue Ø{shank} de l'outil {}", t.number)).into());
        }
    }
    for c in &p.cut {
        if !materials.material.iter().any(|m| m.id == c.material) {
            return Err(ctx(format!("matériau « {} » absent de materials.toml", c.material)).into());
        }
        for n in &c.tools {
            if !numbers.contains(n) {
                return Err(ctx(format!("coupe {} : outil n°{n} inconnu", c.material)).into());
            }
        }
        for (nom, r) in [("passe", c.stepdown_mm), ("broche", c.rpm), ("avance", c.feed_mm_min), ("plongée", c.plunge_mm_min)] {
            if let Some([a, b]) = r
                && !(a > 0.0 && a <= b)
            {
                return Err(ctx(format!("coupe {} : plage {nom} [{a}, {b}] invalide", c.material)).into());
            }
        }
    }
    Ok(())
}

fn insert_profile(tx: &rusqlite::Transaction<'_>, p: &Profile, tools: &[(String, Tool)]) -> Res<()> {
    tx.execute(
        "INSERT INTO profile VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
        params![p.id, p.name, p.class, p.source, p.travel_mm[0], p.travel_mm[1], p.travel_mm[2],
                p.controller, p.connection, p.table, p.spindle.name, p.spindle.power_w,
                p.spindle.rpm_max, p.tabs.height_mm, p.tabs.spacing_mm],
    )?;
    for c in &p.collets_mm {
        tx.execute("INSERT INTO profile_collet VALUES(?1,?2)", params![p.id, c])?;
    }
    for (role, file) in &p.models {
        tx.execute("INSERT INTO profile_model VALUES(?1,?2,?3)", params![p.id, role, file])?;
    }
    for t in &p.tool {
        let shank = tools
            .iter()
            .find(|(_, x)| x.id == t.tool)
            .map(|(_, x)| x.shank_mm.unwrap_or(x.diameter_mm))
            .unwrap_or_default();
        tx.execute("INSERT INTO profile_tool VALUES(?1,?2,?3,?4,?5)", params![p.id, t.number, t.tool, shank, t.usage])?;
    }
    for c in &p.cut {
        for n in &c.tools {
            let Some(t) = p.tool.iter().find(|t| t.number == *n) else { continue };
            tx.execute(
                "INSERT INTO profile_cut VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
                params![p.id, c.material, t.tool, c.thickness_mm, lo(c.stepdown_mm), hi(c.stepdown_mm),
                        lo(c.rpm), hi(c.rpm), lo(c.feed_mm_min), hi(c.feed_mm_min),
                        lo(c.plunge_mm_min), hi(c.plunge_mm_min), c.note],
            )?;
        }
    }
    Ok(())
}
