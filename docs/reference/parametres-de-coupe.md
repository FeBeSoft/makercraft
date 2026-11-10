# Paramètres de coupe (fraisage CNC)

Référence pour un futur calculateur d'avances et de vitesses dans MakerCraft
(extension CAM, et proposition de valeurs par défaut dans c2d_to_nc).

## Sources

- BenMaker - Apprendre à calculer les paramètres de coupe pour la CNC :
  https://benmaker.fr/calculer-parametres-coupe-cnc/
- 3018 France - tableau CncFraises pour machines « hobby » (Vc et fz par matière et diamètre) :
  https://3018france.webador.fr/_downloads/43cb2ab6a283ab3ab6deeb04f94d0edb

## Formules

| Grandeur | Formule | Unités |
|---|---|---|
| Vitesse de broche | N = (Vc x 1000) / (π x D) | N tr/min, Vc m/min, D mm |
| Vitesse d'avance | Vf = N x Z x fz | Vf mm/min, Z dents, fz mm/dent |
| Charge par dent | fz = Vf / (N x Z) | mm/dent |
| Débit de copeaux | Q = ap x ae x Vf / 1000 | Q cm³/min, ap et ae mm |
| Profondeur de passe | ap = k x D | k selon matière et machine |

- ap : profondeur de passe axiale ; ae : engagement radial (pas latéral).
- Sur une machine hobby, la broche est souvent bridée (ex. 12 000 à 24 000 tr/min) :
  fixer N au maximum utile, puis recalculer Vf avec fz.
- Signe de bons paramètres : de vrais copeaux, pas de poussière (« paillettes ») ni de traces de brûlure.
- Appliquer une avance de plongée (axe Z) cohérente, en général une fraction de Vf.

## Pour MakerCraft

- Base matières (bois, MDF, contreplaqué, OSB, PMMA, aluminium...) avec Vc et fz par plage de diamètre.
- Profil machine : N min/max, Vf max, rigidité (coefficient de prudence).
- Sorties : N, Vf, avance de plongée, ap et ae conseillés, débit de copeaux.
