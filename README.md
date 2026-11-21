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
crates/ext/    extensions : machine, c2d, cam, dao (CadCraft), cao (SolidCraft), mkc (projet), galaad, tooldb
apps/          makercraft (application de bureau), c2d_to_nc, makercraft-send (console + envoi
               de G-code), makercraft-cli (commandes des extensions), makercraft-c2d, makercraft-outils
assets/        branding : icône (.ico, .png) et illustration « à propos »
xtask/         maintenance : cargo xtask ci | cargo xtask upstream [--fetch]
upstream.toml  projets amont suivis et fichiers portés
```

## Application MakerCraft (v0.1)

```
cargo run -p makercraft                       # ou : makercraft.exe "projet.mkc" (ou un .dxf / .dwg)
cargo build --release -p makercraft           # -> target\release\makercraft.exe
```

Le bouton **Atelier**, en haut à gauche, déplie la liste des ateliers (comme l'espace de travail
de SolidCraft, CATIA ou SolidWorks) ; ceux qui ne sont pas encore disponibles sont grisés.
MakerCraft rouvre le dernier atelier utilisé.

| Atelier | Contenu de la v0.1 |
|---|---|
| CNC - Fraisage 2,5D | onglets dans l'ordre de fabrication : **Machine** (machine de la banque d'outils, courses), **Brut** (dimensions, matière, zéro pièce, zones de bridage), **Dessin** (moteur de CadCraft : ligne, polyligne, rectangle, cercle, arc, modifier, accroches, annuler ; import DXF/DWG ; un calque = un contour), **Parcours** (contour extérieur/intérieur, sur le trait, poche, perçage, gravure ; l'outil et ses vitesses se choisissent dans chaque parcours), **Simulation** (passages dans les brides, profondeur, durée), **Usinage** (G-code Grbl, envoi vers l'atelier Machines). Fenêtre : panneau de l'étape, dessin 2D à gauche, vue 3D en lecture seule à droite |
| CNC - Usinage 3D, Gravure d'images | à venir |
| Laser, PCB | à venir |
| Machines | Grbl 1.1 / grblHAL par l'émulateur (port choisi par défaut) ou un port série. Onglets **Fiche** (machine de la banque, mécanique, broche, pinces, équipements installés à cocher, outils de l'atelier, corrections de calibrage), **Pilotage** (cartes d'axes avec état de référence, zéro par axe, déplacement manuel au clavier ou à la souris, broche, arrêt logiciel, vue 3D / 2D, commandes Grbl, programme `.nc`, console), **Calibrage** (zéro table manuel ; assistants à venir), **Réglages Grbl** et **Entretien et journal** (à venir). Sans zéro valable, les déplacements longs sur une vraie machine demandent une ou deux validations. Mode élève : fiche en lecture seule, onglets de réglage masqués, broche, déverrouillage et console réservés |

Projet CNC : fichier `.mkc`, une archive qui contient le dessin au format natif de CadCraft (`dessin.dxf` ou `dessin.dwg`), le modèle SolidCraft (`modele.sc`) et `makercraft.json` (machine, brut, brides, parcours). Les deux fichiers natifs s'extraient tels quels.

Interface : une barre d'accès rapide (Fichier, Ouvrir, document ouvert, état de la machine,
Options) puis un **ruban** dont les onglets et groupes changent avec l'atelier, choisi par le grand
bouton **Atelier** à gauche du ruban. Le ruban se replie par double-clic sur l'onglet actif.

Menu **Extensions** : toutes les commandes des extensions chargées, exécutables avec leurs
paramètres JSON. Bouton **Options** (comme dans GravéoCraft) : thème (sombre par défaut ou clair),
unités, démarrage, dossiers et fichiers récents (`%APPDATA%\FeBe\MakerCraft\preferences.json`),
une page par extension avec ses commandes, et l'À propos (auteur, remerciements, droits d'auteur,
sources, mises à jour, contact).

Sûreté : avant d'envoyer un programme à une vraie machine, MakerCraft demande confirmation ;
**Arrêt** fait une pause puis un reset de la carte.

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
inattendu de la carte et des « ok » perdus ; jamais de dépassement du tampon de la carte ;
aucun caractère non ASCII envoyé (Grbl les prendrait pour des commandes temps réel).
Erreurs et alarmes affichées en français avec la cause probable et quoi faire ; à une pause
`M0` du programme, le commentaire qui la précède (outil et pince ER11 à monter) est affiché.

G-code écrit : avant chaque `M0` de changement d'outil, rappel de l'outil et de sa pince ER11
(lue dans le profil de l'atelier, sinon diamètre de queue) ; en fin de programme, montée en Z
de sécurité puis dégagement en X0, Y = profondeur du brut + 10 mm (ou position `G53` si la
machine fait sa prise d'origine).

## Projets Carbide Create (`.c2d`) vers Grbl

**Application `c2d_to_nc`** (fenêtre) : glisser un `.c2d` dans la fenêtre ou sur l'icône de
l'exe, lire les vérifications (✔ / ⚠ / ✖), cliquer **Exporter les .nc** : les fichiers sont
écrits à côté du projet. Options : un seul fichier avec pauses M0, aperçu SVG.

```
cargo build --release -p c2d_to_nc      # → target\release\c2d_to_nc.exe
```

En ligne de commande :

```
makercraft-c2d info  "projet.c2d"                 # parcours, outils, durée estimée, avertissements
makercraft-c2d gcode "projet.c2d" [-o dossier]    # un .nc par outil + aperçu SVG
makercraft-c2d gcode "projet.c2d" --un-fichier    # un seul .nc, pause M0 aux changements d'outil
makercraft-c2d gcode "projet.c2d" --seul cutout   # seulement certains parcours
```

Le G-code que Carbide Create range dans le `.c2d` est chiffré : il n'est pas utilisé.
MakerCraft relit le dessin, les outils et les réglages des parcours, et recalcule les
trajectoires : découpe (extérieur/intérieur, passes), poche (anneaux concentriques,
surépaisseur), perçage (débourrage), V-carve **approché** par une gravure sur le trait.
Pas encore : attaches, rampes, V-carve à profondeur variable, parcours 3D, arcs G2/G3.
Zéro : X0 Y0 coin avant gauche, Z0 dessus du brut - à refaire à chaque changement d'outil.

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
