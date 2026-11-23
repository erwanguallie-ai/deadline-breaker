//! Générateur PDF minimal, sans dépendance : texte sur pages A4, polices standard Helvetica
//! (aucune police à embarquer), encodage WinAnsi (accents français, « », €, œ).
//! Utilisé pour l'export du journal d'activité.

const LARGEUR_PAGE: f32 = 595.0;
const HAUTEUR_PAGE: f32 = 842.0;
const MARGE: f32 = 56.0;
const BAS_PAGE: f32 = 64.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Police {
    Normale,
    Grasse,
}

/// Couleur de gris (0 = noir, 1 = blanc).
#[derive(Clone, Copy)]
pub struct Style {
    pub police: Police,
    pub taille: f32,
    pub gris: f32,
}

pub const TITRE: Style = Style { police: Police::Grasse, taille: 16.0, gris: 0.07 };
pub const SOUS_TITRE: Style = Style { police: Police::Normale, taille: 9.5, gris: 0.4 };
pub const GRAS: Style = Style { police: Police::Grasse, taille: 10.5, gris: 0.07 };
pub const TEXTE: Style = Style { police: Police::Normale, taille: 9.5, gris: 0.2 };
pub const DISCRET: Style = Style { police: Police::Normale, taille: 8.5, gris: 0.45 };

/// Conversion d'un caractère Unicode vers WinAnsi (cp1252).
fn winansi(c: char) -> Vec<u8> {
    let u = c as u32;
    if (0x20..0x7F).contains(&u) || (0xA0..=0xFF).contains(&u) {
        return vec![u as u8];
    }
    let b = match c {
        '€' => 0x80, '‚' => 0x82, 'ƒ' => 0x83, '„' => 0x84, '…' => 0x85, '†' => 0x86, '‡' => 0x87, 'ˆ' => 0x88,
        '‰' => 0x89, 'Š' => 0x8A, '‹' => 0x8B, 'Œ' => 0x8C, 'Ž' => 0x8E, '‘' => 0x91, '’' => 0x92, '“' => 0x93,
        '”' => 0x94, '•' => 0x95, '–' => 0x96, '—' => 0x97, '˜' => 0x98, '™' => 0x99, 'š' => 0x9A, '›' => 0x9B,
        'œ' => 0x9C, 'ž' => 0x9E, 'Ÿ' => 0x9F, '\u{202F}' | '\u{2007}' | '\u{2009}' => 0xA0,
        '→' => return b"->".to_vec(),
        '≤' => return b"<=".to_vec(),
        '≥' => return b">=".to_vec(),
        '\t' => b' ',
        _ => b'?',
    };
    vec![b]
}

pub fn encoder(texte: &str) -> Vec<u8> {
    texte.chars().flat_map(winansi).collect()
}

/// Largeurs Helvetica / Helvetica-Bold (millièmes d'em) des caractères 32 à 126.
const HELV: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556, 556,
    556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778,
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556,
    556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];
const HELV_GRAS: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556, 556,
    556, 556, 333, 333, 584, 584, 584, 611, 975, 722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722, 611, 833, 722, 778,
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 333, 278, 333, 584, 556, 333, 556, 611, 556, 611, 556, 333, 611,
    611, 278, 278, 556, 278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584,
];

/// Lettre de base d'un caractère accentué (pour sa largeur).
fn base(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'î' | 'ï' | 'í' | 'ì' => 'i',
        'Î' | 'Ï' | 'Í' | 'Ì' => 'I',
        'ô' | 'ö' | 'ó' | 'ò' | 'õ' => 'o',
        'Ô' | 'Ö' | 'Ó' | 'Ò' | 'Õ' => 'O',
        'ù' | 'û' | 'ü' | 'ú' => 'u',
        'Ù' | 'Û' | 'Ü' | 'Ú' => 'U',
        'ç' => 'c',
        'Ç' => 'C',
        'ÿ' => 'y',
        'Ÿ' => 'Y',
        'ñ' => 'n',
        '’' | '‘' => '\'',
        '«' | '»' => '<',
        '–' => '-',
        '\u{A0}' | '\u{202F}' => ' ',
        _ => c,
    }
}

pub fn largeur(texte: &str, police: Police, taille: f32) -> f32 {
    let table = if police == Police::Grasse { &HELV_GRAS } else { &HELV };
    let total: u32 = texte
        .chars()
        .map(|c| match c {
            'Œ' => 1000,
            'œ' | '€' => 944,
            '—' => 1000,
            _ => {
                let b = base(c) as u32;
                if (32..127).contains(&b) { table[(b - 32) as usize] as u32 } else { 556 }
            }
        })
        .sum();
    total as f32 * taille / 1000.0
}

/// Découpe un texte en lignes tenant dans `max` points (coupe aussi les mots trop longs).
pub fn couper(texte: &str, style: Style, max: f32) -> Vec<String> {
    let mut lignes = Vec::new();
    for paragraphe in texte.split('\n') {
        let mut ligne = String::new();
        for mot in paragraphe.split_whitespace() {
            let essai = if ligne.is_empty() { mot.to_string() } else { format!("{} {}", ligne, mot) };
            if largeur(&essai, style.police, style.taille) <= max {
                ligne = essai;
                continue;
            }
            if !ligne.is_empty() {
                lignes.push(std::mem::take(&mut ligne));
            }
            // Mot plus long que la ligne : coupé caractère par caractère.
            let mut morceau = String::new();
            for c in mot.chars() {
                morceau.push(c);
                if largeur(&morceau, style.police, style.taille) > max {
                    morceau.pop();
                    lignes.push(std::mem::take(&mut morceau));
                    morceau.push(c);
                }
            }
            ligne = morceau;
        }
        lignes.push(ligne);
    }
    lignes
}

enum Op {
    Texte { x: f32, y: f32, style: Style, contenu: String },
    Trait { y: f32 },
}

/// Document en cours de mise en page.
pub struct Document {
    pages: Vec<Vec<Op>>,
    y: f32,
    pied: String,
}

impl Document {
    pub fn nouveau(pied: &str) -> Document {
        Document { pages: vec![Vec::new()], y: HAUTEUR_PAGE - MARGE, pied: pied.to_string() }
    }

    fn page(&mut self) -> &mut Vec<Op> {
        self.pages.last_mut().expect("au moins une page")
    }

    fn saut_si_besoin(&mut self, hauteur: f32) {
        if self.y - hauteur < BAS_PAGE {
            self.pages.push(Vec::new());
            self.y = HAUTEUR_PAGE - MARGE;
        }
    }

    pub fn largeur_utile(&self) -> f32 {
        LARGEUR_PAGE - 2.0 * MARGE
    }

    /// Paragraphe avec retour à la ligne, à partir du décalage `retrait`.
    pub fn paragraphe(&mut self, texte: &str, style: Style, retrait: f32) {
        let interligne = style.taille * 1.35;
        for ligne in couper(texte, style, self.largeur_utile() - retrait) {
            self.saut_si_besoin(interligne);
            self.y -= interligne;
            let y = self.y;
            self.page().push(Op::Texte { x: MARGE + retrait, y, style, contenu: ligne });
        }
    }

    /// Bloc indivisible : passe à la page suivante s'il ne tient pas en bas de page.
    pub fn garder_ensemble(&mut self, hauteur: f32) {
        self.saut_si_besoin(hauteur);
    }

    pub fn espace(&mut self, h: f32) {
        self.y -= h;
    }

    pub fn trait_horizontal(&mut self) {
        self.saut_si_besoin(8.0);
        self.y -= 6.0;
        let y = self.y;
        self.page().push(Op::Trait { y });
        self.y -= 4.0;
    }

    /// Hauteur approximative d'un paragraphe (pour garder un bloc entier).
    pub fn hauteur(&self, texte: &str, style: Style, retrait: f32) -> f32 {
        couper(texte, style, self.largeur_utile() - retrait).len() as f32 * style.taille * 1.35
    }

    #[cfg(test)]
    pub fn nombre_pages(&self) -> usize {
        self.pages.len()
    }

    /// Sérialisation PDF 1.4 (table xref exacte).
    pub fn produire(self) -> Vec<u8> {
        let n = self.pages.len();
        // Objets : 1 catalogue, 2 arbre des pages, 3-4 polices, puis (page, contenu) par page.
        let mut objets: Vec<Vec<u8>> = Vec::new();
        objets.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
        let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 5 + 2 * i)).collect();
        objets.push(format!("<< /Type /Pages /Count {} /Kids [{}] >>", n, kids.join(" ")).into_bytes());
        objets.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec());
        objets.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>".to_vec());
        for (i, ops) in self.pages.iter().enumerate() {
            let mut flux: Vec<u8> = Vec::new();
            for op in ops {
                match op {
                    Op::Texte { x, y, style, contenu } => {
                        let police = if style.police == Police::Grasse { "F2" } else { "F1" };
                        flux.extend_from_slice(format!("BT /{} {:.1} Tf {:.3} g {:.2} {:.2} Td (", police, style.taille, style.gris, x, y).as_bytes());
                        for b in encoder(contenu) {
                            if b == b'(' || b == b')' || b == b'\\' {
                                flux.push(b'\\');
                            }
                            flux.push(b);
                        }
                        flux.extend_from_slice(b") Tj ET\n");
                    }
                    Op::Trait { y } => {
                        flux.extend_from_slice(format!("0.85 G 0.6 w {:.2} {:.2} m {:.2} {:.2} l S\n", MARGE, y, LARGEUR_PAGE - MARGE, y).as_bytes());
                    }
                }
            }
            let pied = format!("{} - page {} sur {}", self.pied, i + 1, n);
            let x = LARGEUR_PAGE - MARGE - largeur(&pied, Police::Normale, 8.0);
            flux.extend_from_slice(format!("BT /F1 8.0 Tf 0.55 g {:.2} {:.2} Td (", x, 34.0).as_bytes());
            for b in encoder(&pied) {
                if b == b'(' || b == b')' || b == b'\\' {
                    flux.push(b'\\');
                }
                flux.push(b);
            }
            flux.extend_from_slice(b") Tj ET\n");
            objets.push(
                format!(
                    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {} 0 R >>",
                    LARGEUR_PAGE as u32,
                    HAUTEUR_PAGE as u32,
                    6 + 2 * i
                )
                .into_bytes(),
            );
            let mut contenu = format!("<< /Length {} >>\nstream\n", flux.len()).into_bytes();
            contenu.extend_from_slice(&flux);
            contenu.extend_from_slice(b"\nendstream");
            objets.push(contenu);
        }

        let mut sortie: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
        let mut positions = Vec::with_capacity(objets.len());
        for (i, obj) in objets.iter().enumerate() {
            positions.push(sortie.len());
            sortie.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
            sortie.extend_from_slice(obj);
            sortie.extend_from_slice(b"\nendobj\n");
        }
        let debut_xref = sortie.len();
        sortie.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objets.len() + 1).as_bytes());
        for p in positions {
            sortie.extend_from_slice(format!("{:010} 00000 n \n", p).as_bytes());
        }
        sortie.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n", objets.len() + 1, debut_xref).as_bytes());
        sortie
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodage_winansi() {
        assert_eq!(encoder("é€œ « »"), vec![0xE9, 0x80, 0x9C, b' ', 0xAB, b' ', 0xBB]);
        assert_eq!(encoder("a→b ≤ ✓"), b"a->b <= ?".to_vec());
    }

    #[test]
    fn largeurs_et_coupure() {
        assert!((largeur("Helvetica", Police::Normale, 10.0) - 41.12).abs() < 0.01);
        assert!(largeur("é", Police::Normale, 10.0) == largeur("e", Police::Normale, 10.0));
        let lignes = couper(&"mot ".repeat(200), TEXTE, 200.0);
        assert!(lignes.len() > 5 && lignes.iter().all(|l| largeur(l, Police::Normale, 9.5) <= 200.0));
        let long = couper(&"x".repeat(500), TEXTE, 100.0);
        assert!(long.len() > 1 && long.iter().all(|l| largeur(l, Police::Normale, 9.5) <= 100.0));
    }

    #[test]
    fn structure_pdf_et_table_xref() {
        let mut doc = Document::nouveau("Journal");
        doc.paragraphe("Journal d'activité (test)", TITRE, 0.0);
        for i in 0..120 {
            doc.paragraphe(&format!("Entrée {} : échéance « signification » \\ 100 €", i), TEXTE, 0.0);
        }
        assert!(doc.nombre_pages() >= 2);
        let pdf = doc.produire();
        assert!(pdf.starts_with(b"%PDF-1.4"));
        let txt = String::from_utf8_lossy(&pdf).to_string();
        assert!(txt.contains("/Count ") && txt.ends_with("%%EOF\n"));
        // Chaque entrée de la table xref pointe exactement sur « n 0 obj ».
        let debut = txt.rfind("startxref\n").unwrap();
        let pos_xref: usize = txt[debut + 10..].lines().next().unwrap().parse().unwrap();
        let table: Vec<&str> = std::str::from_utf8(&pdf[pos_xref..]).unwrap_or("").lines().skip(3).take_while(|l| l.ends_with(" n ")).collect();
        assert!(!table.is_empty());
        for (i, l) in table.iter().enumerate() {
            let p: usize = l[..10].parse().unwrap();
            assert!(pdf[p..].starts_with(format!("{} 0 obj", i + 1).as_bytes()), "objet {}", i + 1);
        }
        // Parenthèses et barres obliques inverses échappées, accents en WinAnsi.
        assert!(pdf.windows(6).any(|w| w == b"\\(test"));
        assert!(pdf.windows(3).any(|w| w == b"\\\\ "));
        assert!(pdf.windows(2).any(|w| w == [b'\xE9', b'c']));
    }
}
