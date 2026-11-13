# Banque d'outils coupants MakerCraft

Banque d'outils (fraises, V-bits, pointes à graver, forets) et calcul des paramètres de coupe.
Les fichiers TOML sont compilés en SQLite par `crates/ext/tooldb/build.rs` et intégrés à l'exe.

## Principe

Les paramètres de coupe ne sont **pas stockés** : ils sont **calculés** à partir de
valeurs fabricant (Sorotec : vitesse de coupe vc et charge par dent fz selon le Ø),
du matériau et de la classe de machine (plage de broche, rigidité).

```
n  = vc x 1000 / (pi x D)       bridé à la plage de broche de la machine
fz = fz_Sorotec(D) x fz_factor   (rigidité de la machine)
vf = n x z x fz                 plongée = vf / 3
passe = depth_x_d x D x depth_factor (limitée à la longueur utile)
pas latéral = 25 % D (fraise plate), 10 % D (hémisphérique)
```

Ø engagé : D pour une fraise, D/2 pour un V-bit, la pointe pour une pointe à graver.
Pas de modèle (à renseigner à la main) pour : fraises de forme, à fileter, coniques
hémisphériques, et les matériaux PCB, composites, pierre.

## Arborescence

| Chemin | Rôle | Publié |
|---|---|---|
| `materials.toml` | Matériaux normalisés (id, libellé, alias des logiciels) | oui |
| `machines.toml` | Classes de machine : `cnc3018`, `defonceuse`, `broche_pro` | oui |
| `references/sorotec.toml` | Modèle de calcul (vc, fz par Ø, profondeur conseillée) | oui |
| `references/amana.toml` | Borne haute industrielle, pour le recoupement | oui |
| `tools/*.toml` | Catalogue curé (géométries seules) | oui |
| `_import/import_sources.py` | Import des banques des logiciels installés (lecture seule) | oui |
| `_import/recouper.py` | Compare logiciels et modèle fabricant | oui |
| `imported/` | Résultats bruts, rapports `_rapport.md` et `_recoupement.md` | **non** |

`imported/` reste local : données de logiciels tiers servant de point de comparaison.

## Utilisation en Rust

```rust
use makercraft_tooldb::ToolDb;
let db = ToolDb::open(&dossier_utilisateur)?;          // copie la banque intégrée si besoin
if let Some(p) = db.suggest("flat-2f-6", "mdf", "cnc3018")? {
    println!("{} tr/min, {} mm/min, passe {} mm", p.rpm, p.feed_mm_min, p.stepdown_mm);
    for n in &p.notes { println!("  {n}"); }           // ex. broche bridée
}
```

## Valeurs personnelles (prioritaires)

Le dossier utilisateur contient deux bases :

| Fichier | Contenu | Mise à jour de MakerCraft |
|---|---|---|
| `tools.db` | Banque intégrée (catalogue, matériaux, machines, références) | remplacé automatiquement |
| `user_tools.db` | Outils personnels + valeurs éprouvées | **jamais touché** |

Ordre de priorité dans `suggest()` : valeurs perso (outil + matériau + machine), puis calcul.
Un champ non saisi est complété par le calcul (`Origin::Mixed`). Sans modèle (PCB, fraise de forme…),
seules les valeurs perso sont proposées, à condition d'avoir au moins tr/min et avance.

```rust
use makercraft_tooldb::UserCut;
db.save_user_cut("flat-2f-3.175", "bois_tendre", "cnc3018",
    &UserCut { feed_mm_min: Some(600.0), note: Some("CTP peuplier, essai OK".into()), ..Default::default() })?;
db.save_user_tool(&mon_outil)?;                     // outil absent du catalogue
db.compute_for(outil, matiere, machine)?;           // calcul seul, pour comparer
```

## Ajouter un outil au catalogue

Ajouter un bloc `[[tool]]` dans `tools/generique.toml` (ou un nouveau fichier `tools/<fabricant>.toml`) :

```toml
[[tool]]
id = "flat-2f-3"
name = "Fraise 2 dents Ø3"
type = "flat"            # flat, ballnose, bullnose, vbit, engraver, drill...
diameter_mm = 3.0
flutes = 2
flute_length_mm = 12
shank_mm = 3
```

## Sources des logiciels importés

| Source | Fichier | Format |
|---|---|---|
| Vectric VCarve Pro 10.5 / Aspire 12.5 | `C:\ProgramData\Vectric\...\ToolDatabase\tools.vtdb` | SQLite |
| Carbide Create V8 | export CSV (`%LOCALAPPDATA%\Carbide 3D\Carbide Create\tools\*.csv`) | CSV (`ccpro.db` chiffré, non lu) |
| CNC3D Commander | `CNC3D TurboCut.tdat` | texte délimité 0x15-0x19 |
| Galaad | `Galaad.too` | INI (vitesses supposées en mm/s) |
| CAMotics | `_import/CAMotics_tools.json` | JSON |

Relancer : `python banque/_import/import_sources.py` puis `python banque/_import/recouper.py [cnc3018|defonceuse|broche_pro]`

## Références fabricants

- Sorotec, *Cutting parameters - Calculation and practical hints* v1.0.1 :
  https://webseite.sorotec.de/download/fraesparameter/schnittwerte_en.pdf
- Amana Tool, *Solid Carbide Spektra Coated Spiral Plunge Speed and Feed Chart*
