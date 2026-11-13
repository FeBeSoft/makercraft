# -*- coding: utf-8 -*-
"""
MakerCraft - import des banques d'outils existantes vers banque/imported/*.toml
Sources : Vectric (VCarve/Aspire .vtdb), Carbide Create (CSV exporte),
          CNC3D Commander (.tdat), Galaad (.too), CAMotics (.json)
Unites de sortie : mm, mm/min, tr/min, degres.
Usage : python import_sources.py   (lecture seule sur les sources)
"""
import csv, json, math, re, sqlite3, configparser, datetime
from pathlib import Path

HERE = Path(__file__).resolve().parent
OUT = HERE.parent / "imported"
IN2MM = 25.4

SOURCES = {
    "vectric": [
        r"C:\ProgramData\Vectric\Aspire\V12.5\Backups\Migration\2026_11_03_1046\ToolDatabase\tools.vtdb",
        r"C:\ProgramData\Vectric\Aspire\V12.5\ToolDatabase\tools.vtdb",
        r"C:\ProgramData\Vectric\VCarve Pro\V10.5\ToolDatabase\tools.vtdb",
    ],
    "carbide": [str(Path.home() / r"AppData\Local\Carbide 3D\Carbide Create\tools")],
    "cnc3d": [r"D:\CNC3D Commander\CNC3D TurboCut.tdat"],
    "galaad": [r"D:\Galaad\Galaad.too"],
    "camotics": [str(HERE / "CAMotics_tools.json")],
}

# --- normalisation des materiaux -------------------------------------------
MATERIALS = {
    "mdf": ("MDF", ["MDF"]),
    "bois_tendre": ("Bois tendre (resineux)", ["Softwood"]),
    "bois_dur": ("Bois dur (feuillus)", ["Hardwood"]),
    "bois": ("Bois (generique)", ["Timbers", "Wood"]),
    "mousse": ("Mousses (PU, PS, PVC expanse)", ["Foams", "Foam"]),
    "pmma": ("Acrylique / PMMA", ["Acrylic"]),
    "plastique": ("Plastiques (generique)", ["Plastics", "Plastic"]),
    "plastique_tendre": ("Plastique tendre (PE, PP, PVC souple)", ["Soft plastic"]),
    "plastique_dur": ("Plastique dur (PVC rigide, ABS, POM)", ["Hard plastic"]),
    "alu_fonte": ("Aluminium de fonderie", ["Cast aluminium"]),
    "acier": ("Acier", ["Steel"]),
    "alu": ("Aluminium", ["Aluminium", "Aluminum"]),
    "laiton": ("Laiton", ["Brass"]),
    "alliages": ("Alliages non ferreux (generique)", ["Alloys"]),
    "pcb": ("Circuit imprime FR4", ["PCB"]),
    "composite": ("Composites (fibre de verre, carbone)", ["Fibreglass", "Fiberglass", "Carbon Fiber", "Carbon Fibre"]),
    "pierre": ("Pierre / marbre", ["Stone", "Marble"]),
    "generique": ("Non precise", []),
}
ALIAS = {a.lower(): k for k, (_, al) in MATERIALS.items() for a in al}

def mat_ids(text):
    """'Timbers / Foams' -> ['bois','mousse'] ; '' -> ['generique']"""
    if not text:
        return ["generique"]
    ids = [ALIAS[w.strip().lower()] for w in re.split(r"[/&,]|\band\b", text) if w.strip().lower() in ALIAS]
    return ids or ["generique"]

def num(x):
    try:
        v = float(x)
        return None if math.isnan(v) else v
    except (TypeError, ValueError):
        return None

def rd(v, n=4):
    return None if v is None else round(v, n)

def cut(material, machine, rpm, feed, plunge, stepdown, stepover_mm=None, stepover_pct=None, note=None):
    return dict(material=material, machine=machine, rpm=rd(rpm, 0), feed_mm_min=rd(feed, 1),
                plunge_mm_min=rd(plunge, 1), stepdown_mm=rd(stepdown), stepover_mm=rd(stepover_mm),
                stepover_pct=rd(stepover_pct, 1), note=note)

# --- Vectric .vtdb (SQLite) -------------------------------------------------
VECTRIC_TYPES = {0: "ballnose", 1: "flat", 2: "bullnose", 3: "vbit", 4: "engraver",
                 5: "radiused_engraver", 6: "drill", 8: "form", 9: "laser"}
VECTRIC_RATE = {0: 60.0, 1: 1.0, 2: 1000.0, 3: 60 * IN2MM, 4: IN2MM, 5: 12 * IN2MM}  # -> mm/min

def vectric_name(fmt, t, d, ang, units):
    s = fmt or ""
    s = s.replace("{Tool Type}", t).replace("{Units Short}", "mm" if units == 0 else '"')
    s = re.sub(r"\{Diameter\|[^}]*\}", f"{d:g}", s)
    s = re.sub(r"\{Included Angle\|[^}]*\}", f"{(ang or 0):g}", s)
    s = re.sub(r"\{[^}]*\}", "", s)
    return s.replace("\ufffd", "deg").replace("\xb0", "deg").strip()

def import_vectric(paths):
    tools = {}
    for p in paths:
        if not Path(p).exists():
            continue
        c = sqlite3.connect(f"file:{p}?mode=ro", uri=True)
        q = """select g.id,g.name_format,g.tool_type,g.units,g.diameter,g.included_angle,g.flat_diameter,
                      g.num_flutes,g.flute_length,g.tip_radius,m.name,mc.name,
                      d.rate_units,d.length_units,d.feed_rate,d.plunge_rate,d.spindle_speed,d.stepdown,d.stepover
               from tool_geometry g
               left join tool_entity e on e.tool_geometry_id=g.id
               left join tool_cutting_data d on d.id=e.tool_cutting_data_id
               left join material m on m.id=e.material_id
               left join machine mc on mc.id=e.machine_id"""
        for (gid, fmt, ttype, units, dia, ang, flat, fl, flen, tip, mat, mach,
             ru, lu, feed, plunge, rpm, sd, so) in c.execute(q):
            k = VECTRIC_TYPES.get(ttype, f"vectric_{ttype}")
            if k == "laser":
                continue
            dia = dia or 0.0
            g = IN2MM if units == 1 else 1.0
            key = (k, units, dia, ang, flat, fl, tip)
            t = tools.setdefault(key, dict(
                id=f"vectric-{gid[:8]}", source="vectric", source_files=[], name=vectric_name(fmt, k, dia, ang, units),
                type=k, units_origin="inch" if units == 1 else "mm", diameter_mm=rd(dia * g), angle_deg=ang,
                flat_diameter_mm=rd(flat * g) if flat else None, flutes=fl,
                flute_length_mm=rd(flen * g) if flen else None, tip_radius_mm=rd(tip * g) if tip else None,
                cutting=[]))
            if p not in t["source_files"]:
                t["source_files"].append(p)
            if feed is None or mat is None:
                continue
            lg = IN2MM if lu == 1 else 1.0
            fr = VECTRIC_RATE.get(ru, 1.0)
            cd = cut(mat_ids(mat)[0], mach, rpm, feed * fr, (plunge or 0) * fr, (sd or 0) * lg, (so or 0) * lg)
            if dia:
                cd["stepover_pct"] = rd(100 * cd["stepover_mm"] / (dia * g), 1)
            if cd not in t["cutting"]:
                t["cutting"].append(cd)
    return list(tools.values())

# --- Carbide Create (CSV exporte) --------------------------------------------
CARBIDE_TYPES = {"flat": "flat", "ball": "ballnose", "vee": "vbit", "engraver": "engraver",
                 "drill": "drill", "taperedball": "tapered_ballnose", "bullnose": "bullnose"}

def import_carbide(dirs):
    out = []
    for d in dirs:
        for f in sorted(Path(d).glob("*.csv")) if Path(d).is_dir() else []:
            with open(f, newline="", encoding="utf-8-sig", errors="replace") as fh:
                for row in csv.DictReader(fh):
                    g = 1.0 if row.get("metric", "1").strip() == "1" else IN2MM
                    dia = num(row.get("diameter"))
                    t = dict(id=f"carbide-{f.stem[:12]}-{row.get('number')}", source="carbide_create",
                             source_files=[str(f)], name=row.get("name", "").replace("\ufffd", "deg"),
                             type=CARBIDE_TYPES.get(row.get("type", "").lower(), row.get("type")),
                             units_origin="mm" if g == 1 else "inch",
                             vendor=row.get("vendor") or None, model=row.get("model") or None, url=row.get("URL") or None,
                             diameter_mm=rd(dia * g) if dia else None, angle_deg=num(row.get("angle")) or None,
                             flutes=int(num(row.get("numflutes")) or 0) or None,
                             flute_length_mm=rd(num(row.get("flutelength")) * g) if num(row.get("flutelength")) else None,
                             corner_radius_mm=rd(num(row.get("cornerradius")) * g) if num(row.get("cornerradius")) else None,
                             cutting=[])
                    if num(row.get("feedrate")):
                        t["cutting"].append(cut(mat_ids(row.get("material"))[0], row.get("machine") or None,
                                                num(row.get("rpm")), num(row.get("feedrate")) * g,
                                                (num(row.get("plungerate")) or 0) * g, (num(row.get("depth")) or 0) * g,
                                                stepover_pct=num(row.get("3dstepover"))))
                    out.append(t)
    return out

# --- CNC3D Commander .tdat (texte delimite par 0x15..0x19) -------------------
CNC3D_TYPES = {"F": "flat", "B": "ballnose", "BN": "ballnose", "VC": "vbit", "VE": "engraver",
               "TBN": "tapered_ballnose", "BO": "bowl", "D": "drill"}

def import_cnc3d(paths):
    out = []
    for p in paths:
        if not Path(p).exists():
            continue
        txt = Path(p).read_bytes().decode("latin-1")
        group = ""
        for i, rec in enumerate(txt.split("\x19")):
            f = re.split("[\x15-\x18]", rec)
            if len(f) > 3 and f[1] == "1":          # en-tete de groupe
                group = f[2]
                continue
            if len(f) < 16 or f[1] != "2":
                continue
            group, shape, name = f[3], f[4], f[5]
            vals = [x for x in f[6:] if not x.startswith(("http", "PLEASE"))]
            n = [num(x) for x in vals]
            # n: [unite(1=mm), ?, ?, D, stepdown, stepover%, feed, plunge, rpm, extras...]
            dia, sd, so, feed, plunge, rpm = n[3], n[4], n[5], n[6], n[7], n[8]
            ex = n[9:]
            t = dict(id=f"cnc3d-{i:03d}", source="cnc3d_commander", source_files=[p],
                     name=f"{group} - {name}".replace("\xc2", "").replace("\xb0", "deg").replace("\ufffd", "deg"),
                     type=CNC3D_TYPES.get(shape, shape), units_origin="mm", vendor="CNC3D",
                     url=next((x for x in f if x.startswith("http")), None), diameter_mm=dia, cutting=[])
            mf = re.search(r"\b(\d)F\b", group)
            if mf:
                t["flutes"] = int(mf.group(1))
            if shape == "VC" and len(ex) >= 2:
                t["angle_deg"], t["cone_height_mm"] = ex[0], ex[1]
            if shape == "TBN" and len(ex) >= 3:
                t["shank_diameter_mm"], t["half_angle_deg"], t["tip_diameter_mm"], t["flute_length_mm"] = dia, ex[0], ex[1], ex[2]
                t["diameter_mm"] = ex[1]
            mats = ["generique"]
            for src in (group, name):            # materiau dans le groupe, sinon dans le nom
                for par in re.findall(r"\(([^)]*)\)", src):
                    if mat_ids(par) != ["generique"]:
                        mats = mat_ids(par)
                        break
                if mats != ["generique"]:
                    break
            for m in mats:
                t["cutting"].append(cut(m, None, rpm, feed, plunge, sd, stepover_pct=so))
            out.append(t)
    return out

# --- Galaad .too (INI) -------------------------------------------------------
def import_galaad(paths):
    out = []
    for p in paths:
        if not Path(p).exists():
            continue
        cp = configparser.ConfigParser(interpolation=None)
        cp.read(p, encoding="latin-1")
        for s in cp.sections():
            x = cp[s]
            t = dict(id=f"galaad-{x.get('Number', s)}", source="galaad", source_files=[p], name=x.get("Name"),
                     type={"Cylindrical": "flat", "Conical": "vbit", "Spherical": "ballnose"}.get(x.get("Profile"), x.get("Profile")),
                     units_origin="mm", diameter_mm=num(x.get("Diameter")), flutes=int(num(x.get("NumberOfTeeth")) or 0) or None,
                     angle_deg=num(x.get("ConicalAngle")), cutting=[])
            # Galaad : vitesses en mm/s (a confirmer) -> converties en mm/min
            t["cutting"].append(cut("generique", "Galaad", num(x.get("RotationSpeed")),
                                    (num(x.get("AbsPlotSpeed")) or 0) * 60, None, num(x.get("Zmax")),
                                    note="vitesses Galaad supposees en mm/s (x60)"))
            out.append(t)
    return out

# --- CAMotics .json ----------------------------------------------------------
def import_camotics(paths):
    out = []
    for p in paths:
        if not Path(p).exists():
            continue
        for k, v in json.loads(Path(p).read_text(encoding="utf-8")).items():
            g = IN2MM if v.get("units") == "imperial" else 1.0
            out.append(dict(id=f"camotics-{k}", source="camotics", source_files=[p],
                            name=v.get("description") or f"T{k}", units_origin=v.get("units"),
                            type={"cylindrical": "flat", "ballnose": "ballnose", "conical": "vbit"}.get(v.get("shape"), v.get("shape")),
                            diameter_mm=rd(v["diameter"] * g), flute_length_mm=rd(v["length"] * g),
                            angle_deg=v.get("angle"), cutting=[]))
    return out

# --- controles croises -------------------------------------------------------
def check(t):
    w = []
    d = t.get("diameter_mm")
    if t["type"] == "vbit" and t.get("angle_deg") and t.get("cone_height_mm") and d:
        ang = 2 * math.degrees(math.atan((d / 2) / t["cone_height_mm"]))
        if abs(ang - t["angle_deg"]) > 2:
            w.append(f"angle annonce {t['angle_deg']} deg mais D/hauteur du cone donnent {ang:.0f} deg")
    for c in t["cutting"]:
        fl = t.get("flutes")
        if fl and c["feed_mm_min"] and c["rpm"]:
            cl = c["feed_mm_min"] / (c["rpm"] * fl)
            c["chipload_mm"] = round(cl, 4)
            if cl < 0.005 or cl > 0.3:
                w.append(f"[{c['material']}] charge/dent {cl:.4f} mm hors plage 0.005-0.3")
        if d and c["stepdown_mm"] and c["stepdown_mm"] > 1.5 * d and t["type"] in ("flat", "ballnose"):
            w.append(f"[{c['material']}] passe {c['stepdown_mm']} mm > 1.5 x D")
        if d and c.get("stepover_mm") and c["stepover_mm"] > d:
            w.append(f"[{c['material']}] pas lateral {c['stepover_mm']} mm > D")
    return w

# --- sortie TOML -------------------------------------------------------------
def tv(v):
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, (int, float)):
        return repr(int(v)) if float(v).is_integer() and abs(v) < 1e15 else repr(float(v))
    if isinstance(v, list):
        return "[" + ", ".join(tv(x) for x in v) + "]"
    return json.dumps(str(v), ensure_ascii=False)

def write_toml(path, tools, header):
    L = [f"# {header}", f"# genere le {datetime.datetime.now():%Y-%m-%d %H:%M} par _import/import_sources.py", ""]
    for t in tools:
        L.append("[[tool]]")
        for k, v in t.items():
            if k in ("cutting", "warnings") or v is None:
                continue
            L.append(f"{k} = {tv(v)}")
        if t.get("warnings"):
            L.append(f"warnings = {tv(t['warnings'])}")
        for c in t["cutting"]:
            L.append("  [[tool.cutting]]")
            L += [f"  {k} = {tv(v)}" for k, v in c.items() if v is not None]
        L.append("")
    path.write_text("\n".join(L), encoding="utf-8")

def main():
    OUT.mkdir(exist_ok=True)
    runs = [("vectric", import_vectric), ("carbide", import_carbide), ("cnc3d", import_cnc3d),
            ("galaad", import_galaad), ("camotics", import_camotics)]
    rep = ["# Rapport d'import", "", "| Source | Outils | Jeux de coupe | Alertes |", "|---|---|---|---|"]
    details = []
    for name, fn in runs:
        tools = fn(SOURCES[name])
        nw = 0
        for t in tools:
            w = check(t)
            if w:
                t["warnings"] = w
                nw += len(w)
                details.append(f"- **{t['name']}** ({name}) : " + " ; ".join(w))
        write_toml(OUT / f"{name}.toml", tools, f"Outils importes depuis {name}")
        rep.append(f"| {name} | {len(tools)} | {sum(len(t['cutting']) for t in tools)} | {nw} |")
    L = ["# Materiaux normalises (id -> libelle, alias des sources)", ""]
    for k, (lib, al) in MATERIALS.items():
        L += ["[[material]]", f"id = {tv(k)}", f"name = {tv(lib)}", f"aliases = {tv(al)}", ""]
    (HERE.parent / "materials.toml").write_text("\n".join(L), encoding="utf-8")
    rep += ["", "## Alertes (controles croises)", ""] + (details or ["Aucune."])
    (OUT / "_rapport.md").write_text("\n".join(rep), encoding="utf-8")
    print("\n".join(rep[:9 + 1]))
    print(f"{len(details)} outils avec alertes -> imported/_rapport.md")

if __name__ == "__main__":
    main()
