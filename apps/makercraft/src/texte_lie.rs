//! Texte avec liens intégrés, commun aux logiciels FeBe (copie de `H:\_src\_commun\rust\texte_lie.rs`).
//!
//! Dans les textes communs (bio, etc.), `[libellé](adresse)` affiche « libellé » cliquable,
//! sans montrer l'adresse ; le reste est du texte brut. Paragraphes séparés par une ligne vide.

use eframe::egui::{self, Color32, RichText, Ui};

/// Morceau de texte : texte simple ou lien.
#[derive(Debug, PartialEq, Eq)]
pub enum Morceau<'a> {
    Texte(&'a str),
    Lien(&'a str, &'a str),
}

/// Découpe un texte en morceaux selon la syntaxe `[libellé](adresse)`.
pub fn decouper(texte: &str) -> Vec<Morceau<'_>> {
    let mut out = Vec::new();
    let mut reste = texte;
    let mut debut_texte = 0usize; // début du texte simple en attente, dans `reste`
    let mut i = 0usize;
    while let Some(p) = reste[i..].find('[') {
        let deb = i + p;
        if let Some((libelle, adresse, fin)) = lien_a(&reste[deb..]) {
            if deb > debut_texte {
                out.push(Morceau::Texte(&reste[debut_texte..deb]));
            }
            out.push(Morceau::Lien(libelle, adresse));
            reste = &reste[deb + fin..];
            debut_texte = 0;
            i = 0;
        } else {
            i = deb + 1;
        }
    }
    if reste.len() > debut_texte {
        out.push(Morceau::Texte(&reste[debut_texte..]));
    }
    out
}

/// Si `s` commence par `[libellé](adresse)`, renvoie (libellé, adresse, longueur consommée).
fn lien_a(s: &str) -> Option<(&str, &str, usize)> {
    let fin_lib = s.find("](")?;
    let libelle = &s[1..fin_lib];
    if libelle.is_empty() || libelle.contains(['[', ']', '\n']) {
        return None;
    }
    let reste = &s[fin_lib + 2..];
    let fin_adr = reste.find(')')?;
    let adresse = &reste[..fin_adr];
    if adresse.is_empty() || adresse.contains(char::is_whitespace) {
        return None;
    }
    Some((libelle, adresse, fin_lib + 2 + fin_adr + 1))
}

#[allow(dead_code)]
/// Texte sans la syntaxe des liens (libellés seuls), pour une copie ou un affichage simple.
pub fn sans_liens(texte: &str) -> String {
    decouper(texte)
        .into_iter()
        .map(|m| match m {
            Morceau::Texte(t) | Morceau::Lien(t, _) => t,
        })
        .collect()
}

/// Apparence du texte ; `None` : valeur du thème egui.
#[derive(Clone, Copy, Default)]
pub struct Style {
    pub taille: Option<f32>,
    pub couleur: Option<Color32>,
    pub couleur_lien: Option<Color32>,
    pub interligne: Option<f32>,
}

/// Affiche le texte, liens cliquables (ouverts dans le navigateur, adresse en info-bulle).
pub fn afficher(ui: &mut Ui, texte: &str, style: Style) {
    let base = |t: &str| {
        let mut rt = RichText::new(t);
        if let Some(s) = style.taille {
            rt = rt.size(s);
        }
        if let Some(h) = style.interligne {
            rt = rt.line_height(Some(h));
        }
        rt
    };
    let ecart = style.taille.unwrap_or(13.0) * 0.8;
    for (n, para) in texte.trim().split("\n\n").enumerate() {
        if n > 0 {
            ui.add_space(ecart);
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for m in decouper(para.trim_matches('\n')) {
                match m {
                    Morceau::Texte(t) => {
                        let mut rt = base(t);
                        if let Some(c) = style.couleur {
                            rt = rt.color(c);
                        }
                        ui.add(egui::Label::new(rt).wrap());
                    }
                    Morceau::Lien(l, u) => {
                        let mut rt = base(l);
                        if let Some(c) = style.couleur_lien {
                            rt = rt.color(c);
                        }
                        ui.hyperlink_to(rt, u);
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoupe_les_liens() {
        let m = decouper("avec [Mórrighan](https://a.fr/@M), puis [x] et [b](c d).");
        assert_eq!(
            m,
            vec![
                Morceau::Texte("avec "),
                Morceau::Lien("Mórrighan", "https://a.fr/@M"),
                Morceau::Texte(", puis [x] et [b](c d)."),
            ]
        );
        assert_eq!(sans_liens("a [b](https://c) d"), "a b d");
    }
}
