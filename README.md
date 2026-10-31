# MakerCraft

![MakerCraft](assets/branding/makercraft-about.jpg)

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
apps/          makercraft-send (console + envoi de G-code), makercraft-cli (commandes des extensions)
               l'app graphique viendra avec l'extension dao
assets/        branding : icône (.ico, .png) et illustration « à propos »
xtask/         maintenance : cargo xtask ci | cargo xtask upstream [--fetch]
upstream.toml  projets amont suivis et fichiers portés
```

## Piloter une machine (extension `machine`)

Grbl 1.1 et grblHAL, par port série USB, TCP (ESP32, grblHAL réseau) ou émulateur intégré.

```
cargo run -p makercraft-send -- ports                       # ports série disponibles
cargo run -p makercraft-send -- --port COM3                 # console interactive
cargo run -p makercraft-send -- --port COM3 dessin.nc       # envoi d'un fichier
cargo run -p makercraft-send -- --emulator dessin.nc        # sans machine
```

Console : G-code ou commandes `$`, plus `?` (état), `!` (pause), `~` (reprise), `reset`, `home`,
`unlock`, `jogstop`, `quit`. **Ctrl-C = arrêt d'urgence** (pause puis reset de la carte : le laser
s'arrête). Options : `--sync` (une ligne à la fois), `--no-reset`, `--pause-on-error`.

Sûreté : arrêt de l'envoi sur alarme, détection d'une machine muette, d'un redémarrage
inattendu de la carte et des « ok » perdus ; jamais de dépassement du tampon de la carte.

Détails : `docs/architecture.md`. Mises à jour amont : `UPSTREAM.md`.

## Démarrer

Prérequis : Rust ≥ 1.95 (MSVC sous Windows), Git.

```
cargo build
cargo test
cargo run -p makercraft-cli -- list
cargo run -p makercraft-cli -- run machine.parse "{\"line\":\"<Hold:0|MPos:1,2,3>\"}"
cargo run -p makercraft-cli -- run machine.error "{\"code\":22}"
cargo xtask ci          # fmt + clippy + tests, avant chaque commit
cargo xtask upstream    # état des projets amont (--fetch pour rafraîchir)
```

## Règles

- **Ne jamais planter** : pas de `unwrap`/`expect`/`panic!` hors tests, pas d'`unsafe`, erreurs en `Result`.
- **Tout est une commande** `extension.action`, appelée en JSON : menus, CLI, scripts, MCP.
- **Le noyau ne dépend d'aucune extension** ; une extension ne dépend que du noyau et des
  extensions qu'elle déclare dans `depends_on`.
- **On ne modifie jamais le code amont** : adaptateurs dans nos crates, correctifs via fork + `[patch]`.
