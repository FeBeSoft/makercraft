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
         CREATE TABLE depth_override(material TEXT PRIMARY KEY, x_d REAL NOT NULL);",
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
    tx.commit()?;
    db.execute_batch("VACUUM;")?;
    Ok(())
}
