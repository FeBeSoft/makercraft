# Journal des modifications - MakerCraft

Toutes les modifications notables de MakerCraft sont notées ici, de la plus récente à la plus
ancienne. La rubrique « Non publié » se remplit au fil des commits ; à chaque version, elle
devient la note de version (`docs/versions/`).

*All notable changes to MakerCraft are listed here, newest first. Each section ends with a
short English summary.*

Rubriques : **Ajouté**, **Modifié**, **Corrigé**, **Retiré**, **Amont** (projets greffés ou portés).

## Non publié

### Ajouté
- Spécification du format de projet `.mkc` version 2 (`docs/reference/format-mkc.md`) :
  archive zip à parties avec manifeste, versions par partie et empreintes, façon `.docx`.
  Pas encore codée.

### Amont
- SolidCraft v0.41.0 -> v0.51.2 (extension `cao`) : import STEP plus robuste, import IGES,
  mises en plan, assemblages, tôlerie. Aucun changement de notre code, sauf le numéro de version.
- CadCraft reste en v0.20.1 : pas de nouvelle release stable depuis le 11/11/2026.

*English: SolidCraft upgraded to v0.51.2; CadCraft stays on v0.20.1 (no newer stable release).*

## v0.1.0 bêta 20261121 - 21/11/2026

Première version test publique, pour Windows 10 / 11 (64 bits). Notes complètes :
[docs/versions/v0.1.0-beta.20261121.md](docs/versions/v0.1.0-beta.20261121.md).

### Ajouté
- **Application de bureau** à ateliers (egui 0.37) : sélecteur « Ateliers » en haut à gauche,
  ruban, barre d'accès rapide, thème sombre, Options avec le choix du mode maker / prof / élève.
- **Atelier CNC - Fraisage 2,5D** en six étapes (Machine, Brut, Dessin, Parcours, Simulation,
  Usinage) :
  - dessin avec le moteur de CadCraft, import DXF / DWG, un calque = un contour ;
  - parcours contour extérieur ou intérieur, sur le trait, poche, perçage, gravure, vitesses
    proposées par la banque d'outils ;
  - G-code Grbl avec rappel de l'outil et de la pince ER11 avant chaque pause M0, dégagement de
    fin de programme (pièce ou G53) ;
  - projets `.mkc` (dessin CadCraft, modèle SolidCraft et données MakerCraft dans une archive).
- **Atelier Machines** : Fiche (machine, équipements, état du calibrage), Pilotage (positions
  pièce et table, zéro par axe, déplacement manuel, broche, arrêt logiciel, vue 3D / 2D, console,
  envoi d'un programme), rail de gauche, machine, port et connexion dans le ruban ; double
  validation des déplacements longs sans zéro valable.
- **Pilotage Grbl 1.1 / grblHAL** (extension `machine`) : transports série, TCP et émulateur
  intégré ; erreurs et alarmes expliquées en français (cause, quoi faire).
- **Banque d'outils** (extension `tooldb`, exe autonome `makercraft-outils`) : profil Genmitsu
  3018-PRO, 11 outils d'atelier numérotés avec leur pince, valeurs de coupe OSB, MDF,
  contreplaqué, mousse, acrylique.
- **Carbide Create** : lecture des projets `.c2d` (extension `c2d`), appli glisser-déposer
  `c2d_to_nc` et outil en ligne de commande `makercraft-c2d`.
- **Outils en ligne de commande** : `makercraft-cli` (commandes des extensions),
  `makercraft-send` (console et envoi de G-code).
- **À propos** selon la règle FeBe : 7 onglets (Le logiciel, L'Auteur, Droits d'auteur, RGPD,
  Sources, Mises à jour avec les versions des sources, Contact).
- **Dépôt public** : README bilingue, captures, modèles d'issues, Discussions GitHub avec
  formulaires en français et en anglais, page Patreon.

### Amont
- Greffe de **CadCraft v0.20.1** (extension `dao`) et de **SolidCraft v0.41.0** (extension `cao`),
  en dépendances git épinglées, sans modifier leurs sources.
- Portages GPL suivis dans `upstream.toml` : LaserGRBL 7.14.1, Candle, Universal G-Code Sender
  2.1.27 ; projets de référence en lecture seule (bCNC, OpenBuilds CONTROL, COPO, craft-launcher,
  Cut-Craft, cavalier_contours, grblHAL, gSender, svg2gcode).
- Fiche de référence Carbide Create V8 (analyse boîte noire, interopérabilité) et abaque des
  paramètres de coupe dans `docs/reference/`.

### Fondations (18/10 au 13/11/2026)
- Workspace Rust : noyau `geom` et `ext-api` (contrat d'extension, registre de commandes),
  `xtask ci` et `xtask upstream`, suivi des projets amont.

*English: first public test release. Desktop app with workshops: CNC 2.5D milling (CadCraft
drawing engine, DXF/DWG import, profile, pocket, drilling and engraving toolpaths, Grbl G-code,
.mkc projects) and Machines (machine sheet, jog, spindle, software stop, program sending,
built-in emulator, Grbl alarms explained in French). Standalone tool library, Carbide Create
.c2d reader and c2d_to_nc, command-line tools, classroom mode preview (teacher / pupil).*
