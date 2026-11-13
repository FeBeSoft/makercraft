# -*- coding: utf-8 -*-
"""
Recoupement : valeurs des logiciels (banque/imported/*.toml) vs modele fabricant
(references/sorotec.toml, meme calcul que crates/ext/tooldb::compute) et borne haute Amana.
Sortie : banque/imported/_recoupement.md (local, non publie).
Usage : python recouper.py [classe_machine]   (defaut : defonceuse)
"""
import math, statistics, sys, tomllib
from pathlib import Path

B = Path(__file__).resolve().parent.parent
load = lambda p: tomllib.loads((B / p).read_text(encoding="utf-8"))
SORO = load("references/sorotec.toml")
AMANA = load("references/amana.toml")
MACH = {m["id"]: m for m in load("machines.toml")["machine"]}
ROWS = {r["key"]: r for r in SORO["row"]}
DIAS = SORO["diameters_mm"]

def ref_of(mat):
    key = SORO["map"].get(mat, mat)
    return ROWS.get(key)

def fz_at(row, d):
    t = list(zip(DIAS, row["fz"]))
    if d <= t[0][0]:
        return t[0][1] * d / t[0][0]
    if d >= t[-1][0]:
        return t[-1][1]
    for (d0, f0), (d1, f1) in zip(t, t[1:]):
        if d <= d1:
            return f0 + (f1 - f0) * (d - d0) / (d1 - d0)

def model(tool, mat, m):
    row = ref_of(mat)
    d = tool.get("diameter_mm")
    if not row or not d:
        return None
    k = tool["type"]
    if k not in ("flat", "ballnose", "bullnose", "vbit", "engraver", "drill"):
        return None
    d_eff = max(d / 2, tool.get("tip_diameter_mm") or 0) if k == "vbit" else \
            max(tool.get("tip_diameter_mm") or d, 0.2) if k == "engraver" else d
    fz = fz_at(row, d_eff) * m["fz_factor"] * (0.5 if k == "drill" else 1)
    rpm = min(max(row["vc"] * 1000 / (math.pi * d_eff), m["rpm_min"]), m["rpm_max"])
    z = tool.get("flutes") or 2
    return dict(rpm=rpm, feed=rpm * z * fz, fz=fz)

def amana_max(mat, d):
    for r in AMANA["row"]:
        if mat in r["materials"]:
            ds, fm = r["diameters_mm"], r["fz_max"]
            if d <= ds[0]:
                return fm[0] * d / ds[0]
            for (d0, f0), (d1, f1) in zip(zip(ds, fm), zip(ds[1:], fm[1:])):
                if d <= d1:
                    return f0 + (f1 - f0) * (d - d0) / (d1 - d0)
            return fm[-1]
    return None

def main():
    mid = sys.argv[1] if len(sys.argv) > 1 else "defonceuse"
    m = MACH[mid]
    L = [f"# Recoupement logiciels / fabricants (machine : {m['name']})", "",
         "Ratio = valeur du logiciel / valeur du modele Sorotec. 1.0 = identique.", "",
         "| Source | Materiau | Jeux compares | Ratio avance (mediane) | min - max | Charge/dent > Amana |",
         "|---|---|---|---|---|---|"]
    outliers, nomodel = [], 0
    for f in sorted((B / "imported").glob("*.toml")):
        data = tomllib.loads(f.read_text(encoding="utf-8"))
        groups = {}
        for t in data.get("tool", []):
            for c in t.get("cutting", []):
                if not c.get("feed_mm_min") or not c.get("rpm"):
                    continue
                mo = model(t, c["material"], m)
                if not mo:
                    nomodel += 1
                    continue
                ratio = c["feed_mm_min"] / mo["feed"]
                g = groups.setdefault(c["material"], [[], 0])
                g[0].append(ratio)
                am = amana_max(c["material"], t["diameter_mm"])
                if am and c.get("chipload_mm") and c["chipload_mm"] > am:
                    g[1] += 1
                if ratio > 2.5 or ratio < 0.4:
                    outliers.append(f"| {f.stem} | {t['name']} | {c['material']} | {c['rpm']:.0f} / {mo['rpm']:.0f} | "
                                    f"{c['feed_mm_min']:.0f} / {mo['feed']:.0f} | {ratio:.2f} |")
        for mat, (rs, over) in sorted(groups.items()):
            L.append(f"| {f.stem} | {mat} | {len(rs)} | {statistics.median(rs):.2f} | "
                     f"{min(rs):.2f} - {max(rs):.2f} | {over} |")
    L += ["", f"{nomodel} jeux sans modele (materiau sans reference fabricant : PCB, composites, pierre, non precise).",
          "", "## Ecarts importants (ratio < 0.4 ou > 2.5)", "",
          "| Source | Outil | Materiau | tr/min logiciel / modele | avance logiciel / modele | ratio |",
          "|---|---|---|---|---|---|"] + (outliers or ["| - | aucun | | | | |"])
    out = B / "imported" / "_recoupement.md"
    out.write_text("\n".join(L), encoding="utf-8")
    print("\n".join(L[:40]))
    print(f"... {len(outliers)} ecarts -> {out}")

if __name__ == "__main__":
    main()
