//! Lecture d'un programme G-code avant l'envoi : lignes à envoyer et notes de pause.
//!
//! Les commentaires ne sont pas envoyés à Grbl. Celui qui précède une pause programmée (`M0`,
//! `M1`) est gardé à part pour être affiché quand la machine s'arrête (rappel de pince, outil à
//! monter...).

use crate::protocol::prepare_line;

/// Programme prêt à envoyer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProgramText {
    /// Lignes envoyées au contrôleur, telles qu'écrites dans le fichier.
    pub lines: Vec<String>,
    /// (index dans `lines` d'un `M0`/`M1`, commentaire qui le précède).
    pub notes: Vec<(usize, String)>,
}

impl ProgramText {
    pub fn parse(text: &str) -> Self {
        let mut p = Self::default();
        let mut pending: Vec<String> = Vec::new();
        for raw in text.lines() {
            let comment = comment_of(raw);
            match prepare_line(raw) {
                Some(clean) if !clean.trim().is_empty() => {
                    if let Some(c) = comment {
                        pending.push(c);
                    }
                    if is_pause(&clean) && !pending.is_empty() {
                        p.notes.push((p.lines.len(), pending.join(" - ")));
                    }
                    pending.clear();
                    p.lines.push(raw.to_owned());
                }
                _ => {
                    if let Some(c) = comment {
                        pending.push(c);
                    }
                }
            }
        }
        p
    }

    /// Note de la dernière pause déjà envoyée (`sent` lignes parties vers la machine).
    pub fn note_before(&self, sent: usize) -> Option<&str> {
        self.notes.iter().rev().find(|(i, _)| *i < sent).map(|(_, s)| s.as_str())
    }
}

/// Texte des commentaires d'une ligne (`( ... )` et `; ...`), sans les délimiteurs.
fn comment_of(raw: &str) -> Option<String> {
    let mut out = Vec::new();
    let mut rest = raw;
    while let Some(a) = rest.find('(') {
        let after = &rest[a + 1..];
        let Some(b) = after.find(')') else { break };
        out.push(after[..b].trim().to_owned());
        rest = &after[b + 1..];
    }
    if let Some(s) = rest.find(';') {
        out.push(rest[s + 1..].trim().to_owned());
    }
    out.retain(|s| !s.is_empty());
    if out.is_empty() { None } else { Some(out.join(" ")) }
}

/// `true` si la ligne contient une pause programmée `M0` ou `M1`.
fn is_pause(clean: &str) -> bool {
    let up = clean.to_ascii_uppercase();
    let b = up.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'M' {
            let mut j = i + 1;
            while j < b.len() && b[j] == b' ' {
                j += 1;
            }
            let start = j;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            let frac = j < b.len() && b[j] == b'.';
            if j > start && !frac && up[start..j].parse::<u32>().is_ok_and(|n| n <= 1) {
                return true;
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_of_tool_change() {
        let p = ProgramText::parse("G21\n(fin outil 1)\nG0 Z10\n(CHANGEMENT D'OUTIL)\n(Changer outil T2 : fraise plate 3.175 - pince ER11 3.175)\nM0\nG1 X1 F100\nM30\n");
        assert_eq!(p.lines.len(), 5);
        assert_eq!(p.notes.len(), 1);
        assert_eq!(p.notes[0].0, 2);
        assert_eq!(p.note_before(2), None);
        let n = p.note_before(3).unwrap_or_default();
        assert!(n.contains("pince ER11 3.175") && n.contains("CHANGEMENT"), "{n}");
    }

    #[test]
    fn pause_detection() {
        assert!(is_pause("M0"));
        assert!(is_pause("M00"));
        assert!(is_pause("G0 Z5 M1"));
        assert!(!is_pause("M3 S1000"));
        assert!(!is_pause("M30"));
        assert!(!is_pause("M5"));
        assert!(!is_pause("M10"));
        let p = ProgramText::parse("M0 (pause seule)\n");
        assert_eq!(p.notes, vec![(0, "pause seule".to_owned())]);
    }
}
