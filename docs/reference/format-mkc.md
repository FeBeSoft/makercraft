# Format de projet `.mkc` - version 2 (spécification)

Statut : **proposition validée le 28/11/2026, pas encore codée** (tâche t017). Crate : `crates/ext/mkc`.

Un fichier `.mkc` est une **archive zip** qui contient plusieurs **parties**, sur le principe des
documents Office (`.docx`) et OpenDocument (`.odt`) : un sommaire, des parties versionnées une à
une, et les formats natifs de CadCraft et SolidCraft gardés tels quels.

## 1. Pourquoi le zip

- Accès direct à une partie sans tout décompresser.
- Ouvrable partout (Windows, macOS, Linux, ENT, Nuage) : on peut renommer en `.zip` et extraire
  le DXF à la main.
- Un projet pèse de quelques centaines de Ko à quelques Mo : réécrire tout le fichier à chaque
  enregistrement ne coûte rien.
- Écartés : 7z (bibliothèques et outils moins répandus), tar + zstd (pas d'accès direct),
  SQLite (opaque pour l'utilisateur ; reste possible plus tard comme **partie** du zip, par
  exemple pour un historique complet des versions).

## 2. Règles de l'archive

1. **Méthodes de compression** : Deflate, ou Stored (sans compression). Jamais zstd, LZMA ni
   bzip2 : l'explorateur Windows ne sait pas les ouvrir.
2. **Premier fichier : `mimetype`**, en Stored (non compressé), sans champ extra, contenu exact
   `application/vnd.febe.makercraft` (ASCII, sans fin de ligne). On reconnaît un `.mkc` en lisant
   ses premiers octets, même renommé.
3. **Noms des parties** : ASCII, minuscules, sans accent ni espace, séparateur `/`. Le drapeau
   UTF-8 (bit 11) est quand même posé à l'écriture, pour les noms venant de l'utilisateur
   (pièces jointes, voir `medias/`).
4. **Sécurité à la lecture** : refuser les chemins absolus, les `..`, les liens symboliques, les
   doublons ; plafonds de taille par partie et au total (contre les archives piégées), vérifiés
   pendant la décompression et pas seulement d'après l'en-tête.
5. **Enregistrement sûr** : écriture dans `<nom>.mkc.tmp` à côté du fichier, relecture et
   vérification du manifeste, puis remplacement par renommage. Indispensable sur un disque
   synchronisé (Nextcloud, Leviia, Nuage).
6. **Dates** des entrées du zip : celles de l'enregistrement (la date de création du projet est
   dans `projet/projet.json`).

## 3. Structure

```
projet.mkc
├─ mimetype                    identification (règle 2)
├─ manifest.json               sommaire des parties (section 4) - obligatoire
├─ projet/
│  ├─ projet.json              nom, auteur, dates, version de MakerCraft, unités, atelier - obligatoire
│  ├─ machine.json             machine choisie, profil figé à l'enregistrement
│  ├─ brut.json                brut, matière, zéro pièce, brides, martyr
│  ├─ outils.json              outils retenus pour cette pièce (copie, pas un lien vers la banque)
│  └─ parcours.json            parcours, groupes par outil, ordre
├─ dessin/
│  ├─ dessin.dxf | dessin.dwg  dessin natif CadCraft, inchangé
│  └─ contours.json            ce que CadCraft ne garde pas : noms affichés, groupes, réseaux
├─ modele/modele.sc            modèle natif SolidCraft (facultatif)
├─ usinage/                    G-code produit (facultatif, voir section 7)
├─ medias/                     photos, images de fond, PDF joints (facultatif)
├─ classe/                     élève, classe, validation du prof, remarques (facultatif)
├─ historique/journal.json     enregistrements et usinages (facultatif, lien avec t009)
└─ apercu.png                  vignette PNG 256 x 256 au plus (facultatif, t010)
```

Seuls `mimetype`, `manifest.json` et `projet/projet.json` sont obligatoires : un projet vide est
valide.

## 4. Le manifeste

`manifest.json` liste **toutes** les parties, sauf `mimetype` et lui-même.

```json
{
  "format": "makercraft",
  "format_version": 2,
  "generator": "MakerCraft 0.2.0",
  "saved": "2026-11-28T09:15:00+01:00",
  "parts": [
    { "path": "projet/projet.json",   "type": "mkc.projet",   "version": 1, "sha256": "…", "size": 412 },
    { "path": "projet/parcours.json", "type": "mkc.parcours", "version": 1, "sha256": "…", "size": 2210 },
    { "path": "dessin/dessin.dxf",    "type": "cadcraft.dxf", "version": "0.20.1", "sha256": "…", "size": 48113 },
    { "path": "medias/photo-1.jpg",   "type": "image/jpeg",   "sha256": "…", "size": 812044 }
  ]
}
```

- `type` : `mkc.<partie>` pour nos parties, `cadcraft.dxf` / `cadcraft.dwg` / `solidcraft.sc`
  pour les natifs (la `version` est alors celle de CadCraft ou SolidCraft qui l'a écrite), type
  MIME pour les médias.
- `sha256` et `size` : du contenu décompressé.
- `version` : version **de la partie**, entier qui monte à chaque changement incompatible.

## 5. Règles de lecture et d'écriture

1. **Le manifeste fait foi** : une entrée du zip absente du manifeste est ignorée (et signalée).
2. **Empreinte fausse** : la partie est déclarée abîmée. Le projet s'ouvre quand même si elle
   n'est pas obligatoire, avec un avertissement qui la nomme ; sinon, ouverture refusée avec la
   raison.
3. **Version de partie trop récente** : la partie est ouverte en lecture seule ou ignorée, avec un
   message clair (« projet enregistré avec une version plus récente de MakerCraft »), jamais de
   plantage.
4. **Parties inconnues conservées** : une partie que cette version ne connaît pas (type inconnu)
   est recopiée telle quelle à l'enregistrement, avec sa ligne du manifeste. Rien ne se perd
   entre deux postes qui n'ont pas la même version.
5. **Projet autonome** : machine et outils sont copiés dans le projet. À l'ouverture, MakerCraft
   compare avec la banque locale et signale les différences (outil absent, valeurs changées),
   sans rien modifier sans accord.
6. **Ordre d'écriture** : `mimetype`, `manifest.json`, `projet/projet.json`, puis les autres
   parties par chemin. Deux enregistrements identiques donnent des parties identiques (JSON trié,
   indentation fixe), ce qui rend les différences lisibles.

## 6. Compatibilité avec le format 1

Le format 1 (v0.1.0) contient `makercraft.json`, `dessin.dxf` ou `dessin.dwg`, et `modele.sc`,
sans `mimetype` ni manifeste.

- Reconnaissance : pas de `mimetype` et présence de `makercraft.json`.
- Migration à l'ouverture : `makercraft.json` est découpé en `projet/*.json`, les natifs sont
  déplacés dans `dessin/` et `modele/`.
- Le fichier d'origine n'est jamais réécrit sans accord : au premier enregistrement, MakerCraft
  propose d'enregistrer au format 2 (l'ancien fichier est gardé en `.mkc.v1`).

## 7. Points ouverts

- **G-code dans le projet** (`usinage/`) : à trancher. Pour : la fiche de gamme (t063) et le
  dépôt des élèves ont tout dans un seul fichier. Contre : le G-code se régénère et peut devenir
  faux si on modifie un parcours après coup. Proposition : le garder, marqué avec l'empreinte des
  parties dont il dépend, et le signaler « périmé » si elles ont changé.
- Signature de la validation du prof dans `classe/` (lien avec t101).
- Historique complet des versions (SQLite en partie du zip) : plus tard.

## 8. Tests à écrire

- Aller-retour : enregistrer, relire, comparer partie par partie.
- Partie inconnue conservée à l'enregistrement.
- Partie abîmée détectée (empreinte), partie obligatoire manquante refusée.
- Chemins dangereux et archive piégée refusés.
- Lecture d'un `.mkc` format 1 (exemple porte-clés) et migration.
- Archive ouverte par un outil zip standard ; `mimetype` lisible en tête de fichier.
