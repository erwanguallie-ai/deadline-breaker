//! Extraction locale du texte d'une pièce déposée (PDF ou texte), avant anonymisation et
//! envoi à l'IA : le fichier lui-même ne quitte jamais le poste.

use crate::langue::t;
use serde::Serialize;

const TAILLE_MAX: usize = 25 * 1024 * 1024;

#[derive(Debug, Serialize)]
pub struct DocumentExtrait {
    pub texte: String,
    pub pages: usize,
    pub caracteres: usize,
}

fn nettoyer(texte: &str) -> String {
    let mut sortie = String::new();
    let mut vides = 0;
    for ligne in texte.replace('\u{feff}', "").replace('\r', "").replace('\u{c}', "\n").lines() {
        let l = ligne.trim_end();
        if l.trim().is_empty() {
            vides += 1;
            if vides > 1 {
                continue;
            }
        } else {
            vides = 0;
        }
        sortie.push_str(l);
        sortie.push('\n');
    }
    sortie.trim().to_string()
}

fn compter_pages(pdf: &[u8]) -> usize {
    let motif = b"/Type /Page";
    let compact = b"/Type/Page";
    let mut n = 0;
    for (i, _) in pdf.iter().enumerate() {
        for m in [&motif[..], &compact[..]] {
            if pdf[i..].starts_with(m) && pdf.get(i + m.len()) != Some(&b's') {
                n += 1;
            }
        }
    }
    n.max(1)
}

pub fn extraire(nom: &str, octets: &[u8]) -> Result<DocumentExtrait, String> {
    if octets.is_empty() {
        return Err(t("Fichier vide.").to_string());
    }
    if octets.len() > TAILLE_MAX {
        return Err(t("Fichier trop volumineux (25 Mo au maximum).").to_string());
    }
    let bas = nom.to_lowercase();
    let (brut, pages) = if bas.ends_with(".pdf") || octets.starts_with(b"%PDF") {
        let resultat = std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem(octets));
        match resultat {
            Ok(Ok(texte)) => (texte, compter_pages(octets)),
            Ok(Err(e)) => return Err(crate::tr!("PDF illisible : {}", "Unreadable PDF: {}", e)),
            Err(_) => return Err(t("PDF illisible (structure non prise en charge).").to_string()),
        }
    } else if bas.ends_with(".txt") || bas.ends_with(".md") || bas.ends_with(".text") {
        (String::from_utf8_lossy(octets).into_owned(), 1)
    } else {
        return Err(t("Format non pris en charge : déposez un PDF ou un fichier texte (.txt).").to_string());
    };
    let texte = nettoyer(&brut);
    if texte.chars().filter(|c| c.is_alphabetic()).count() < 40 {
        return Err(t("Ce PDF ne contient pas de texte exploitable (document scanné ?). Collez le texte, ou utilisez une version du PDF avec reconnaissance de caractères (OCR).").to_string());
    }
    let caracteres = texte.chars().count();
    Ok(DocumentExtrait { texte, pages, caracteres })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf;

    #[test]
    fn extraction_pdf_et_texte() {
        let mut doc = pdf::Document::nouveau("Test");
        doc.paragraphe("ASSIGNATION DEVANT LE TRIBUNAL JUDICIAIRE DE LYON", pdf::TITRE, 0.0);
        doc.paragraphe("La société requérante sollicite le paiement de la somme de 18 400 euros au titre de la facture impayée, outre intérêts.", pdf::TEXTE, 0.0);
        let octets = doc.produire();
        let r = extraire("piece.pdf", &octets).unwrap();
        assert!(r.texte.contains("TRIBUNAL JUDICIAIRE DE LYON"), "{}", r.texte);
        assert!(r.texte.contains("société requérante"), "accents conservés : {}", r.texte);
        assert_eq!(r.pages, 1);
        let txt = extraire("note.txt", "\u{feff}Conclusions en défense : la demande est prescrite depuis le 3 mars 2025, art. 2224 du Code civil.".as_bytes()).unwrap();
        assert!(txt.texte.starts_with("Conclusions"));
        assert!(extraire("scan.pdf", &pdf::Document::nouveau("x").produire()).unwrap_err().contains("scanné"));
        assert!(extraire("image.png", b"\x89PNG....").unwrap_err().contains("Format"));
        assert!(extraire("abime.pdf", b"%PDF-1.4 n'importe quoi").is_err());
    }
}
