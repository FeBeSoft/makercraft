# Paramètres de coupe (fraisage CNC)

Référence pour un futur calculateur d'avances et de vitesses dans MakerCraft
(extension CAM, et proposition de valeurs par défaut dans c2d_to_nc).

## Sources

- BenMaker - Apprendre à calculer les paramètres de coupe pour la CNC (2022), abaque fourni par Euro-makers.com :
  https://benmaker.fr/calculer-parametres-coupe-cnc/
- BenMaker propose aussi un calculateur en ligne (article « Calculateur de paramètres de coupe pour la CNC »).
- 3018 France - tableau CncFraises pour machines « hobby » :
  https://3018france.webador.fr/_downloads/43cb2ab6a283ab3ab6deeb04f94d0edb

## Formules

| Grandeur | Formule | Unités |
|---|---|---|
| Vitesse de broche | n = (1000 x Vc) / (π x D) | n tr/min, Vc m/min, D mm |
| Vitesse d'avance XY | Vf = n x fz x Z | Vf mm/min, Z dents, fz mm/dent |
| Vitesse de plongée Z | Vf / 2 | mm/min |
| Profondeur de passe | ap = K x D (ou lue dans l'abaque) | mm |
| Vitesse de coupe | Vc = (D x π x n) / 1000 | m/min |
| Charge par dent | fz = Vf / (Z x n) | mm/dent |
| Débit de copeaux | Q = ap x ae x Vf / 1000 | cm³/min |

## Abaque (Euro-makers, via BenMaker)

Valeurs pour machines de bureau de type Shapeoko, X-Carve, Aureus 3x.
Pour un diamètre absent, prendre la ligne inférieure la plus proche.

### Vitesse de coupe Vc (m/min)

| Matière | HSS | Carbure |
|---|---|---|
| Bois tendre, MDF | 250 | 500 |
| Bois dur, contreplaqué | 250 | 500 |
| PVC expansé | 175 | 350 |
| POM, PMMA, PC | 130 | 260 |
| Cuivre, laiton | 75 à 100 | 150 à 200 |
| Aluminium, profil 1 | 50 à 75 | 100 à 150 |
| Aluminium, profil 2 | 30 à 50 | 60 à 100 |

### Avance par dent fz (mm/dent) selon le diamètre

| Matière | Ø2 | Ø3 | Ø4 | Ø5 | Ø6 |
|---|---|---|---|---|---|
| Bois tendre, MDF | 0,050 | 0,060 | 0,150 | 0,200 | 0,300 |
| Bois dur, contreplaqué | 0,040 | 0,060 | 0,090 | 0,130 | 0,180 |
| PVC expansé | 0,050 | 0,100 | 0,200 | 0,300 | 0,400 |
| POM, PMMA, PC | 0,025 | 0,035 | 0,050 | 0,070 | 0,100 |
| Cuivre, laiton | 0,015 | 0,025 | 0,040 | 0,050 | 0,070 |
| Aluminium, profil 1 | 0,015 | 0,025 | 0,035 | 0,045 | 0,060 |
| Aluminium, profil 2 | 0,006 | 0,015 | 0,020 | 0,030 | 0,040 |

### Profondeur de passe ap (mm) selon le diamètre

| Matière | Ø2 | Ø3 | Ø4 | Ø5 | Ø6 |
|---|---|---|---|---|---|
| Bois tendre, MDF | 1 | 2 | 3 | 4 | 6 |
| Bois dur, contreplaqué | 0,6 | 1,5 | 2,5 | 3,5 | 6 |
| PVC expansé | 1,2 | 2,5 | 4 | 5 | 6 |
| POM, PMMA, PC | 0,6 | 1,2 | 2 | 3,5 | 5 |
| Cuivre, laiton | 0,4 | 0,7 | 0,9 | 1,2 | 1,5 |
| Aluminium, profil 1 | 0,3 | 0,6 | 0,9 | 1,2 | 1,5 |
| Aluminium, profil 2 | 0,15 | 0,3 | 0,5 | 0,7 | 0,9 |

Les deux profils aluminium portent le même titre sur la page (« Si < 10 %, avec lubrification ») :
le second, plus prudent, correspond probablement à un alliage plus dur ou à un usinage sans lubrification. À vérifier sur l'abaque Euro-makers.

Coefficient machine sur ap : CNC 3018 (peu rigide) ap / 4 ; machine industrielle ap x 2,5.
Garde-fou : ne jamais dépasser 2 x D ni la moitié de la longueur de coupe de la fraise.

## Méthode d'adaptation aux limites de la machine

1. Lire Vc, fz et ap dans l'abaque (matière, type de fraise, diamètre).
2. Calculer n ; s'il dépasse n max de la broche, prendre n = n max.
3. Calculer Vf = n x fz x Z ; s'il dépasse Vf max de la machine, prendre Vf = Vf max.
4. Dans ce cas, réduire n dans le même rapport (ratio = Vf max / Vf calculée) pour garder la charge par dent fz.
5. Plongée = Vf / 2 ; appliquer le coefficient machine à ap.

Exemple : contreplaqué, carbure Ø3, 2 dents, machine 25 000 tr/min et 2 500 mm/min max.
n calculé 53 051, plafonné à 25 000 ; Vf = 25 000 x 0,06 x 2 = 3 000, plafonnée à 2 500 ;
ratio 0,83, donc n = 20 750 tr/min ; plongée 1 250 mm/min ; ap = 1,5 mm.

Valider par des essais : partir sous les valeurs calculées et augmenter progressivement.
Copeaux trop fins (poussière, échauffement) : avance trop faible par rapport à la rotation.
Copeaux trop gros (bourrage, broutage, risque de casse) : avance trop forte par rapport à la rotation.

## Pour MakerCraft

- Base matières reprenant l'abaque ci-dessus (fichier de données, modifiable par l'utilisateur).
- Profil machine : n min/max, Vf max, coefficient de rigidité sur ap.
- Profil fraise : D, Z, matériau (HSS ou carbure), longueur de coupe.
- Sorties : n, Vf, plongée, ap conseillé, débit de copeaux, avec l'étape de plafonnement affichée.
