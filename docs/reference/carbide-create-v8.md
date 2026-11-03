# Carbide Create V8 — fiche de référence (analyse boîte noire)

> Analyse du 3 novembre 2026 sur l'installation `D:\Program Files\CarbideCreateV8`
> (Copyright 2014-2026 Carbide 3D LLC). Méthode : inventaire des fichiers, dépendances PE
> (`dumpbin /dependents`), chaînes de caractères du binaire, en-têtes de fichiers.
> **Aucune décompilation, aucun code recopié.** Logiciel propriétaire : ce document sert à
> l'interopérabilité et à situer MakerCraft (CPI art. L122-6-1). Rien d'ici ne doit être
> transposé tel quel dans le code ; MakerCraft réécrit ses propres solutions.

## Architecture technique

| Élément | Constat | Indice |
|---|---|---|
| Langage, compilateur | C++, Visual C++ x64 (runtime 2015-2022) | `MSVCP140`, `VCRUNTIME140_1`, symboles décorés MSVC |
| Interface | **Qt 6 Widgets** (+ Svg, PrintSupport, Network) | DLL Qt6*, plugin `qmodernwindowsstyle` |
| Rendu 3D/2D du plan de travail | **QRhiWidget** (couche RHI de Qt 6.7+) : Direct3D 12 par défaut, OpenGL logiciel en secours | `QRhiWidget`, `dxcompiler.dll`, `dxil.dll`, `opengl32sw.dll`, shaders pré-compilés (« Shader failed to deserialize ») |
| Géométrie 2D | **Clipper** (v6, `ClipperLib`) **et Clipper2** : décalages, booléens | classes `ClipperLib::`, `Clipper2Lib::ClipperOffset` |
| Fichiers et bibliothèque | **SQLite** lié statiquement, avec **SQLite Encryption Extension** (AES-256-OFB) | `CODEC=see-aes256-ofb`, mention de licence Hipp, Wyrick & Co |
| Post-processeurs | Scripts **JavaScript** exécutés par **Duktape** (moteur JS embarqué) | `external\duktape\duktape.c`, fonctions `onOpen`… |
| Journalisation | bibliothèque de formatage type `fmt` / `std::format` | messages `{:.3f}` |
| Plantages | **BugSplat** (`BugSplat64.dll`, `BsSndRpt64.exe` = envoi des rapports) | — |
| Licence | clé collée dans l'app, version Pro débloquée par licence | `Paste license code below`, `Licensed To:` |
| Langues | interface en anglais uniquement (seules les traductions de Qt sont livrées) | `translations\qt_*.qm` |

Organisation interne visible par les noms : `art::` (modélisation de reliefs : `art::Doc`,
`art::applyRelief`, `art::operations::applyTexture`), `cam::` (`cam::ParallelPocket2`,
`cam::SimpleLinker`), `pocket_sort::BuildPocketTree` (arbre contours/îlots pour les poches).

## Format `.c2d`

- Fichier **SQLite** (depuis la V7 ; JSON avant). Requêtes présentes dans le binaire :
  - `params (key, value)` : réglages du document (clé/valeur) ;
  - `items (uuid, name, type, version, data, sz)` : un objet par ligne (forme, parcours…),
    `data` compressé (`sqlar_compress`), `version` par objet ;
  - `sqlar (name, mode, sz, data)` : **archive SQLite standard** pour les fichiers embarqués
    (images, modèles).
- `elements (name, data, pro)` : table de la **bibliothèque** (`ccpro.db`, 53 Mo), avec un
  drapeau `pro` pour le contenu réservé à la version Pro.
- **`ccpro.db` est chiffré** : en-tête aléatoire (`A5 91 F6 72…`) au lieu de `SQLite format 3`.
- **À vérifier sur un vrai `.c2d`** : chiffré ou non ? Lire ses 16 premiers octets suffit.
  - En clair (`SQLite format 3`) → import possible dans MakerCraft (tables ci-dessus, format
    des `data` à étudier sur des fichiers de test créés exprès).
  - Chiffré → **on s'arrête** : contourner une protection technique est interdit, même pour
    l'interopérabilité. On passera alors par les exports ouverts (SVG, DXF, G-code).
- Le `.c2d` contient le projet (dessins, brut, outils, **paramètres** des parcours), pas le
  G-code : celui-ci est calculé à l'export.

## Post-processeurs G-code

Quatre post-processeurs JavaScript embarqués : **« GRBL »**, **« Basic G-code »**,
**« Carbide 3D Shapeoko »**, **« Carbide 3D Nomad Pro »**.

API événementielle (même famille que les post-processeurs de Fusion 360) : `onOpen`,
`onSection`, `onRapid`, `onLinear`, `onSpindleSpeed`, `onComment`, `onClose`, `onTerminate`,
avec `writeLn`, `writeBlock`, `createFormatter`. Observations :
- **pas de `onCircular`** : les arcs sont sortis en segments droits (pas de G2/G3) ;
- en-tête `G90`, `G21` ou `G20` ; fin `M05` / `M02` ;
- retrait sécurisé en coordonnées machine : `G53 Z…` / `G53 G0 Z…` (hauteur depuis le haut) ;
- commentaires G-code entre parenthèses ;
- envoi direct possible vers **Carbide Motion** (« Send to Carbide Motion »), qui pilote la
  machine (Grbl modifié par Carbide 3D sur Shapeoko/Nomad).

→ Le G-code produit est du Grbl standard : `makercraft-send` peut l'envoyer tel quel.

## Fonctions (d'après les icônes de commandes et les types de parcours)

**Dessin 2D** : rectangle, cercle, polygone, polyligne, courbe, texte, édition de nœuds,
déplacement/rotation/échelle, miroirs, alignement, grouper/dégrouper, joindre, découper (trim),
congé (et congé sur tout), décalage, booléens (union, différence, intersection, soudure),
réseaux linéaire et circulaire, **imbrication (nest)**, constructeur de régions, vectorisation
d'image (trace), image de fond, règle, grille, notes, bibliothèque de formes.

**Parcours (FAO)** : contour, découpe, poche (avec reprise d'usinage, *rest machining*),
perçage (et pointage), surfaçage, gravure, `toolpath_drag` (rôle exact non vérifié, sans doute
couteau ou pointe traînés), trou de serrure
(*keyhole*), **V-carve** et **V-carve avancé**, **incrustation (inlay)**, texture, attaches
(*tabs*), **ébauche 3D** et **finition 3D** (dont *waterline*), projection sur modèle.

**Modélisation de reliefs (2.5D)** : composants de type *heightmap* (carte de hauteurs),
formes de base (plan, cylindre, forme extrudée), texture, image PNG en relief, **import STL**.

**Formats** : `.c2d` (natif), import DXF, SVG, STL, images (PNG/JPEG/GIF).

## Moteurs de dessin 2D et de simulation

- **Dessin 2D** : affichage par QRhi (GPU), géométrie par Clipper/Clipper2 (polygones entiers
  à virgule fixe pour des booléens et décalages robustes).
- **Reliefs et simulation** : sur une **grille raster** (carte de hauteurs). Le message
  `getDimensionsFromResolution … mm -> … px, … total pixels` montre une résolution en mm
  convertie en pixels : c'est un modèle *Z-map* (une hauteur par pixel), le plus simple et le
  plus rapide pour du 2.5D. Une option « High Resolution Simulation » (désactivable), une
  simulation animée pas à pas et une vue dédiée.
- Pas de bibliothèque géométrique lourde (ni CGAL, ni Open CASCADE, ni Eigen trouvés) : le cœur
  CAM et relief est fait maison.

## Ce qu'on en retient pour MakerCraft

| Sujet | Décision proposée |
|---|---|
| Rendu | même approche (GPU) : on a déjà **wgpu** via egui (CadCraft/SolidCraft), équivalent de QRhi |
| Géométrie 2D | booléens et décalages robustes indispensables : crate Rust **`clipper2`** (port de Clipper2, licence Boost), ou `geo` + `i_overlay` |
| Simulation | commencer par un **Z-map** (carte de hauteurs en pixels) : suffisant pour laser et fraisage 2.5D, facile à afficher en texture |
| Post-processeurs | gabarits simples par machine (Grbl, grblHAL, Shapeoko) **avec arcs G2/G3** (meilleur que Carbide Create) ; un moteur de script n'est pas nécessaire au début |
| Import `.c2d` | seulement si les fichiers sont en clair ; sinon SVG/DXF/G-code |
| Fonctions prioritaires (fablab) | contour/découpe avec attaches, poche, perçage, gravure, V-carve, imbrication |

## Limites de cette analyse

- Aucune exécution de Carbide Create, aucune décompilation : les comportements fins (ordre des
  passes, calcul du V-carve, choix des lieux d'attaches) ne sont pas connus.
- Le chiffrement des `.c2d` n'est pas encore vérifié (aucun fichier de test disponible).
- Les noms de tables et d'opérations viennent des chaînes du binaire : leur rôle exact est déduit.
