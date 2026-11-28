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
- Dessin : ruban à groupes façon SolidCraft. La flèche à droite du titre d'un groupe déplie ses
  autres fonctions en mini-ruban ; flèches de défilement aux extrémités quand le ruban déborde.
  Nouvelles commandes : ellipse, polygone, spline, échelle, couper, prolonger, joindre,
  décomposer, chanfrein, réseau (Étoile, Oblong, Grouper, Booléens, Aligner, Répartir grisés).
- Dessin : invites de CadCraft en français (catalogue fr de CadCraft 0.20.1) et noms de
  commandes français dans la ligne de commande.
- Dessin : sélection façon Carbide Create. Rectangle glissé (vers la droite : objets
  entièrement dedans ; vers la gauche : objets touchés), cadre orange à huit poignées et poignée
  de rotation : glisser déplace, met à l'échelle et tourne ; Maj : depuis le centre, rotation
  par pas de 15°. Entrée ouvre la boîte des cotes, à la manière de Galaad.
- Dessin : échelle uniforme ou non, au choix (bouton « Proportions liées » des Propriétés, Ctrl
  l'inverse sur les poignées). Étiré, un cercle devient une ellipse, un arc un arc d'ellipse et
  les arcs des polylignes des segments fins (0,01 mm) ; une seule étape d'annulation.
- Dessin : colonne de droite à un panneau à la fois, onglets en bas : Propriétés (X, Y,
  largeur, hauteur, rayon, rotation, point de référence 3 x 3), 3D, Commentaires (par contour,
  enregistrés dans le `.mkc`).
- Dessin : arbre des contours avec œil (visibilité) et cadenas (verrouillage : plus de
  modification) par contour, barre d'outils (contour courant seul, masquer les autres, tout
  afficher, tout déverrouiller, déplier / replier, nouveau contour), tracés de chaque contour,
  icône « lien » quand des parcours utilisent le contour.
- Dessin : clic droit, menu de la commande en cours ou de la sélection ; clic molette, menu des
  vues (tout voir, cadrer la sélection, zoom).
- Options > Général > Dessin : commandes de CadCraft dans la ligne de commande, origine dans
  la vue 2D, origine dans la vue 3D.
- Barre de titre : numéro de version après MakerCraft, bouton « Mettre à jour » à gauche de
  Machine. La vérification se fait au clic (GitHub, via curl), ou au démarrage si l'option « Vérifier les mises à jour au démarrage » est cochée (non par défaut) ; la fenêtre propose
  Valider (page de téléchargement), Passer cette version, Annuler.
- Fermeture ou ouverture d'un autre projet avec des modifications non enregistrées : fenêtre
  « Enregistrer les modifications ? » (Enregistrer, Ne pas enregistrer, Annuler).
- Barre de titre de Windows : nom du fichier, puis MakerCraft et sa version (aussi dans la barre des tâches) ; logo et nom retirés de la barre d'accès rapide. Aide tout à droite. État de la machine : pastille verte (connectée) ou rouge après l'onglet « Machine » du ruban CNC, un clic ouvre l'atelier Machines, et rappel dans la barre d'état avant les unités (fin du doublon du nom de fichier) ; menu « Extensions » caché par défaut (Options > Général >
  Interface).
- Dessin : variantes au clic droit sur une icône du ruban (cercle : 6, arc : 11, ellipse : 3,
  réseau : rectangulaire, circulaire, le long d'un tracé) ; la dernière utilisée reste affichée et
  se relance d'un clic, retenue d'une séance à l'autre. Chaque variante a son icône (points et éléments qui définissent la forme) ; petit coin sur les boutons à variantes.
- Dessin : réseaux toujours associatifs (lien gardé avec la forme d'origine, modifiables après
  coup, y compris après enregistrement), posés sur le contour de la forme ; décomposer un réseau
  demande confirmation. Réseau rectangulaire créé d'un clic (2 x 2) puis réglé dans Propriétés :
  colonnes, rangées, pas ; réseau circulaire : nombre, angle, rotation des copies ; « Modifier la
  forme d'origine » puis « Terminer » met à jour tous les exemplaires. Le réseau le long d'un
  tracé n'est pas proposé : CadCraft 0.20.1 n'en fait que des copies sans lien. Un réseau choisi seul
  ne montre que ses paramètres.
- Dessin : Étoile (branches, rayons, rotation) et Oblong (longueur hors tout ou entraxe,
  largeur), posés en polylignes fermées ; Point et Points multiples (repères de perçage) ;
  Miroir avec variantes Retourner horizontalement / verticalement (sur place) ; Caler (deux
  paires de points), Étirer, Remplir le brut (mise à l'échelle sans déformer, centrée).
- Contours : renommer sur place (double-clic, F2 ou clic droit ; parcours et commentaires suivent),
  sélectionner ses objets, supprimer un contour et ses objets après confirmation (annulable).
- Contours : couleurs jamais répétées (14 couleurs distinctes, puis teintes réparties).
- Dessin : barre des aides au tracé en bas à gauche de la vue : Accroches (F3, types au clic
  droit), Grille magnétique (F9, pas au clic droit), Ortho (F8), Polaire (F10, pas d'angle au clic
  droit).
- Dessin : édition des nœuds par double-clic sur un objet : ses poignées (sommets, milieux,
  centre) se glissent, avec accroches et aides au tracé ; Échap ou clic dans le vide pour finir.
- Dessin : Polyligne (fermer, ouvrir, joindre...), Diviser, Jalonner, Allonger, Inverser le sens,
  Raccord lisse, Doublons ; groupe Aides : lignes de construction et cotes sur leurs contours
  Construction et Cotes (jamais usinés, contour courant rétabli ensuite), Mesurer (distance,
  aire) avec le résultat sous la vue.
- Dessin : calculs dans les valeurs saisies (W/2+5, @(10+5)*2,0 ; W et H : largeur et hauteur
  du brut) ; accroches sur les coins, milieux des bords et centre du brut.
- Dessin : clic droit sur la sélection > Envoyer dans le contour (changer les objets de contour).
- Dessin : menu d'un sommet en édition des nœuds (supprimer, segment en arc ou en droite,
  ajouter un sommet au milieu) ; longueur et angle affichés près du curseur ; Contraintes
  (automatiques, géométriques, cotes pilotantes) ; Réparer (fermer les tracés presque fermés,
  joindre) ; Dégagements dogbone et T-bone au rayon de la fraise.
- Ruban : le petit ruban des autres fonctions d'un groupe reste dans la fenêtre et défile avec
  des chevrons quand il est trop large.
- Dessin : Historique de construction dans la colonne de droite (clic sur une étape : revenir
  juste après ; caché à la prochaine action) ; points de construction (contour Construction) ;
  croix bleue au point qui sera pris par la grille, l'ortho ou le polaire.
- Dessin : un clic remplace la sélection (Maj ajoute ou retire), Ctrl+Z / Ctrl+Y annulent et
  rétablissent, chevrons étroits aux extrémités du ruban.

### Amont
- SolidCraft v0.41.0 -> v0.51.2 (extension `cao`) : import STEP plus robuste, import IGES,
  mises en plan, assemblages, tôlerie. Aucun changement de notre code, sauf le numéro de version.
- CadCraft reste en v0.20.1 : pas de nouvelle release stable depuis le 11/11/2026.

*English: drawing step reworked (grouped ribbon with fold-out groups, French prompts, new
commands, Carbide-style selection frame with move / scale / rotate grips, Galaad-style
dimension box, right column with Properties / 3D / Comments tabs, contour tree with visibility
and lock, context menus, drawing options); on-demand update check; SolidCraft upgraded to
v0.51.2; CadCraft stays on v0.20.1 (no newer stable release).*

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
