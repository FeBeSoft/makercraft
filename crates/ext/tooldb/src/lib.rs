//! Banque d'outils coupants MakerCraft.
//!
//! La banque (`banque/*.toml` à la racine du dépôt) est compilée en SQLite par `build.rs`
//! et intégrée à l'exécutable. Au premier lancement, elle est copiée dans le dossier
//! utilisateur ; elle est remplacée automatiquement quand la banque intégrée change
//! (les outils personnels iront dans une base séparée, jamais écrasée).
//!
//! Les paramètres de coupe ne sont pas stockés : ils sont calculés par [`compute`] à partir
//! des valeurs Sorotec (vitesse de coupe vc, charge par dent fz selon le Ø), du matériau
//! et de la classe de machine (plage de broche, rigidité).

use std::f64::consts::PI;
use std::fs;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

/// Base SQLite produite par build.rs.
pub static EMBEDDED_DB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tools.db"));

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("base d'outils : {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("fichier : {0}")]
    Io(#[from] std::io::Error),
    #[error("introuvable : {0}")]
    NotFound(String),
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq)]
pub struct Tool {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub diameter_mm: f64,
    pub flutes: u32,
    pub angle_deg: Option<f64>,
    pub tip_diameter_mm: Option<f64>,
    pub flute_length_mm: Option<f64>,
    pub shank_mm: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Machine {
    pub id: String,
    pub name: String,
    pub rpm_min: f64,
    pub rpm_max: f64,
    pub fz_factor: f64,
    pub depth_factor: f64,
    pub max_diameter_mm: f64,
}

/// Ligne de référence fabricant pour un matériau : vc et fz par diamètre.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialRef {
    pub key: String,
    pub vc_m_min: f64,
    /// (diamètre mm, fz mm/dent), triés par diamètre croissant
    pub fz_table: Vec<(f64, f64)>,
    pub depth_x_d: f64,
}

/// Provenance des paramètres proposés.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Calculés depuis la référence fabricant et la classe de machine.
    Computed,
    /// Entièrement saisis par l'utilisateur.
    User,
    /// Valeurs utilisateur complétées par le calcul pour les champs non saisis.
    Mixed,
    /// Valeurs éprouvées de l'atelier (profil de machine), complétées par le calcul si besoin.
    Atelier,
}

/// Profil d'une machine réelle (`banque/profils/*.toml`) : s'appuie sur une classe de machine
/// pour le calcul et décrit la machine elle-même.
#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    pub id: String,
    pub name: String,
    /// Classe de machine (`machines.toml`) utilisée pour le calcul.
    pub class: String,
    pub source: String,
    /// Courses X, Y, Z (mm).
    pub travel_mm: [f64; 3],
    pub controller: String,
    pub connection: String,
    pub table: String,
    pub spindle: String,
    pub spindle_power_w: f64,
    pub spindle_rpm_max: f64,
    /// Pinces disponibles (Ø mm), croissantes.
    pub collets_mm: Vec<f64>,
    /// Ponts d'attache par défaut : hauteur et espacement (mm).
    pub tab_height_mm: f64,
    pub tab_spacing_mm: f64,
    /// (rôle, nom de fichier) des modèles 3D : table, martyr, écrou T, vis...
    pub models: Vec<(String, String)>,
}

impl Profile {
    /// `true` si la boîte `[min, max]` (mm, origine machine) tient dans les courses.
    pub fn fits(&self, min: [f64; 3], max: [f64; 3]) -> bool {
        (0..3).all(|i| min[i] >= -1e-6 && max[i] <= self.travel_mm[i] + 1e-6 && min[i] <= max[i])
    }
}

/// Outil numéroté d'un atelier, avec la pince à monter.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileTool {
    pub number: u32,
    pub tool: Tool,
    pub collet_mm: f64,
    pub usage: String,
}

/// Valeurs éprouvées saisies par l'utilisateur pour un trio outil / matériau / machine.
/// Un champ `None` est complété par le calcul.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UserCut {
    pub rpm: Option<f64>,
    pub feed_mm_min: Option<f64>,
    pub plunge_mm_min: Option<f64>,
    pub stepdown_mm: Option<f64>,
    pub stepover_mm: Option<f64>,
    pub note: Option<String>,
}

/// Paramètres de coupe proposés.
#[derive(Debug, Clone, PartialEq)]
pub struct CutParams {
    pub origin: Origin,
    pub rpm: f64,
    pub feed_mm_min: f64,
    pub plunge_mm_min: f64,
    pub stepdown_mm: f64,
    /// `None` pour les outils de gravure (pas de balayage latéral)
    pub stepover_mm: Option<f64>,
    pub fz_mm: f64,
    pub notes: Vec<String>,
}

impl MaterialRef {
    /// fz interpolé linéairement ; sous le plus petit Ø, proportionnel au Ø ; au-delà du plus grand, plafonné.
    pub fn fz_at(&self, d: f64) -> f64 {
        let t = &self.fz_table;
        let (Some(first), Some(last)) = (t.first(), t.last()) else { return 0.0 };
        if d <= first.0 {
            return first.1 * d / first.0;
        }
        if d >= last.0 {
            return last.1;
        }
        for w in t.windows(2) {
            let ((d0, f0), (d1, f1)) = (w[0], w[1]);
            if d <= d1 {
                return f0 + (f1 - f0) * (d - d0) / (d1 - d0);
            }
        }
        last.1
    }
}

/// Types d'outils couverts par le modèle (les fraises de forme, à fileter, coniques
/// hémisphériques… n'ont pas de modèle fiable : à renseigner à la main).
pub fn supported(kind: &str) -> bool {
    matches!(kind, "flat" | "ballnose" | "bullnose" | "vbit" | "engraver" | "drill")
}

/// Calcul des paramètres (modèle Sorotec + classe de machine).
pub fn compute(tool: &Tool, machine: &Machine, r: &MaterialRef, depth_x_d: f64) -> CutParams {
    let mut notes = Vec::new();
    let d = tool.diameter_mm.max(0.01);
    // diamètre réellement engagé : moitié du Ø pour un V-bit, pointe pour une pointe à graver
    let d_eff = match tool.kind.as_str() {
        "vbit" => (d / 2.0).max(tool.tip_diameter_mm.unwrap_or(0.0)),
        "engraver" => tool.tip_diameter_mm.unwrap_or(d).max(0.2),
        _ => d,
    };
    let z = f64::from(tool.flutes.max(1));
    let mut fz = r.fz_at(d_eff) * machine.fz_factor;
    if tool.kind == "drill" {
        fz *= 0.5;
        notes.push("perçage : fz réduit de moitié (règle prudente)".into());
    }
    let n_ideal = r.vc_m_min * 1000.0 / (PI * d_eff);
    let rpm = n_ideal.clamp(machine.rpm_min, machine.rpm_max);
    if n_ideal > machine.rpm_max * 1.05 {
        notes.push(format!(
            "vitesse idéale {:.0} tr/min, broche limitée à {:.0} : avance réduite d'autant",
            n_ideal, machine.rpm_max
        ));
    }
    let feed = rpm * z * fz;
    let plunge = if tool.kind == "drill" { feed } else { feed / 3.0 };
    let mut stepdown = depth_x_d * d_eff * machine.depth_factor;
    if let Some(fl) = tool.flute_length_mm {
        stepdown = stepdown.min(fl);
    }
    let stepover = match tool.kind.as_str() {
        "flat" | "bullnose" => Some(0.25 * d),
        "ballnose" | "tapered_ballnose" => Some(0.10 * d),
        _ => None,
    };
    if d > machine.max_diameter_mm {
        notes.push(format!("Ø{d} supérieur au Ø conseillé pour cette machine ({})", machine.max_diameter_mm));
    }
    CutParams {
        origin: Origin::Computed,
        rpm: rpm.round(),
        feed_mm_min: (feed / 10.0).round() * 10.0,
        plunge_mm_min: (plunge / 10.0).round() * 10.0,
        stepdown_mm: (stepdown * 100.0).round() / 100.0,
        stepover_mm: stepover.map(|s| (s * 100.0).round() / 100.0),
        fz_mm: (fz * 10000.0).round() / 10000.0,
        notes,
    }
}

/// Recouvre `base` (calcul, éventuellement absent) par des valeurs saisies.
/// Plongée non saisie : avance / 3 si l'avance est saisie, sinon celle de `base`.
/// `None` si rien d'utilisable (pas de base et ni tr/min ni avance).
fn overlay(base: Option<CutParams>, u: &UserCut, flutes: u32, full: Origin, partial: Origin, completed: &str) -> Option<CutParams> {
    let z = f64::from(flutes.max(1));
    let fz_of = |rpm: f64, feed: f64, dflt: f64| if rpm > 0.0 { (feed / (rpm * z) * 10000.0).round() / 10000.0 } else { dflt };
    let mut notes: Vec<String> = u.note.iter().cloned().collect();
    Some(match base {
        Some(c) => {
            let all = u.rpm.is_some() && u.feed_mm_min.is_some() && u.plunge_mm_min.is_some() && u.stepdown_mm.is_some();
            if !all {
                notes.push(completed.into());
            }
            let rpm = u.rpm.unwrap_or(c.rpm);
            let feed = u.feed_mm_min.unwrap_or(c.feed_mm_min);
            let plunge = match (u.plunge_mm_min, u.feed_mm_min) {
                (Some(p), _) => p,
                (None, Some(f)) => (f / 3.0 / 10.0).round() * 10.0,
                (None, None) => c.plunge_mm_min,
            };
            CutParams {
                origin: if all { full } else { partial },
                rpm,
                feed_mm_min: feed,
                plunge_mm_min: plunge,
                stepdown_mm: u.stepdown_mm.unwrap_or(c.stepdown_mm),
                stepover_mm: u.stepover_mm.or(c.stepover_mm),
                fz_mm: fz_of(rpm, feed, c.fz_mm),
                notes,
            }
        }
        // pas de modèle : seules les valeurs saisies, si elles suffisent
        None => {
            let (Some(rpm), Some(feed)) = (u.rpm, u.feed_mm_min) else { return None };
            CutParams {
                origin: full,
                rpm,
                feed_mm_min: feed,
                plunge_mm_min: u.plunge_mm_min.unwrap_or(feed / 3.0),
                stepdown_mm: u.stepdown_mm.unwrap_or(0.0),
                stepover_mm: u.stepover_mm,
                fz_mm: fz_of(rpm, feed, 0.0),
                notes,
            }
        }
    })
}

fn range_text(label: &str, lo: Option<f64>, hi: Option<f64>, unit: &str) -> Option<String> {
    match (lo, hi) {
        (Some(a), Some(b)) if (b - a).abs() > 1e-9 => Some(format!("{label} {a}-{b} {unit}")),
        _ => None,
    }
}

pub struct ToolDb {
    conn: Connection,
}

fn hash_of(conn: &Connection) -> Option<String> {
    conn.query_row("SELECT value FROM meta WHERE key='hash'", [], |r| r.get(0)).ok()
}

impl ToolDb {
    /// Ouvre `<dir>/tools.db`, en (re)copiant la banque intégrée si absente ou périmée.
    pub fn open(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir)?;
        let path = dir.join("tools.db");
        let embedded_hash = {
            let tmp = dir.join("tools.db.new");
            fs::write(&tmp, EMBEDDED_DB)?;
            let h = hash_of(&Connection::open(&tmp)?);
            let stale = !path.exists() || hash_of(&Connection::open(&path)?) != h;
            if stale {
                fs::rename(&tmp, &path)?;
            } else {
                fs::remove_file(&tmp)?;
            }
            h
        };
        let conn = Connection::open(&path)?;
        debug_assert_eq!(hash_of(&conn), embedded_hash);
        // base personnelle : jamais remplacée par une mise à jour de la banque
        let user = dir.join("user_tools.db");
        conn.execute("ATTACH DATABASE ?1 AS perso", [user.to_string_lossy()])?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS perso.user_tool(
                 id TEXT PRIMARY KEY, name TEXT NOT NULL, type TEXT NOT NULL,
                 diameter_mm REAL NOT NULL, flutes INTEGER NOT NULL, angle_deg REAL,
                 tip_diameter_mm REAL, flute_length_mm REAL, shank_mm REAL);
             CREATE TABLE IF NOT EXISTS perso.user_cut(
                 tool_id TEXT NOT NULL, material TEXT NOT NULL, machine TEXT NOT NULL,
                 rpm REAL, feed_mm_min REAL, plunge_mm_min REAL, stepdown_mm REAL, stepover_mm REAL,
                 note TEXT, updated TEXT NOT NULL DEFAULT (datetime('now')),
                 PRIMARY KEY(tool_id, material, machine));",
        )?;
        Ok(Self { conn })
    }

    /// Outils de la banque et outils personnels (un outil perso masque l'outil de même id).
    pub fn tools(&self) -> Result<Vec<Tool>> {
        let mut st = self.conn.prepare(
            "SELECT id,name,type,diameter_mm,flutes,angle_deg,tip_diameter_mm,flute_length_mm,shank_mm
               FROM perso.user_tool
             UNION ALL
             SELECT id,name,type,diameter_mm,flutes,angle_deg,tip_diameter_mm,flute_length_mm,shank_mm
               FROM main.tool WHERE id NOT IN (SELECT id FROM perso.user_tool)
             ORDER BY 3, 4, 5",
        )?;
        let rows = st.query_map([], row_tool)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn tool(&self, id: &str) -> Result<Tool> {
        const COLS: &str = "id,name,type,diameter_mm,flutes,angle_deg,tip_diameter_mm,flute_length_mm,shank_mm";
        for table in ["perso.user_tool", "main.tool"] {
            let sql = format!("SELECT {COLS} FROM {table} WHERE id=?1");
            if let Some(t) = self.conn.query_row(&sql, [id], row_tool).optional()? {
                return Ok(t);
            }
        }
        Err(Error::NotFound(format!("outil {id}")))
    }

    /// `true` si l'outil vient de la base personnelle.
    pub fn is_user_tool(&self, id: &str) -> Result<bool> {
        Ok(self
            .conn
            .query_row("SELECT 1 FROM perso.user_tool WHERE id=?1", [id], |_| Ok(()))
            .optional()?
            .is_some())
    }

    /// Ajoute ou remplace un outil personnel.
    pub fn save_user_tool(&self, t: &Tool) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO perso.user_tool VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![t.id, t.name, t.kind, t.diameter_mm, t.flutes, t.angle_deg,
                    t.tip_diameter_mm, t.flute_length_mm, t.shank_mm],
        )?;
        Ok(())
    }

    /// Supprime un outil personnel et ses valeurs éprouvées.
    pub fn delete_user_tool(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM perso.user_tool WHERE id=?1", [id])?;
        self.conn.execute("DELETE FROM perso.user_cut WHERE tool_id=?1", [id])?;
        Ok(())
    }

    /// Enregistre des valeurs éprouvées (prioritaires sur le calcul).
    pub fn save_user_cut(&self, tool_id: &str, material: &str, machine: &str, u: &UserCut) -> Result<()> {
        self.tool(tool_id)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO perso.user_cut
               (tool_id,material,machine,rpm,feed_mm_min,plunge_mm_min,stepdown_mm,stepover_mm,note)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![tool_id, material, machine, u.rpm, u.feed_mm_min, u.plunge_mm_min,
                    u.stepdown_mm, u.stepover_mm, u.note],
        )?;
        Ok(())
    }

    pub fn user_cut(&self, tool_id: &str, material: &str, machine: &str) -> Result<Option<UserCut>> {
        Ok(self
            .conn
            .query_row(
                "SELECT rpm,feed_mm_min,plunge_mm_min,stepdown_mm,stepover_mm,note FROM perso.user_cut
                 WHERE tool_id=?1 AND material=?2 AND machine=?3",
                params![tool_id, material, machine],
                |r| {
                    Ok(UserCut {
                        rpm: r.get(0)?,
                        feed_mm_min: r.get(1)?,
                        plunge_mm_min: r.get(2)?,
                        stepdown_mm: r.get(3)?,
                        stepover_mm: r.get(4)?,
                        note: r.get(5)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn delete_user_cut(&self, tool_id: &str, material: &str, machine: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM perso.user_cut WHERE tool_id=?1 AND material=?2 AND machine=?3",
            params![tool_id, material, machine],
        )?;
        Ok(())
    }

    pub fn machines(&self) -> Result<Vec<Machine>> {
        let mut st = self.conn.prepare(
            "SELECT id,name,rpm_min,rpm_max,fz_factor,depth_factor,max_diameter_mm FROM machine",
        )?;
        let rows = st.query_map([], row_machine)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn machine(&self, id: &str) -> Result<Machine> {
        self.conn
            .query_row(
                "SELECT id,name,rpm_min,rpm_max,fz_factor,depth_factor,max_diameter_mm FROM machine WHERE id=?1",
                [id],
                row_machine,
            )
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("machine {id}")))
    }

    /// (id, libellé) des matériaux.
    pub fn materials(&self) -> Result<Vec<(String, String)>> {
        let mut st = self.conn.prepare("SELECT id,name FROM material ORDER BY id")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Référence fabricant d'un matériau, `None` s'il n'y en a pas (PCB, composites, pierre…).
    pub fn material_ref(&self, material: &str) -> Result<Option<(MaterialRef, f64)>> {
        let Some((key, vc, depth)): Option<(String, f64, f64)> = self
            .conn
            .query_row(
                "SELECT r.key, r.vc, r.depth_x_d FROM ref_map m JOIN ref_row r ON r.key=m.ref_key
                 WHERE m.material=?1",
                [material],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?
        else {
            return Ok(None);
        };
        let depth: f64 = self
            .conn
            .query_row("SELECT x_d FROM depth_override WHERE material=?1", [material], |r| r.get(0))
            .optional()?
            .unwrap_or(depth);
        let mut st = self.conn.prepare("SELECT diameter_mm, fz FROM ref_fz WHERE key=?1 ORDER BY diameter_mm")?;
        let fz_table = st
            .query_map(params![key], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<std::result::Result<_, _>>()?;
        Ok(Some((MaterialRef { key, vc_m_min: vc, fz_table, depth_x_d: depth }, depth)))
    }

    /// Paramètres calculés seuls (sans les valeurs personnelles).
    pub fn compute_for(&self, tool_id: &str, material: &str, machine_id: &str) -> Result<Option<CutParams>> {
        let tool = self.tool(tool_id)?;
        if !supported(&tool.kind) {
            return Ok(None);
        }
        let machine = self.machine(machine_id)?;
        Ok(self.material_ref(material)?.map(|(r, depth)| compute(&tool, &machine, &r, depth)))
    }

    /// Paramètres proposés : valeurs éprouvées de l'utilisateur en priorité,
    /// complétées par le calcul pour les champs non saisis.
    pub fn suggest(&self, tool_id: &str, material: &str, machine_id: &str) -> Result<Option<CutParams>> {
        let computed = self.compute_for(tool_id, material, machine_id)?;
        let Some(u) = self.user_cut(tool_id, material, machine_id)? else {
            return Ok(computed);
        };
        let flutes = self.tool(tool_id)?.flutes;
        Ok(overlay(computed, &u, flutes, Origin::User, Origin::Mixed, "valeurs perso complétées par le calcul"))
    }

    /// Profils de machines réelles.
    pub fn profiles(&self) -> Result<Vec<Profile>> {
        let ids: Vec<String> = {
            let mut st = self.conn.prepare("SELECT id FROM profile ORDER BY name")?;
            let rows = st.query_map([], |r| r.get(0))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        ids.iter().map(|id| self.profile(id)).collect()
    }

    pub fn profile(&self, id: &str) -> Result<Profile> {
        let mut p = self
            .conn
            .query_row(
                "SELECT id,name,class,source,travel_x,travel_y,travel_z,controller,connection,table_desc,
                        spindle,spindle_power_w,spindle_rpm_max,tab_height_mm,tab_spacing_mm
                   FROM profile WHERE id=?1",
                [id],
                |r| {
                    Ok(Profile {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        class: r.get(2)?,
                        source: r.get(3)?,
                        travel_mm: [r.get(4)?, r.get(5)?, r.get(6)?],
                        controller: r.get(7)?,
                        connection: r.get(8)?,
                        table: r.get(9)?,
                        spindle: r.get(10)?,
                        spindle_power_w: r.get(11)?,
                        spindle_rpm_max: r.get(12)?,
                        collets_mm: Vec::new(),
                        tab_height_mm: r.get(13)?,
                        tab_spacing_mm: r.get(14)?,
                        models: Vec::new(),
                    })
                },
            )
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("profil {id}")))?;
        let mut st = self.conn.prepare("SELECT diameter_mm FROM profile_collet WHERE profile=?1 ORDER BY diameter_mm")?;
        p.collets_mm = st.query_map([id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        let mut st = self.conn.prepare("SELECT role, file FROM profile_model WHERE profile=?1 ORDER BY role")?;
        p.models = st.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
        Ok(p)
    }

    /// Outils numérotés de l'atelier d'un profil, par numéro croissant.
    pub fn profile_tools(&self, profile: &str) -> Result<Vec<ProfileTool>> {
        let rows: Vec<(u32, String, f64, String)> = {
            let mut st = self.conn.prepare(
                "SELECT number, tool_id, collet_mm, usage FROM profile_tool WHERE profile=?1 ORDER BY number",
            )?;
            let rows = st.query_map([profile], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        rows.into_iter()
            .map(|(number, id, collet_mm, usage)| Ok(ProfileTool { number, tool: self.tool(&id)?, collet_mm, usage }))
            .collect()
    }

    /// Valeurs éprouvées de l'atelier pour un outil et un matériau : milieu de la plage pour la
    /// broche, bas de la plage pour l'avance, la plongée et la passe (machine peu rigide).
    pub fn atelier_cut(&self, profile: &str, tool_id: &str, material: &str) -> Result<Option<UserCut>> {
        type Row = (Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<String>);
        let row: Option<Row> = self
            .conn
            .query_row(
                "SELECT stepdown_min,stepdown_max,rpm_min,rpm_max,feed_min,feed_max,plunge_min,plunge_max,note
                   FROM profile_cut WHERE profile=?1 AND tool_id=?2 AND material=?3",
                params![profile, tool_id, material],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?)),
            )
            .optional()?;
        let Some((sd0, sd1, n0, n1, f0, f1, p0, p1, note)) = row else { return Ok(None) };
        let mut text = vec!["valeurs de l'atelier".to_string()];
        text.extend(note);
        text.extend(range_text("broche", n0, n1, "tr/min"));
        text.extend(range_text("avance", f0, f1, "mm/min"));
        text.extend(range_text("plongée", p0, p1, "mm/min"));
        text.extend(range_text("passe", sd0, sd1, "mm"));
        Ok(Some(UserCut {
            rpm: n0.zip(n1).map(|(a, b)| ((a + b) / 2.0).round()),
            feed_mm_min: f0,
            plunge_mm_min: p0,
            stepdown_mm: sd0,
            stepover_mm: None,
            note: Some(text.join(" ; ")),
        }))
    }

    /// Paramètres proposés sur une machine réelle : valeurs perso (enregistrées sous l'id du
    /// profil), puis valeurs de l'atelier, puis calcul avec la classe de machine du profil.
    pub fn suggest_in(&self, tool_id: &str, material: &str, profile_id: &str) -> Result<Option<CutParams>> {
        let profile = self.profile(profile_id)?;
        let flutes = self.tool(tool_id)?.flutes;
        let computed = self.compute_for(tool_id, material, &profile.class)?;
        let base = match self.atelier_cut(profile_id, tool_id, material)? {
            Some(a) => overlay(computed, &a, flutes, Origin::Atelier, Origin::Atelier, "complétées par le calcul"),
            None => computed,
        };
        let Some(u) = self.user_cut(tool_id, material, profile_id)? else { return Ok(base) };
        Ok(overlay(base, &u, flutes, Origin::User, Origin::Mixed, "valeurs perso complétées par l'atelier ou le calcul"))
    }

    /// Accès direct pour les requêtes personnalisées.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }
}

fn row_tool(r: &rusqlite::Row<'_>) -> rusqlite::Result<Tool> {
    Ok(Tool {
        id: r.get(0)?,
        name: r.get(1)?,
        kind: r.get(2)?,
        diameter_mm: r.get(3)?,
        flutes: r.get(4)?,
        angle_deg: r.get(5)?,
        tip_diameter_mm: r.get(6)?,
        flute_length_mm: r.get(7)?,
        shank_mm: r.get(8)?,
    })
}

fn row_machine(r: &rusqlite::Row<'_>) -> rusqlite::Result<Machine> {
    Ok(Machine {
        id: r.get(0)?,
        name: r.get(1)?,
        rpm_min: r.get(2)?,
        rpm_max: r.get(3)?,
        fz_factor: r.get(4)?,
        depth_factor: r.get(5)?,
        max_diameter_mm: r.get(6)?,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn dir() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let n = N.fetch_add(1, Ordering::SeqCst);
        let d = std::env::temp_dir().join(format!("mc-tooldb-test-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    fn db() -> ToolDb {
        ToolDb::open(&dir()).unwrap()
    }

    #[test]
    fn valeurs_perso_prioritaires() {
        let db = db();
        let calc = db.suggest("flat-2f-3.175", "bois_tendre", "cnc3018").unwrap().unwrap();
        assert_eq!(calc.origin, Origin::Computed);
        // seule l'avance est saisie : le reste vient du calcul
        let u = UserCut { feed_mm_min: Some(600.0), note: Some("CTP peuplier, essai OK".into()), ..Default::default() };
        db.save_user_cut("flat-2f-3.175", "bois_tendre", "cnc3018", &u).unwrap();
        let p = db.suggest("flat-2f-3.175", "bois_tendre", "cnc3018").unwrap().unwrap();
        assert_eq!(p.origin, Origin::Mixed);
        assert_eq!(p.feed_mm_min, 600.0);
        assert_eq!(p.rpm, calc.rpm);
        assert!(p.notes.iter().any(|n| n.contains("CTP")));
        // les autres machines ne sont pas touchées
        let autre = db.suggest("flat-2f-3.175", "bois_tendre", "defonceuse").unwrap().unwrap();
        assert_eq!(autre.origin, Origin::Computed);
        db.delete_user_cut("flat-2f-3.175", "bois_tendre", "cnc3018").unwrap();
        assert_eq!(db.suggest("flat-2f-3.175", "bois_tendre", "cnc3018").unwrap().unwrap(), calc);
    }

    #[test]
    fn outil_perso_sans_modele() {
        let db = db();
        let t = Tool { id: "perso-pcb-20".into(), name: "Ma pointe PCB".into(), kind: "engraver".into(),
                       diameter_mm: 3.175, flutes: 1, angle_deg: Some(20.0), tip_diameter_mm: Some(0.1),
                       flute_length_mm: None, shank_mm: Some(3.175) };
        db.save_user_tool(&t).unwrap();
        assert!(db.is_user_tool("perso-pcb-20").unwrap());
        assert!(db.tools().unwrap().iter().any(|x| x.id == "perso-pcb-20"));
        // PCB : pas de référence fabricant -> rien sans valeurs perso
        assert!(db.suggest("perso-pcb-20", "pcb", "cnc3018").unwrap().is_none());
        let u = UserCut { rpm: Some(10000.0), feed_mm_min: Some(150.0), stepdown_mm: Some(0.05), ..Default::default() };
        db.save_user_cut("perso-pcb-20", "pcb", "cnc3018", &u).unwrap();
        let p = db.suggest("perso-pcb-20", "pcb", "cnc3018").unwrap().unwrap();
        assert_eq!(p.origin, Origin::User);
        assert_eq!(p.plunge_mm_min, 50.0);
        db.delete_user_tool("perso-pcb-20").unwrap();
        assert!(db.tool("perso-pcb-20").is_err());
    }

    #[test]
    fn perso_conserve_apres_mise_a_jour_banque() {
        let d = dir();
        {
            let db = ToolDb::open(&d).unwrap();
            let u = UserCut { feed_mm_min: Some(700.0), ..Default::default() };
            db.save_user_cut("flat-2f-6", "mdf", "cnc3018", &u).unwrap();
            // simule une banque périmée
            db.connection().execute("UPDATE main.meta SET value='ancienne' WHERE key='hash'", []).unwrap();
        }
        let db = ToolDb::open(&d).unwrap();
        assert_ne!(hash_of(db.connection()).as_deref(), Some("ancienne"));
        assert_eq!(db.suggest("flat-2f-6", "mdf", "cnc3018").unwrap().unwrap().feed_mm_min, 700.0);
    }

    #[test]
    fn banque_chargee() {
        let db = db();
        assert!(db.tools().unwrap().len() >= 20);
        assert_eq!(db.machines().unwrap().len(), 3);
        assert!(db.materials().unwrap().iter().any(|(id, _)| id == "mdf"));
    }

    #[test]
    fn exemple_sorotec_alu_8mm() {
        // Exemple du document Sorotec : alu corroyé, Ø8, 2 dents -> ~19 900 tr/min, ~2 550 mm/min
        let t = Tool { id: "t".into(), name: "t".into(), kind: "flat".into(), diameter_mm: 8.0, flutes: 2,
                       angle_deg: None, tip_diameter_mm: None, flute_length_mm: None, shank_mm: None };
        let m = Machine { id: "m".into(), name: "m".into(), rpm_min: 0.0, rpm_max: 1e6,
                          fz_factor: 1.0, depth_factor: 1.0, max_diameter_mm: 20.0 };
        let (r, depth) = db().material_ref("alu").unwrap().unwrap();
        let p = compute(&t, &m, &r, depth);
        assert!((p.rpm - 19894.0).abs() < 20.0, "{p:?}");
        assert!((p.feed_mm_min - 2550.0).abs() <= 10.0, "{p:?}");
    }

    #[test]
    fn mdf_6mm_selon_machine() {
        let db = db();
        let pro = db.suggest("flat-2f-6", "mdf", "broche_pro").unwrap().unwrap();
        let p3018 = db.suggest("flat-2f-6", "mdf", "cnc3018").unwrap().unwrap();
        assert!((pro.rpm - 23873.0).abs() < 2.0);
        assert_eq!(p3018.rpm, 10000.0);
        assert!(p3018.feed_mm_min < pro.feed_mm_min / 3.0);
        assert!(!p3018.notes.is_empty());
    }

    #[test]
    fn materiau_sans_reference() {
        assert!(db().suggest("engr-20-0.1", "pcb", "cnc3018").unwrap().is_none());
    }

    const P3018: &str = "genmitsu-3018-pro";

    #[test]
    fn profil_3018() {
        let db = db();
        let p = db.profile(P3018).unwrap();
        assert_eq!(p.class, "cnc3018");
        assert_eq!(p.travel_mm, [300.0, 180.0, 45.0]);
        assert_eq!(p.spindle_rpm_max, 20000.0);
        assert_eq!(p.collets_mm, vec![3.175, 4.0, 6.0]);
        assert_eq!((p.tab_height_mm, p.tab_spacing_mm), (2.0, 50.0));
        assert!(p.models.iter().any(|(r, f)| r == "table" && f == "3018 - Table.sc"));
        assert!(p.fits([0.0, 0.0, 0.0], [300.0, 180.0, 45.0]));
        assert!(!p.fits([0.0, 0.0, 0.0], [301.0, 100.0, 10.0]));
        assert!(!p.fits([-1.0, 0.0, 0.0], [10.0, 10.0, 10.0]));
        assert_eq!(db.profiles().unwrap().len(), 1);
        assert!(db.profile("inconnu").is_err());
    }

    #[test]
    fn outils_atelier_3018_et_pinces() {
        let db = db();
        let t = db.profile_tools(P3018).unwrap();
        assert_eq!(t.iter().map(|t| t.number).collect::<Vec<_>>(), (1..=11).collect::<Vec<_>>());
        // outils 1 à 8 : pince 3,175 ; 9 à 11 : pince 6 mm (règle de la doc atelier)
        assert!(t.iter().filter(|t| t.number <= 8).all(|t| t.collet_mm == 3.175));
        assert!(t.iter().filter(|t| t.number >= 9).all(|t| t.collet_mm == 6.0));
        let n2 = &t[1].tool;
        assert_eq!((n2.kind.as_str(), n2.diameter_mm, n2.flutes, n2.flute_length_mm), ("flat", 3.175, 2, Some(17.0)));
        assert_eq!((t[0].tool.angle_deg, t[0].tool.tip_diameter_mm), (Some(60.0), Some(0.2)));
        assert_eq!(t[2].tool.tip_diameter_mm, Some(0.1));
        assert_eq!(t[10].tool.angle_deg, Some(90.0));
    }

    #[test]
    fn valeurs_atelier_osb() {
        let db = db();
        let p = db.suggest_in("atelier-3018-02", "osb", P3018).unwrap().unwrap();
        assert_eq!(p.origin, Origin::Atelier);
        assert_eq!((p.rpm, p.feed_mm_min, p.plunge_mm_min, p.stepdown_mm), (11000.0, 800.0, 300.0, 2.5));
        assert_eq!(p.stepover_mm, Some(0.79)); // 25 % du Ø, complété par le calcul
        assert!(p.notes.iter().any(|n| n.contains("800-1000")));
        // mêmes valeurs pour les outils 6 et 8, pas pour la Ø1
        assert_eq!(db.suggest_in("atelier-3018-08", "osb", P3018).unwrap().unwrap().origin, Origin::Atelier);
        assert_eq!(db.suggest_in("atelier-3018-04", "osb", P3018).unwrap().unwrap().origin, Origin::Computed);
    }

    #[test]
    fn valeurs_atelier_sans_plongee() {
        // mousse PU : la doc ne donne pas la plongée -> avance / 3
        let p = db().suggest_in("atelier-3018-02", "mousse", P3018).unwrap().unwrap();
        assert_eq!(p.origin, Origin::Atelier);
        assert_eq!((p.rpm, p.feed_mm_min, p.plunge_mm_min, p.stepdown_mm), (10000.0, 1200.0, 400.0, 5.0));
    }

    #[test]
    fn perso_prioritaire_sur_atelier() {
        let db = db();
        let u = UserCut { feed_mm_min: Some(950.0), ..Default::default() };
        db.save_user_cut("atelier-3018-02", "osb", P3018, &u).unwrap();
        let p = db.suggest_in("atelier-3018-02", "osb", P3018).unwrap().unwrap();
        assert_eq!(p.origin, Origin::Mixed);
        assert_eq!((p.feed_mm_min, p.rpm, p.stepdown_mm), (950.0, 11000.0, 2.5));
        // les valeurs perso du profil ne touchent pas la classe de machine
        assert_eq!(db.suggest("atelier-3018-02", "osb", "cnc3018").unwrap().unwrap().origin, Origin::Computed);
    }

    #[test]
    fn fraise_a_lamer_sans_modele() {
        // pas de calcul ni de valeurs d'atelier : à saisir à la main
        assert!(db().suggest_in("atelier-3018-09", "mdf", P3018).unwrap().is_none());
    }

    #[test]
    fn interpolation_fz() {
        let r = MaterialRef { key: "x".into(), vc_m_min: 1.0, fz_table: vec![(1.0, 0.02), (2.0, 0.04)], depth_x_d: 1.0 };
        assert!((r.fz_at(1.5) - 0.03).abs() < 1e-9);
        assert!((r.fz_at(0.5) - 0.01).abs() < 1e-9);
        assert!((r.fz_at(5.0) - 0.04).abs() < 1e-9);
    }
}
