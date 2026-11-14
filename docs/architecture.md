# Architecture de MakerCraft

## Couches

| Couche | Crate | Rôle | État |
|---|---|---|---|
| L0 | `core/geom` | vecteurs 2D/3D, boîtes englobantes (unifiés plus tard avec cadcraft/solidcraft-geom) | ✅ base |
| L0 | `core/units` | mm/inch, angles | à faire |
| L1 | `core/doc` | Workspace : plusieurs documents typés, sélection, undo | à faire |
| L1 | `core/ext-api` | `Extension`, `Registry`, `CommandSpec`, `Host` (ordre des dépendances) | ✅ base |
| L2 | `core/scene` | scène neutre 2D/3D (polylignes, maillages) → rendu wgpu | à faire |
| L2 | `core/io` | registre d'importeurs/exporteurs | à faire |
| L3 | `ext/dao` | adaptateur CadCraft 0.20.1 : `Sketch` = moteur de dessin `cadcraft-engine` (commandes à invites, accroches, sélection, annuler), DXF/DWG par `cadcraft-io`, liste d'affichage `cadcraft-render` ; contours par calque | 🟡 dessin éditable |
| L3 | `ext/cao` | adaptateur SolidCraft 0.41.0 : lecture STEP et mesures (`solidcraft-kernel`) ; vue 3D en lecture seule (`view3d`, rendu sur processeur de `solidcraft-viewport`) | 🟡 STEP, vue 3D |
| L3 | `ext/mkc` | projet `.mkc` : archive zip (`makercraft.json` + dessin CadCraft `.dxf`/`.dwg` + modèle SolidCraft `.sc`) ; machine, brut, brides, parcours | ✅ format 1 |
| L3 | `ext/c2d` | lecture des projets Carbide Create `.c2d` (formes, calques, outils, parcours) ; commande `c2d.info` | ✅ |
| L3 | `ext/galaad` | lecture des dessins Galaad 3 `.gal` / `.gli` / `.cub` (géométrie, outils, attributs d'usinage) ; format : `reference/galaad-format-gal.md` | 🟡 lecture |
| L3 | `ext/cam` | FAO 2.5D : découpe, poche, perçage, gravure ; G-code Grbl ASCII ; aperçu SVG | 🟡 v0.1 (laser, attaches, V-carve vrai à faire) |
| L3 | `ext/machine` | transports (série, TCP, émulateur), protocole Grbl 1.1/grblHAL, contrôleur et streaming, codes d'erreur | ✅ v0.1 (aperçu G-code à faire) |
| L4 | `ui/shell` | egui : coquille commune aux applis (prévue quand une 2e appli à ateliers existera) | à faire |
| app | `makercraft` | **application de bureau** : atelier CNC (fraisage 2,5D : Machine > Brut > Dessin > Parcours > Simulation > Usinage), Machines ; Laser, PCB, usinage 3D et gravure à venir ; hôte d'extensions | 🟡 |
| app | `makercraft-cli` | lister / exécuter les commandes | ✅ |
| app | `makercraft-send` | console Grbl et envoi de fichiers G-code | ✅ |
| app | `makercraft-c2d` | projet `.c2d` → G-code Grbl (un fichier par outil) | ✅ |
| app | `c2d_to_nc` | fenêtre egui : glisser un `.c2d`, vérifications, export des `.nc` à côté du projet | ✅ |

Règle : une couche ne dépend que des couches inférieures ; le noyau ne connaît aucune extension.

Ateliers autonomes et intégrés (contrat `Atelier`, format de projet `.makercraft`) : voir `ateliers.md`.

## Contrat d'extension

Une extension implémente `Extension` (`id`, `label`, `depends_on`, `register`). L'hôte charge
un lot d'extensions dans l'ordre de leurs dépendances et refuse : doublons, dépendance absente ou
circulaire, commande hors de son espace de noms (`machine.*` pour l'extension `machine`).

Les extensions métier sont **compilées** avec l'app (features cargo) : pas de DLL, donc pas
d'`unsafe` ni d'ABI instable. Les extensions tierces viendront en WebAssembly, avec la même
surface `run(commande, json)` que celle prévue par SolidCraft (`docs/plugins.md` chez eux).

## Greffe de CadCraft et SolidCraft

Dépendances git épinglées sur leur dernière release stable, compilées telles quelles (détails et
procédure de mise à jour : `UPSTREAM.md`). Seuls des crates **sans interface graphique** sont liés :
`cadcraft-dxf`, `-doc`, `-geom`, `-engine`, `-io`, `-render` dans `ext/dao` ; `solidcraft-kernel`,
`-io`, `-viewport`, `solidcore-record`, `solidcore-tess` dans `ext/cao`. Leurs crates egui ne peuvent
pas cohabiter dans un même exécutable (chacun remplace `egui-wgpu` et `winit` par sa propre
version) : MakerCraft dessine ses vues lui-même (la 2D peint la liste d'affichage de CadCraft, la 3D
affiche l'image du moteur de rendu de SolidCraft).

Deux réglages du workspace, sans toucher à leurs sources : le `Cargo.lock` garde les crates `icu_*` en
2.3 (exigé par `cadcraft-fonts`), et `[patch.crates-io]` reprend la révision de `krilla` que
CadCraft épingle pour `craft-pdf` (Cargo ignore les patchs d'une dépendance).

## Application `makercraft`

```
apps/makercraft/src/
  main.rs        fenêtre (taille 1360 x 860, minimum 1024 x 680), icône, polices
  app.rs         barre d'accès rapide (logo, Fichier, Ouvrir, Enregistrer / Annuler / Rétablir
                 de l'atelier CNC, onglet du document ; à droite Extensions, Aide, état machine, Options),
                 ruban, barre d'état
  ribbon.rs      ruban façon SolidCraft / DraftSight : onglets, groupes nommés (nom en bas,
                 séparateurs), grands boutons libellé dessous, petits empilés par trois,
                 bascules ; repliable (double-clic sur l'onglet actif ou chevron). Chaque atelier
                 fournit ses onglets (`ribbon()`) et exécute les commandes (`command(id)`)
  switcher.rs    sélecteur d'ateliers façon « espace de travail » de SolidCraft (CATIA, SolidWorks) :
                 grand bouton à gauche du ruban, liste des ateliers, ceux à venir grisés
  options.rs     fenêtre Options façon GravéoCraft : catégories à gauche, onglets, une ligne par
                 réglage, Réinitialiser / Annuler / Appliquer ; une page par extension ; À propos
                 en six onglets (Auteur, Remerciements, Droits d'auteur, Sources, Mises à jour,
                 Contact), sur le modèle d'UIdeCraft
  about.rs       textes de l'À propos ; la bio de l'auteur vient de H:\_src\_commun\auteur\bio.txt,
                 copiée par build.rs (repli : assets/bio.txt)
  hote.rs        hôte d'extensions : machine, c2d, cam, dao, cao, mkc ; menu « Extensions » construit
                 depuis les CommandSpec ; fenêtre d'exécution d'une commande (paramètres JSON)
  ateliers/      mod.rs (familles CNC / Laser / PCB / Machines, écran « à venir »),
                 machines.rs (pilotage Grbl), cnc/ :
    mod.rs         atelier CNC : étapes Machine > Brut > Dessin > Parcours > Simulation > Usinage
                   (un onglet de ruban chacune), panneau de l'étape | dessin 2D | vue 3D, projet .mkc
    panels.rs      panneau de gauche de chaque étape (machine, brut et brides, contours, réglages
                   des parcours, vérifications et durée, G-code)
    sketch_view.rs dessin 2D piloté par le moteur de CadCraft : clics et accroches, ligne de
                   commande (noms français acceptés, invites traduites), zoom, déplacement
    view3d.rs      vue 3D en lecture seule : image recalculée seulement quand la scène change,
                   rotation à la souris, zoom
    toolpaths.rs   parcours -> opérations de `ext/cam` (contour, poche, perçage, gravure),
                   lignes 3D, contrôle des zones de bridage
  prefs.rs       préférences JSON (%APPDATA%\FeBe\MakerCraft\preferences.json), dernier atelier
  theme.rs       thème sombre par défaut (esprit GravéoCraft) et clair : bleu = design, orange = composants
  icons.rs       icônes vectorielles dessinées au Painter (pas d'emoji)
  widgets.rs     cartes, boutons, onglets, pastilles
```

- Ateliers disponibles : CNC - Fraisage 2,5D (ouvre `.mkc`, `.dxf`, `.dwg` ; un calque = un contour,
  chaque parcours choisit ses calques et son outil ; G-code Grbl, envoi vers l'atelier Machines) et
  Machines (Grbl par l'émulateur ou un port série). Usinage 3D, Gravure d'images, Laser et PCB
  affichent un écran « à venir » qui décrit ce qu'ils feront, sans bouton factice. Les projets
  `.c2d` restent le domaine de `c2d_to_nc`.
- Les ateliers sont des modules de l'appli en v0.1 ; le contrat `Atelier` et les coquilles
  autonomes de `ateliers.md` viendront quand un deuxième exécutable à ateliers sera fait.
- Sûreté machine : émulateur par défaut, confirmation avant d'envoyer un programme à une vraie
  machine, bouton Arrêt (pause puis reset) toujours visible une fois connecté.

## Échanges entre extensions (à venir)

La FAO ne connaît pas les types internes de CadCraft ou SolidCraft ; elle consomme des
fournisseurs neutres enregistrés par les extensions :

- `Profile2DSource` (dao, cao) : contours fermés, chemins ouverts, calque d'origine ;
- `SolidSource` (cao) : maillage, sections planes (tranches) ;
- `MachineSink` (machine) : capacités de la machine, envoi d'un programme G-code.

## Extension machine

Un thread contrôleur possède le transport ; l'interface envoie des `Command` et lit des `Event`
par canaux, sans jamais bloquer l'envoi. Le `Streamer` (logique pure, testée seule) décide quoi
envoyer : comptage de caractères dans le tampon de la carte (127 octets par défaut, taille réelle
lue par `$I` ou `Bf:`), modes tamponné / synchrone / répétition sur erreur, commandes manuelles
prioritaires sur le programme. Filet de sécurité : `tests/emulator_session.rs` rejoue des sessions
complètes contre l'émulateur (connexion, jog, 2000 lignes sans débordement, erreurs avec numéro
de ligne, alarme, pause/reset, taille de tampon, délai de connexion).

## Feuille de route

1. v0.1 première appli de bureau : coquille à ateliers, DAO/CAO 2D (.c2d, DXF, STEP), FAO fraiseuse
   (.c2d vers .nc), pilotage Grbl (émulateur, série) ; envoyeur : transport série/TCP/émulateur,
   Grbl 1.1/grblHAL, console, jog, streaming.
2. v0.2 laser vectoriel : dessin → G-code, puissance/vitesse par calque, M4, passes, framing.
3. v0.3 raster : image → ligne à ligne, tramage, vectorisation.
4. v0.4 CNC : axe Z, palpage, heightmap, G54–G59.
5. CAO : SolidCraft, tranches → découpe laser, fraisage 2.5D.
