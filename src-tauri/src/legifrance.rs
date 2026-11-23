//! Références Légifrance : codes pris en charge, liens vers legifrance.gouv.fr et cache local
//! des articles consultés (via OpenLegi), pour un usage hors-ligne.


use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Codes reconnus : (code interne, identifiant LEGITEXT, intitulé exact attendu par OpenLegi).
pub const CODES: [(&str, &str, &str); 6] = [
    ("CPC", "LEGITEXT000006070716", "Code de procédure civile"),
    ("CCIV", "LEGITEXT000006070721", "Code civil"),
    ("COJ", "LEGITEXT000006071164", "Code de l'organisation judiciaire"),
    ("CTRAV", "LEGITEXT000006072050", "Code du travail"),
    ("CPCE", "LEGITEXT000025024948", "Code des procédures civiles d'exécution"),
    ("CCONSO", "LEGITEXT000006069565", "Code de la consommation"),
];

pub fn code_connu(code: &str) -> Option<(&'static str, &'static str, &'static str)> {
    CODES.iter().copied().find(|(c, _, _)| *c == code)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ArticleConsulte {
    pub code: String,
    pub code_libelle: String,
    pub num: String,
    pub texte: String,
    pub etat: String,
    pub date_debut: Option<String>,
    pub id: String,
    /// "OPENLEGI" (consultation en direct) ou "CACHE" (copie locale d'une consultation antérieure).
    pub source: String,
    pub url: String,
    pub recupere_le: String,
    pub avertissement: Option<String>,
}

/// Clé de cache normalisée : « CPC_641 », « CTRAV_R1461-1 ».
pub fn cle(code: &str, num: &str) -> String {
    format!("{}_{}", code.trim().to_uppercase(), normaliser_num(num))
}

/// « R. 1461-1 » -> « R1461-1 » ; « 641 » inchangé.
pub fn normaliser_num(num: &str) -> String {
    num.chars().filter(|c| !c.is_whitespace() && *c != '.').collect::<String>().to_uppercase()
}

/// Seules les adresses du site officiel peuvent être ouvertes depuis l'application.
pub fn url_autorisee(url: &str) -> bool {
    url.starts_with("https://www.legifrance.gouv.fr/")
        && url.len() < 300
        && url.chars().all(|c| c.is_ascii_alphanumeric() || "/:._-?=&%".contains(c))
}

/// Page du code entier sur Légifrance (repli lorsque l'identifiant de l'article est inconnu).
pub fn url_code(code: &str) -> Option<String> {
    code_connu(code).map(|(_, legitext, _)| format!("https://www.legifrance.gouv.fr/codes/texte_lc/{}", legitext))
}

pub fn lire_cache(fichier: &Path) -> HashMap<String, ArticleConsulte> {
    std::fs::read_to_string(fichier)
        .ok()
        .and_then(|t| serde_json::from_str(t.trim_start_matches('\u{feff}')).ok())
        .unwrap_or_default()
}

pub fn ecrire_cache(fichier: &Path, cache: &HashMap<String, ArticleConsulte>) {
    if let Ok(json) = serde_json::to_string_pretty(cache) {
        let tmp = fichier.with_extension("json.tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, fichier);
        }
    }
}

/// Copie en cache encore « fraîche » (moins de `jours` jours).
pub fn cache_frais(a: &ArticleConsulte, jours: i64) -> bool {
    chrono::NaiveDate::parse_from_str(&a.recupere_le.chars().take(10).collect::<String>(), "%Y-%m-%d")
        .map(|d| (Local::now().date_naive() - d).num_days() < jours)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeros_et_codes() {
        assert_eq!(normaliser_num("R. 1461-1"), "R1461-1");
        assert_eq!(cle("ctrav", "R. 1461-1"), "CTRAV_R1461-1");
        assert!(code_connu("CPC").is_some() && code_connu("XYZ").is_none());
        assert_eq!(url_code("CCIV").unwrap(), "https://www.legifrance.gouv.fr/codes/texte_lc/LEGITEXT000006070721");
    }

    #[test]
    fn liens_autorises() {
        assert!(url_autorisee("https://www.legifrance.gouv.fr/codes/article_lc/LEGIARTI000006411002"));
        assert!(!url_autorisee("https://exemple.com/"));
        assert!(!url_autorisee("https://www.legifrance.gouv.fr/\" & calc"));
        assert!(!url_autorisee("http://www.legifrance.gouv.fr/x"));
    }

    #[test]
    fn fraicheur_du_cache() {
        let mut a = ArticleConsulte { recupere_le: Local::now().format("%Y-%m-%d %H:%M").to_string(), ..Default::default() };
        assert!(cache_frais(&a, 7));
        a.recupere_le = "2000-01-01 10:00".into();
        assert!(!cache_frais(&a, 7));
    }
}
