# MakerCraft

Moteur générique 2D/3D en Rust, à extensions : DAO (dessin 2D), CAO (modélisation 3D),
FAO et pilotage de machines à commande numérique (laser, fraiseuse…). Pensé pour le fablab
et l'enseignement de la Technologie.

> Projet indépendant, **non affilié à ArtCraft / Storyteller** ni aux auteurs de SolidCraft.
> Il réutilise CadCraft et SolidCraft (MIT/Apache) et porte du code de LaserGRBL, Candle et
> Universal G-Code Sender (GPLv3).

## Licence

GPL-3.0-or-later (voir `LICENSE`). Les composants MIT/Apache intégrés gardent leur licence.

## Organisation

```
crates/core/   noyau sans métier : geom, ext-api (contrat d'extension, commandes, hôte)
crates/ext/    extensions : machine (dao, cao, cam à venir)
apps/          makercraft-cli (l'app graphique viendra avec l'extension dao)
xtask/         maintenance : cargo xtask ci | cargo xtask upstream [--fetch]
upstream.toml  projets amont suivis et fichiers portés
```

Détails : `docs/architecture.md`. Mises à jour amont : `UPSTREAM.md`.

## Démarrer

Prérequis : Rust ≥ 1.95 (MSVC sous Windows), Git.

```
cargo build
cargo test
cargo run -p makercraft-cli -- list
cargo run -p makercraft-cli -- run machine.parse_state "{\"state\":\"Hold:0\"}"
cargo xtask ci          # fmt + clippy + tests, avant chaque commit
cargo xtask upstream    # état des projets amont (--fetch pour rafraîchir)
```

## Règles

- **Ne jamais planter** : pas de `unwrap`/`expect`/`panic!` hors tests, pas d'`unsafe`, erreurs en `Result`.
- **Tout est une commande** `extension.action`, appelée en JSON : menus, CLI, scripts, MCP.
- **Le noyau ne dépend d'aucune extension** ; une extension ne dépend que du noyau et des
  extensions qu'elle déclare dans `depends_on`.
- **On ne modifie jamais le code amont** : adaptateurs dans nos crates, correctifs via fork + `[patch]`.
