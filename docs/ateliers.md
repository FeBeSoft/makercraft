# Ateliers autonomes et intégrés

Statut : proposition (2026-11-11). Complète `architecture.md`.

Objectif : chaque atelier (Dessin, Fabrication, Pilotage, PCB...) peut fonctionner **seul**, dans une
petite application dédiée, et s'intègre **automatiquement** à MakerCraft quand il y est présent,
comme un atelier de plus dans le sélecteur. Un projet commencé dans l'un s'ouvre dans l'autre.

## 1. Principe : une bibliothèque, deux coquilles

Un atelier est un crate de **bibliothèque** (`crates/ateliers/<nom>`) qui contient sa logique et son
interface. Il ne sait pas dans quelle application il tourne : il reçoit un contexte et dessine.

| Mode | Coquille | Contenu |
|---|---|---|
| Autonome | `apps/mc-<nom>` (ex. `mc-dessin`) | fenêtre minimale, un seul atelier, sans sélecteur |
| Intégré | `apps/makercraft` | tous les ateliers compilés, sélecteur d'ateliers |

Les deux coquilles partagent la même base (`crates/ui/shell`) : barre de titre, ruban, fichier,
historique, thèmes. Une coquille autonome n'est qu'une configuration de cette base avec un seul atelier.

Variante sans second exe : `makercraft.exe --atelier dessin` démarre en mode autonome.

## 2. Règle de dépendance

```
apps/makercraft ──┐
apps/mc-dessin ───┼──> crates/ui/shell ──> crates/ateliers/* ──> crates/core/*
apps/c2d_to_nc ───┘
```

- Un atelier dépend **uniquement** du noyau (`core/doc`, `core/ext-api`, `core/geom`, `core/units`, `core/io`).
- **Un atelier ne dépend jamais d'un autre atelier.** Les échanges passent par le document et par
  des fournisseurs neutres (section 5). C'est ce qui rend toutes les combinaisons possibles.
- Le noyau ne connaît aucun atelier (règle déjà en vigueur pour les extensions).

## 3. Le contrat `Atelier`

Un atelier est une `Extension` (id, label, depends_on, register) avec une face graphique en plus.
Esquisse, à affiner lors de la preuve de concept :

```rust
pub trait Atelier: Extension {
    /// Identité affichée dans le sélecteur.
    fn fiche(&self) -> FicheAtelier;            // nom, description, couleur, icône, état
    /// Onglets et groupes du ruban, en données ; chaque bouton renvoie à une commande du registre.
    fn ruban(&self) -> Vec<Onglet>;
    /// Zone centrale (vue 2D, 3D, console...) et panneau latéral facultatif.
    fn ui_centre(&mut self, ui: &mut egui::Ui, ctx: &mut Contexte);
    fn ui_panneau(&mut self, _ui: &mut egui::Ui, _ctx: &mut Contexte) {}
    /// Section du projet dont l'atelier est propriétaire (voir section 4).
    fn section(&self) -> &'static str;
    /// Formats d'import / export proposés dans le menu Fichier.
    fn formats(&self) -> Vec<FormatFichier>;
    /// Appelé quand l'atelier devient actif ou inactif (outils en cours, verrou du sélecteur).
    fn activer(&mut self, _ctx: &mut Contexte) {}
    fn desactiver(&mut self, _ctx: &mut Contexte) -> Result<(), Occupe> { Ok(()) }
}

pub struct FicheAtelier {
    pub nom: &'static str,
    pub description: &'static str,
    pub couleur: [u8; 3],
    pub icone: Icone,
    pub etat: Etat,                              // Disponible, Prevu
}
```

- `Contexte` donne accès au document, à la sélection partagée, à l'historique et aux services
  (fournisseurs, profils machine, journal).
- Toute modification du document passe par une **commande** du registre (`dessin.rectangle`,
  `fabrication.poche`...) : l'historique annuler/rétablir est donc commun à tous les ateliers,
  et tout reste pilotable en JSON (CLI, tests, MCP).
- `desactiver` peut refuser (`Occupe`) : c'est le verrou du sélecteur pendant une esquisse en cours.

## 4. Un seul format de projet : `.makercraft`

Conteneur zip ; chaque atelier possède sa section.

```
projet.makercraft
├── manifest.json        version du format, ateliers ayant écrit, unités, date
├── dessin/              géométrie 2D, calques, textes
├── modelisation/        corps 3D (format natif SolidCraft ou STEP)
├── fabrication/         brut, outils, parcours
├── pilotage/            profil machine (ex. 3018 Pro), décalages
├── apercus/             vignettes PNG pour l'explorateur et l'accueil
└── <section inconnue>/  conservée telle quelle
```

Règles :

1. Un atelier lit et écrit **sa** section ; il peut lire les autres via les fournisseurs, jamais les modifier directement.
2. **Une section inconnue est toujours conservée** à l'enregistrement. Ouvrir un projet complet dans
   MC Dessin puis l'enregistrer ne perd ni les parcours ni le profil machine.
3. Chaque section a sa propre version ; un atelier plus ancien ouvre en lecture seule une section plus récente que lui et le signale.
4. `manifest.json` liste les ateliers ayant écrit ; s'il en manque un à l'ouverture, MakerCraft
   l'indique (« ce projet contient une section PCB : atelier non installé »).
5. Les formats externes (SVG, DXF, .c2d, .nc, STEP) restent des imports / exports, pas des formats de projet.

## 5. Échanges entre ateliers : les fournisseurs

Déjà esquissés dans `architecture.md` ; un atelier enregistre ce qu'il fournit, un autre le consomme
sans connaître son type interne.

| Fournisseur | Fourni par | Consommé par |
|---|---|---|
| `Profile2DSource` : contours fermés, chemins ouverts, calque | Dessin, Modélisation, PCB | Fabrication, Laser, Imbrication |
| `SolidSource` : maillage, sections planes | Modélisation | Fabrication (relief), Laser (tranches) |
| `ToolpathSource` : programme G-code et aperçu | Fabrication, Laser, PCB | Pilotage |
| `MachineSink` : capacités, envoi d'un programme | Pilotage | Fabrication, Laser, PCB, Calcul de coupe |
| `MachineProfile` : n max, Vf max, rigidité | Pilotage (ou projet) | Calcul de coupe, Fabrication |

Si aucun fournisseur n'est présent, l'atelier consommateur dégrade proprement : la Fabrication
autonome importe un SVG ou un .c2d au lieu de lire l'atelier Dessin ; elle exporte des .nc au lieu
d'envoyer à la machine.

## 6. « S'il est présent » : deux niveaux d'intégration

Rust n'a pas d'ABI stable et les types egui ne franchissent pas proprement une frontière de DLL :
charger une **interface** à l'exécution serait fragile et demanderait de l'`unsafe`. D'où deux niveaux.

1. **Ateliers FeBe (avec interface) : composés à la compilation.** Une feature cargo par atelier
   (`atelier-dessin`, `atelier-fabrication`...). MakerCraft complet les active toutes ; une coquille
   autonome n'en active qu'une. « Présent » veut dire « compilé dedans ». Le sélecteur se construit
   à partir des ateliers enregistrés : rien à câbler à la main.
2. **Extensions tierces (sans interface propre) : chargées à l'exécution en WebAssembly** depuis
   `%APPDATA%\FeBe\MakerCraft\extensions\` : importeurs, post-processeurs, pilotes de machine, abaques.
   Découverte au démarrage, bac à sable, surface `run(commande, json)` identique au registre,
   paramètres décrits en JSON pour générer leurs fenêtres de réglage automatiquement.

## 7. Correspondance avec les projets existants

| Atelier | Version autonome | État |
|---|---|---|
| Fabrication | `c2d_to_nc` (import .c2d, export .nc) | existe : premier cas réel, à faire passer sur le contrat |
| Pilotage | Galaad Jog (Isel), puis MC Pilotage (Grbl / grblHAL) | en réflexion |
| Calcul de coupe | MC Coupe (abaque, profil 3018 Pro) | idée : candidat preuve de concept |
| Dessin | MC Dessin (base CadCraft) | à faire |
| PCB | MC PCB (Gerber, Excellon, isolation sur 3018) | idée |

## 8. Plan de mise en oeuvre

1. **Noyau** : `core/doc` (document, sélection, historique par commandes) et le conteneur `.makercraft`
   avec conservation des sections inconnues ; tests d'aller-retour.
2. **Coquille** : `ui/shell` (ruban construit depuis `Atelier::ruban`, sélecteur de la maquette
   `makercraft-ateliers.html`, menu Fichier depuis `Atelier::formats`).
3. **Preuve de concept : atelier Calcul de coupe**, petit et sans dépendance graphique lourde ;
   il doit tourner en autonome (`mc-coupe`) et dans `makercraft` avec un second atelier factice.
   Critères : même code, même fichier de projet, aucune dépendance entre ateliers.
4. **Migration de c2d_to_nc** vers l'atelier Fabrication, en gardant son exe et son dépôt public
   (le crate atelier devient une dépendance git épinglée du dépôt c2d_to_nc).
5. Ensuite : Pilotage, Dessin, puis extensions WebAssembly.

## Questions ouvertes

- Monodépôt (tous les ateliers dans MakerCraft) ou un dépôt par atelier ? Proposition : monodépôt
  pour le développement, exes autonomes publiés séparément (comme c2d_to_nc).
- Nom des exes autonomes : « MC Dessin » ou « DessinCraft » ? (cohérence avec MakerCraft / GravéoCraft)
- Conteneur `.makercraft` : zip de JSON (lisible, diffable) ou SQLite (comme Carbide Create) ?
  Proposition : zip de JSON.
