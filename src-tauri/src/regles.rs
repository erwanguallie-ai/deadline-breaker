//! Catalogue unique des délais procéduraux (source de vérité partagée avec l'interface).
//! Textes vérifiés sur Légifrance (versions en vigueur au 18/11/2026).

use crate::langue::t;

use crate::cpc_engine::Duree;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct RegleDelai {
    pub code: &'static str,
    pub libelle: &'static str,
    pub categorie: &'static str,
    /// "CALCULE" (durée fixe), "LIBRE" (durée saisie) ou "FIXE" (date fixée par la juridiction).
    pub mode: &'static str,
    pub duree: Duree,
    pub fondement: &'static str,
    pub article_code: &'static str,
    pub article_num: &'static str,
    pub sanction: &'static str,
    pub point_de_depart: &'static str,
    /// Augmentation de distance de l'art. 643 CPC (juridiction siégeant en métropole).
    pub distance_643: bool,
    /// Augmentation de distance de l'art. 644 CPC (juridiction siégeant outre-mer).
    pub distance_644: bool,
    /// Pourvoi : porté devant la Cour de cassation, la distance s'apprécie selon l'art. 643.
    pub pourvoi: bool,
    pub note: &'static str,
}

pub const CODE_ECHEANCE_FIXEE: &str = "ECHEANCE_FIXEE";
pub const CODE_DELAI_LIBRE: &str = "DELAI_LIBRE";

const fn m(n: u32) -> Duree {
    Duree { annees: 0, mois: n, jours: 0 }
}
const fn j(n: u32) -> Duree {
    Duree { annees: 0, mois: 0, jours: n }
}
const AUCUNE: Duree = Duree { annees: 0, mois: 0, jours: 0 };
fn note_distance_appel() -> &'static str {
    t("Augmentation de distance non appliquée automatiquement : vérifier les dispositions spéciales applicables à l'instance d'appel.")
}

#[allow(clippy::too_many_arguments)]
fn r(
    code: &'static str,
    libelle: &'static str,
    categorie: &'static str,
    duree: Duree,
    fondement: &'static str,
    article_code: &'static str,
    article_num: &'static str,
    sanction: &'static str,
    point_de_depart: &'static str,
    distance: bool,
    note: &'static str,
) -> RegleDelai {
    RegleDelai {
        code,
        libelle,
        categorie,
        mode: "CALCULE",
        duree,
        fondement,
        article_code,
        article_num,
        sanction,
        point_de_depart,
        distance_643: distance,
        distance_644: distance,
        pourvoi: false,
        note,
    }
}

pub fn catalogue() -> Vec<RegleDelai> {
    vec![
        r("DELAI_APPEL_CONTENTIEUX", t("Appel - matière contentieuse (1 mois)"), t("Voies de recours"), m(1), t("Art. 538 du CPC"), "CPC", "538",
          t("Irrecevabilité de l'appel formé hors délai"), t("Notification / signification du jugement"), true, ""),
        r("DELAI_APPEL_GRACIEUX", t("Appel - matière gracieuse (15 jours)"), t("Voies de recours"), j(15), t("Art. 538 du CPC"), "CPC", "538",
          t("Irrecevabilité de l'appel formé hors délai"), t("Notification de la décision"), true, ""),
        r("DELAI_APPEL_REFERE", t("Appel d'une ordonnance de référé (15 jours)"), t("Voies de recours"), j(15), t("Art. 490 du CPC"), "CPC", "490",
          t("Irrecevabilité de l'appel formé hors délai"), t("Signification de l'ordonnance"), true,
          t("Pas d'appel si l'ordonnance émane du premier président ou a été rendue en dernier ressort (art. 490 CPC).")),
        r("DELAI_APPEL_JEX", t("Appel d'une décision du juge de l'exécution (15 jours)"), t("Voies de recours"), j(15), t("Art. R. 121-20 du CPCE"), "CPCE", "R121-20",
          t("Irrecevabilité de l'appel formé hors délai"), t("Notification de la décision"), true,
          t("Appel formé, instruit et jugé selon la procédure à bref délai ou à jour fixe (art. R. 121-20 CPCE).")),
        r("DELAI_APPEL_PRUDHOMMES", t("Appel d'un jugement prud'homal (1 mois)"), t("Voies de recours"), m(1), t("Art. R. 1461-1 du Code du travail"), "CTRAV", "R1461-1",
          t("Irrecevabilité de l'appel formé hors délai"), t("Notification du jugement"), true, ""),
        r("DELAI_OPPOSITION", t("Opposition - jugement par défaut (1 mois)"), t("Voies de recours"), m(1), t("Art. 538 du CPC (délai des voies ordinaires de recours)"), "CPC", "538",
          t("Irrecevabilité de l'opposition formée hors délai"), t("Notification du jugement rendu par défaut"), true,
          t("L'opposition doit contenir les moyens du défaillant (art. 574 CPC).")),
        RegleDelai {
            code: "DELAI_POURVOI_CASSATION",
            libelle: t("Pourvoi en cassation (2 mois)"),
            categorie: t("Voies de recours"),
            mode: "CALCULE",
            duree: m(2),
            fondement: t("Art. 612 du CPC"),
            article_code: "CPC",
            article_num: "612",
            sanction: t("Irrecevabilité du pourvoi formé hors délai"),
            point_de_depart: t("Signification de la décision"),
            distance_643: true,
            distance_644: false,
            pourvoi: true,
            note: t("Sauf disposition contraire (art. 612 CPC)."),
        },
        r("DELAI_CONSTITUTION_AVOCAT_TJ", t("Constitution d'avocat en défense - TJ (15 jours)"), t("Première instance"), j(15), t("Art. 763 du CPC"), "CPC", "763",
          t("Défaut de comparution : il est néanmoins statué sur le fond (art. 472 CPC)"), t("Assignation"), true,
          t("Si l'assignation est délivrée 15 jours ou moins avant l'audience, constitution possible jusqu'à l'audience (art. 763 al. 2).")),
        r("DELAI_CONCLUSIONS_APPELANT_908", t("Conclusions de l'appelant - mise en état (3 mois)"), t("Appel - procédure avec mise en état"), m(3), t("Art. 908 du CPC"), "CPC", "908",
          t("Caducité de la déclaration d'appel, relevée d'office"), t("Déclaration d'appel"), false, note_distance_appel()),
        r("DELAI_CONCLUSIONS_INTIME_909", t("Conclusions de l'intimé - mise en état (3 mois)"), t("Appel - procédure avec mise en état"), m(3), t("Art. 909 du CPC"), "CPC", "909",
          t("Irrecevabilité des conclusions de l'intimé, relevée d'office"), t("Notification à l'intimé des conclusions de l'appelant"), false, note_distance_appel()),
        r("DELAI_CONCLUSIONS_APPELANT_906_2", t("Conclusions de l'appelant - bref délai (2 mois)"), t("Appel - procédure à bref délai"), m(2), t("Art. 906-2 du CPC"), "CPC", "906-2",
          t("Caducité de la déclaration d'appel, relevée d'office"), t("Réception de l'avis de fixation à bref délai"), false,
          t("Le président de la chambre peut allonger ou réduire ce délai (art. 906-2 CPC).")),
        r("DELAI_CONCLUSIONS_INTIME_906_2", t("Conclusions de l'intimé - bref délai (2 mois)"), t("Appel - procédure à bref délai"), m(2), t("Art. 906-2 du CPC"), "CPC", "906-2",
          t("Irrecevabilité des conclusions de l'intimé, relevée d'office"), t("Notification des conclusions de l'appelant"), false,
          t("Le président de la chambre peut allonger ou réduire ce délai (art. 906-2 CPC).")),
        RegleDelai {
            code: CODE_DELAI_LIBRE,
            libelle: t("Délai libre (durée à saisir : années, mois, jours)"),
            categorie: t("Autres délais"),
            mode: "LIBRE",
            duree: AUCUNE,
            fondement: t("Fondement à préciser"),
            article_code: "",
            article_num: "",
            sanction: t("Sanction à préciser"),
            point_de_depart: t("Date saisie"),
            distance_643: false,
            distance_644: false,
            pourvoi: false,
            note: t("Calcul selon les art. 640 à 642 CPC ; l'augmentation de distance n'est appliquée que si vous la cochez."),
        },
        RegleDelai {
            code: CODE_ECHEANCE_FIXEE,
            libelle: t("Échéance fixée (audience, clôture, calendrier du juge)"),
            categorie: t("Autres délais"),
            mode: "FIXE",
            duree: AUCUNE,
            fondement: t("Date fixée par la juridiction"),
            article_code: "",
            article_num: "",
            sanction: t("Selon la décision ou le calendrier de procédure"),
            point_de_depart: t("Date saisie manuellement"),
            distance_643: false,
            distance_644: false,
            pourvoi: false,
            note: t("Aucun calcul : la date saisie est l'échéance."),
        },
    ]
}

/// Accepte les anciens codes de l'interface (APPEL_1M, CONCL_908, ...).
pub fn normaliser_code(code: &str) -> String {
    let c = code.trim().to_uppercase();
    let mappe = match c.as_str() {
        "APPEL_1M" => "DELAI_APPEL_CONTENTIEUX",
        "APPEL_15J" => "DELAI_APPEL_GRACIEUX",
        "OPPOSITION_1M" => "DELAI_OPPOSITION",
        "POURVOI_2M" => "DELAI_POURVOI_CASSATION",
        "CONST_15J" => "DELAI_CONSTITUTION_AVOCAT_TJ",
        "CONCL_908" => "DELAI_CONCLUSIONS_APPELANT_908",
        "CONCL_909" => "DELAI_CONCLUSIONS_INTIME_909",
        autre => autre,
    };
    mappe.to_string()
}

pub fn trouver(code: &str) -> Option<RegleDelai> {
    let c = normaliser_code(code);
    catalogue().into_iter().find(|r| r.code == c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_coherent() {
        let c = catalogue();
        let mut codes: Vec<&str> = c.iter().map(|r| r.code).collect();
        codes.sort();
        codes.dedup();
        assert_eq!(codes.len(), c.len(), "codes de délai en double");
        for r in &c {
            match r.mode {
                "CALCULE" => assert!(!r.duree.est_nulle(), "{} sans durée", r.code),
                "LIBRE" | "FIXE" => assert!(r.duree.est_nulle()),
                autre => panic!("mode inconnu {}", autre),
            }
        }
        assert_eq!(trouver("CONCL_908").map(|r| r.code), Some("DELAI_CONCLUSIONS_APPELANT_908"));
        assert!(trouver("INCONNU").is_none());
        if std::env::var("LEX_EXPORT_CATALOGUE").is_ok() {
            if let Ok(l) = std::env::var("LEX_LANGUE") {
                crate::langue::definir(&l);
            }
            let c = catalogue();
            println!("CATALOGUE_JSON={}", serde_json::to_string(&c).unwrap());
        }
    }
}
