# Projets amont : suivi et mises à jour

Clones de référence, en lecture seule, dans `H:\_src\upstream\` (chemins dans `upstream.toml`).
Ne jamais y modifier de fichier.

| Projet | Licence | Usage | Version épinglée |
|---|---|---|---|
| CadCraft | MIT/Apache | dépendance Rust (extension dao) | v0.8.0 |
| SolidCraft | MIT/Apache | dépendance Rust (extension cao) | v0.15.0 |
| LaserGRBL | GPLv3 | portage C# → Rust (machine, cam laser) | v7.14.1 |
| Candle | GPLv3 | portage C++ → Rust (G-code, heightmap) | c04e6f2 |
| UGS | GPLv3 | portage Java → Rust (architecture contrôleur) | v2.1.27 |

## Voir où on en est

```
cargo xtask upstream --fetch
```

Pour chaque projet : version épinglée, dernier tag amont, nombre de commits d'écart, et, pour
chaque fichier listé dans `[[port]]`, s'il a changé depuis la version portée (avec la commande
`git diff` à lancer).

## Cas 1 — CadCraft, SolidCraft (dépendances)

1. Choisir le nouveau tag (de préférence une release, pas `main`).
2. Mettre à jour `tag = "…"` dans `Cargo.toml` **et** `pinned = "…"` dans `upstream.toml`.
3. SolidCraft : comparer ses `[patch.crates-io]` (son `Cargo.toml`) avec les nôtres et les recopier.
4. `cargo update`, puis `cargo xtask ci`. Corriger uniquement nos adaptateurs (`crates/ext/dao`, `crates/ext/cao`).
5. Commit : `amont: cadcraft v0.8.0 → v0.9.0`.

Un correctif dans leur code est indispensable ? Fork GitHub, branche `makercraft-patches`,
`[patch."https://github.com/…"]` vers le fork dans notre `Cargo.toml`, et pull request en amont.
Retirer le patch dès qu'il est accepté.

Cadence conseillée : une fois par mois, ou quand une release apporte un correctif utile.

## Cas 2 — LaserGRBL, Candle, UGS (portages GPL)

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
