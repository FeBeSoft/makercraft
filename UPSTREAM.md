# Projets amont : suivi et mises à jour

Clones de référence, en lecture seule, dans `H:\_src\upstream\` (chemins dans `upstream.toml`).
Ne jamais y modifier de fichier.

| Projet | Licence | Usage | Version épinglée |
|---|---|---|---|
| CadCraft | MIT/Apache | dépendance Rust : `cadcraft-dxf` (extension dao) | v0.20.1 |
| SolidCraft | MIT/Apache | dépendance Rust : `solidcraft-kernel` (extension cao) | v0.51.2 |
| LaserGRBL | GPLv3 | portage C# → Rust (machine, cam laser) | v7.14.1 |
| Candle | GPLv3 | portage C++ → Rust (G-code, heightmap) | c04e6f2 |
| UGS | GPLv3 | portage Java → Rust (architecture contrôleur) | v2.1.27 |

## Greffe de CadCraft et SolidCraft (sans modifier leurs sources)

- **Version** : la dernière release stable publiée sur GitHub, comme le fait `craft-launcher`
  (`GET /repos/<repo>/releases/latest`, ni pré-version ni nightly). Le lanceur ne connaît pas
  MakerCraft : la « validation avec MakerCraft » est notre `cargo xtask ci` vert sur ce tag.
- **Mode** : dépendances git épinglées sur le tag, compilées telles quelles. Ni copie, ni fork,
  ni `[patch]` : on ne touche pas à leurs sources.
- **Crates liés** : uniquement des crates **sans interface graphique**, `cadcraft-dxf` (lecture DXF)
  et `solidcraft-kernel` (noyau B-rep, STEP, mesures). Les crates egui des deux projets ne
  peuvent pas cohabiter dans un même exécutable : CadCraft remplace toute la famille egui et
  winit par des forks storytold, SolidCraft remplace `egui-wgpu` et `winit` par ses propres
  copies (`vendor/`). MakerCraft dessine donc ses propres vues avec egui 0.37 standard.
- **Adaptateurs** : `crates/ext/dao` et `crates/ext/cao` traduisent leurs types en types
  MakerCraft ; le reste de MakerCraft ne voit jamais un type CadCraft ou SolidCraft.
- **Coût** : la première compilation du noyau SolidCraft est longue (environ 20 min en
  `opt-level = 2`), ensuite elle reste en cache. Le binaire peut être construit sans lui :
  `cargo build -p makercraft --no-default-features --features dao`.

## Voir où on en est

```
cargo xtask upstream --fetch
```

Pour chaque projet : version épinglée, dernier tag amont, nombre de commits d'écart, et, pour
chaque fichier listé dans `[[port]]`, s'il a changé depuis la version portée (avec la commande
`git diff` à lancer).

## Cas 1 - CadCraft, SolidCraft (dépendances)

1. Choisir le nouveau tag (de préférence une release, pas `main`).
2. Mettre à jour `tag = "…"` dans `Cargo.toml` **et** `pinned = "…"` dans `upstream.toml`.
3. Vérifier que les crates liés restent sans interface graphique : `cargo tree -p makercraft-dao -i egui`
   et `cargo tree -p makercraft-cao -i egui` ne doivent rien trouver.
4. `cargo update`, puis `cargo xtask ci`. Corriger uniquement nos adaptateurs (`crates/ext/dao`, `crates/ext/cao`).
5. Commit : `amont: cadcraft v0.20.1 -> v0.21.0`.

Un correctif dans leur code est indispensable ? Fork GitHub, branche `makercraft-patches`,
`[patch."https://github.com/…"]` vers le fork dans notre `Cargo.toml`, et pull request en amont.
Retirer le patch dès qu'il est accepté.

Cadence conseillée : une fois par mois, ou quand une release apporte un correctif utile.

## Cas 2 - LaserGRBL, Candle, UGS (portages GPL)

Le code est traduit, il ne peut pas se mettre à jour automatiquement. La traçabilité passe par
`upstream.toml` : chaque `[[port]]` relie **notre fichier** au **fichier source** porté.

Quand `cargo xtask upstream` signale un fichier modifié (⚠) :

1. Lire le diff proposé (`git -C … diff <épingle>..origin/HEAD -- <fichier>`).
2. Transposer en Rust ce qui nous concerne (correctifs de protocole, nouveaux firmwares, codes d'erreur).
3. Ajouter un test qui couvre le changement.
4. Une fois tous les fichiers d'un projet à jour, avancer `pinned` dans `upstream.toml`.

Règles de portage :

- Porter **un fichier ou un concept à la fois**, jamais un projet entier d'un bloc.
- En tête de chaque fichier porté : `// Porté de <projet> <version> : <chemin source>`.
- Passer la ligne `[[port]]` de `prévu` à `porté` dans le même commit.
- Les **données** (codes d'erreur/alarme, descriptions des réglages `$`, CSV de LaserGRBL) sont
  copiées comme fichiers de données dans `crates/ext/machine/data/`, pas réécrites en code.

## Filet de sécurité

Aucune mise à jour n'entre sans `cargo xtask ci` vert. Dès que l'émulateur Grbl existera, le CI
lancera aussi une session complète contre lui (connexion, jog, envoi d'un fichier).
