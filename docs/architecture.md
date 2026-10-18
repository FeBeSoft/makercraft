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
| L3 | `ext/dao` | adaptateur CadCraft (dessin 2D, DXF/DWG/SVG/PDF) | à faire |
| L3 | `ext/cao` | adaptateur SolidCraft (B-rep truck, STEP, 3MF) | à faire |
| L3 | `ext/cam` | parcours : laser vectoriel/raster, fraisage 2.5D | à faire |
| L3 | `ext/machine` | transport, contrôleur Grbl/grblHAL, G-code | 🟡 squelette |
| L4 | `ui/shell` | egui : menus et panneaux construits depuis le registre | à faire |
| app | `makercraft-cli` | lister / exécuter les commandes | ✅ |

Règle : une couche ne dépend que des couches inférieures ; le noyau ne connaît aucune extension.

## Contrat d'extension

Une extension implémente `Extension` (`id`, `label`, `depends_on`, `register`). L'hôte charge
un lot d'extensions dans l'ordre de leurs dépendances et refuse : doublons, dépendance absente ou
circulaire, commande hors de son espace de noms (`machine.*` pour l'extension `machine`).

Les extensions métier sont **compilées** avec l'app (features cargo) : pas de DLL, donc pas
d'`unsafe` ni d'ABI instable. Les extensions tierces viendront en WebAssembly, avec la même
surface `run(commande, json)` que celle prévue par SolidCraft (`docs/plugins.md` chez eux).

## Échanges entre extensions (à venir)

La FAO ne connaît pas les types internes de CadCraft ou SolidCraft ; elle consomme des
fournisseurs neutres enregistrés par les extensions :

- `Profile2DSource` (dao, cao) : contours fermés, chemins ouverts, calque d'origine ;
- `SolidSource` (cao) : maillage, sections planes (tranches) ;
- `MachineSink` (machine) : capacités de la machine, envoi d'un programme G-code.

## Feuille de route

1. v0.1 envoyeur : transport série/TCP/émulateur, Grbl 1.1/grblHAL, console, jog, streaming.
2. v0.2 laser vectoriel : dessin → G-code, puissance/vitesse par calque, M4, passes, framing.
3. v0.3 raster : image → ligne à ligne, tramage, vectorisation.
4. v0.4 CNC : axe Z, palpage, heightmap, G54–G59.
5. CAO : SolidCraft, tranches → découpe laser, fraisage 2.5D.
