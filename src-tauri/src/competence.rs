//! Orientation de compétence matérielle et territoriale, taux du ressort.
//! Outil d'aide : les compétences exclusives et textes spéciaux restent à vérifier.

use crate::langue::t;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct Matiere {
    pub code: &'static str,
    pub libelle: &'static str,
}

pub fn matieres() -> Vec<Matiere> {
    vec![
        Matiere { code: "CIVIL_GENERAL", libelle: t("Civil général / créance") },
        Matiere { code: "BAUX_HABITATION", libelle: t("Bail d'habitation / occupation d'un logement") },
        Matiere { code: "CREDIT_CONSO", libelle: t("Crédit à la consommation") },
        Matiere { code: "EXECUTION", libelle: t("Exécution forcée / mesures conservatoires / saisie immobilière") },
        Matiere { code: "COMMERCE", libelle: t("Litige entre commerçants / sociétés commerciales") },
        Matiere { code: "TRAVAIL", libelle: t("Contrat de travail de droit privé") },
    ]
}

/// Référence cliquable vers un texte (code + numéro d'article).
#[derive(Debug, Clone, Serialize)]
pub struct Reference {
    pub libelle: String,
    pub code: &'static str,
    pub article: &'static str,
}

fn reference(libelle: &str, code: &'static str, article: &'static str) -> Reference {
    Reference { libelle: libelle.to_string(), code, article }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Demande {
    pub matiere: String,
    pub montant: Option<f64>,
    /// "DEFENDEUR", "LIVRAISON", "DOMMAGE", "IMMEUBLE" ou "SUCCESSION".
    pub regle_territoriale: String,
    pub ville: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Resultat {
    pub juridiction: String,
    pub materielle: String,
    pub reference_materielle: Option<Reference>,
    pub territoriale: String,
    pub ressort: String,
    pub reference_ressort: Option<Reference>,
    /// Représentation par avocat (obligatoire ou non).
    pub representation: String,
    pub reference_representation: Option<Reference>,
    /// Voie de recours contre la décision à venir et code du délai dans le catalogue.
    pub recours: String,
    pub code_delai_recours: Option<&'static str>,
    pub observations: Vec<String>,
    pub references: Vec<Reference>,
}

pub fn orienter(dm: &Demande) -> Result<Resultat, String> {
    if let Some(m) = dm.montant {
        if !m.is_finite() || m < 0.0 {
            return Err(t("Montant invalide.").into());
        }
    }
    let mut r = Resultat::default();
    match dm.matiere.as_str() {
        "CIVIL_GENERAL" => {
            r.juridiction = t("Tribunal judiciaire").into();
            r.materielle = t("Compétence de droit commun en matière civile (art. L. 211-3 COJ).").into();
        }
        "BAUX_HABITATION" => {
            r.juridiction = t("Juge des contentieux de la protection (tribunal judiciaire)").into();
            r.materielle = t("Actions relatives au louage d'immeubles à usage d'habitation ou à l'occupation d'un logement (art. L. 213-4-4 COJ).").into();
            r.reference_materielle = Some(reference("art. L. 213-4-4 COJ", "COJ", "L213-4-4"));
        }
        "CREDIT_CONSO" => {
            r.juridiction = t("Juge des contentieux de la protection (tribunal judiciaire)").into();
            r.materielle = t("Actions relatives au crédit à la consommation (art. L. 213-4-5 COJ).").into();
            r.reference_materielle = Some(reference("art. L. 213-4-5 COJ", "COJ", "L213-4-5"));
        }
        "EXECUTION" => {
            r.juridiction = t("Juge de l'exécution (tribunal judiciaire)").into();
            r.materielle = t("Compétence exclusive : difficultés relatives aux titres exécutoires, contestations de l'exécution forcée, mesures conservatoires, saisie immobilière (art. L. 213-6 COJ).").into();
            r.reference_materielle = Some(reference("art. L. 213-6 COJ", "COJ", "L213-6"));

        }
        "COMMERCE" => {
            r.juridiction = t("Tribunal de commerce").into();
            r.materielle = t("Contestations entre commerçants, entre sociétés commerciales, relatives aux actes de commerce (art. L. 721-3 C. com.).").into();
            r.observations.push(t("Vérifier si le ressort relève d'un tribunal des activités économiques (expérimentation en cours).").into());
        }
        "TRAVAIL" => {
            r.juridiction = t("Conseil de prud'hommes").into();
            r.materielle = t("Différends nés à l'occasion du contrat de travail (art. L. 1411-1 C. trav.).").into();
            r.observations.push(t("Compétence territoriale spéciale : art. R. 1412-1 C. trav.").into());

        }
        autre => return Err(crate::tr!("Matière inconnue : {}", "Unknown subject matter: {}", autre)),
    }

    r.territoriale = match (dm.matiere.as_str(), dm.regle_territoriale.as_str()) {
        ("TRAVAIL", _) => t("Règles spéciales du Code du travail (art. R. 1412-1 C. trav.).").into(),
        (_, "IMMEUBLE") => t("Art. 44 CPC : en matière réelle immobilière, la juridiction du lieu où est situé l'immeuble est seule compétente.").into(),
        (_, "SUCCESSION") => t("Art. 45 CPC : juridiction du lieu d'ouverture de la succession (demandes entre héritiers, des créanciers du défunt, exécution des dispositions à cause de mort), jusqu'au partage inclusivement.").into(),
        (_, "LIVRAISON") => t("Art. 46 CPC (option du demandeur) : lieu de la livraison effective de la chose ou de l'exécution de la prestation de service.").into(),
        (_, "DOMMAGE") => t("Art. 46 CPC (option du demandeur) : lieu du fait dommageable ou lieu dans le ressort duquel le dommage a été subi.").into(),
        _ => t("Art. 42 CPC : juridiction du lieu où demeure le défendeur (domicile ou résidence ; lieu d'établissement d'une personne morale, art. 43). Plusieurs défendeurs : au choix du demandeur.").into(),
    };

    // Représentation par avocat
    let m = dm.montant;
    match dm.matiere.as_str() {
        "BAUX_HABITATION" | "CREDIT_CONSO" => {
            r.representation = t("Non obligatoire devant le juge des contentieux de la protection (art. 761 1° CPC).").into();
            r.reference_representation = Some(reference("art. 761 CPC", "CPC", "761"));
        }
        "CIVIL_GENERAL" => {
            r.representation = match m {
                Some(v) if v <= 10_000.0 => t("Non obligatoire : demande ≤ 10 000 €, hors matières de compétence exclusive du tribunal judiciaire (art. 761 3° CPC).").into(),
                Some(_) => t("Obligatoire : demande supérieure à 10 000 € devant le tribunal judiciaire (sauf dispense prévue par un texte, art. 761 CPC).").into(),
                None => t("Obligatoire au-delà de 10 000 € ; dispense possible en deçà (art. 761 3° CPC). Préciser le montant.").into(),
            };
            r.reference_representation = Some(reference("art. 761 CPC", "CPC", "761"));
        }
        "COMMERCE" => {
            r.representation = match m {
                Some(v) if v <= 10_000.0 => t("Non obligatoire : demande ≤ 10 000 € (art. 853 CPC) ; représentation possible par toute personne munie d'un pouvoir spécial.").into(),
                Some(_) => t("Obligatoire : constitution d'avocat devant le tribunal de commerce au-delà de 10 000 € (art. 853 CPC).").into(),
                None => t("Obligatoire sauf demande ≤ 10 000 € et autres cas de dispense (art. 853 CPC). Préciser le montant.").into(),
            };
            r.reference_representation = Some(reference("art. 853 CPC", "CPC", "853"));
        }
        "TRAVAIL" => {
            r.representation = t("Non obligatoire en première instance : les parties se défendent elles-mêmes et peuvent se faire assister ou représenter (art. R. 1453-1 C. trav.). En appel : avocat ou défenseur syndical (art. R. 1461-1).").into();
            r.reference_representation = Some(reference("art. R. 1453-1 C. trav.", "CTRAV", "R1453-1"));
        }
        _ => {
            r.representation = t("Règles propres au juge de l'exécution : vérifier le code des procédures civiles d'exécution selon la mesure en cause.").into();
        }
    }

    // Voie de recours contre la décision
    match dm.matiere.as_str() {
        "TRAVAIL" => {
            r.recours = t("Appel dans le délai d'un mois (art. R. 1461-1 C. trav.), sous réserve du taux du ressort.").into();
            r.code_delai_recours = Some("DELAI_APPEL_PRUDHOMMES");
        }
        "EXECUTION" => {
            r.recours = t("Appel dans les quinze jours de la notification (art. R. 121-20 CPCE).").into();
            r.code_delai_recours = Some("DELAI_APPEL_JEX");
        }
        "COMMERCE" => {
            r.recours = t("Appel dans le délai d'un mois (art. 538 CPC), sous réserve du taux du ressort du tribunal de commerce.").into();
            r.code_delai_recours = Some("DELAI_APPEL_CONTENTIEUX");
        }
        _ => match m {
            Some(v) if v <= 5_000.0 => {
                r.recours = t("Décision en dernier ressort : pas d'appel, pourvoi en cassation dans les deux mois (art. 612 CPC).").into();
                r.code_delai_recours = Some("DELAI_POURVOI_CASSATION");
            }
            Some(_) => {
                r.recours = t("Appel dans le délai d'un mois en matière contentieuse (art. 538 CPC).").into();
                r.code_delai_recours = Some("DELAI_APPEL_CONTENTIEUX");
            }
            None => {
                r.recours = t("Appel (1 mois, art. 538 CPC) ou, si la décision est en dernier ressort, pourvoi (2 mois, art. 612 CPC). Préciser le montant.").into();
            }
        },
    }

    match (dm.matiere.as_str(), dm.montant) {
        ("TRAVAIL", _) | ("COMMERCE", _) => r.ressort = t("Taux du ressort propre à cette juridiction : à vérifier (textes spéciaux).").into(),
        (_, None) => r.ressort = t("Montant à préciser pour déterminer le taux du ressort.").into(),
        (_, Some(m)) if m <= 5_000.0 => {
            r.ressort = crate::tr!("Demande de {} € : en dernier ressort (pourvoi en cassation seul) - art. R. 211-3-24 COJ.", "Claim of €{}: decided in last resort (cassation appeal only) - art. R. 211-3-24 COJ.", montant_fr(m));
            r.reference_ressort = Some(reference("art. R. 211-3-24 COJ", "COJ", "R211-3-24"));
        }
        (_, Some(m)) => {
            r.ressort = crate::tr!("Demande de {} € : à charge d'appel (délai d'un mois - art. 538 CPC).", "Claim of €{}: open to appeal (one-month time limit - art. 538 CPC).", montant_fr(m));
            r.reference_ressort = Some(reference("art. 538 CPC", "CPC", "538"));
        }
    }

    let ville = dm.ville.trim();
    if !ville.is_empty() {
        r.juridiction = crate::tr!("{} de {}", "{} of {}", r.juridiction, ville);
    }
    Ok(r)
}

fn montant_fr(m: f64) -> String {
    let entier = m.trunc() as u64;
    let s = entier.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push('\u{202f}');
        }
        out.push(c);
    }
    let cents = ((m - m.trunc()) * 100.0).round() as u64;
    if cents > 0 {
        out.push_str(&format!(",{:02}", cents));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dm(matiere: &str, montant: Option<f64>) -> Demande {
        Demande { matiere: matiere.into(), montant, regle_territoriale: "DEFENDEUR".into(), ville: "Lyon".into() }
    }

    #[test]
    fn taux_du_ressort() {
        let r = orienter(&dm("CIVIL_GENERAL", Some(4000.0))).unwrap();
        assert!(r.ressort.contains("dernier ressort"));
        assert!(r.representation.contains("761 3°"));
        let r = orienter(&dm("CIVIL_GENERAL", Some(5000.0))).unwrap();
        assert!(r.ressort.contains("dernier ressort"));
        let r = orienter(&dm("CIVIL_GENERAL", Some(12000.0))).unwrap();
        assert!(r.ressort.contains("charge d'appel"));
        assert!(r.representation.starts_with("Obligatoire"));
        assert_eq!(r.juridiction, "Tribunal judiciaire de Lyon");
    }

    #[test]
    fn matieres_speciales() {
        assert!(orienter(&dm("BAUX_HABITATION", Some(3000.0))).unwrap().juridiction.starts_with("Juge des contentieux"));
        assert!(orienter(&dm("CREDIT_CONSO", None)).unwrap().materielle.contains("L. 213-4-5"));
        assert!(orienter(&dm("EXECUTION", None)).unwrap().juridiction.starts_with("Juge de l'exécution"));
        assert!(orienter(&dm("TRAVAIL", Some(1000.0))).unwrap().ressort.contains("à vérifier"));
        assert!(orienter(&dm("INCONNUE", None)).is_err());
        assert!(orienter(&dm("CIVIL_GENERAL", Some(-1.0))).is_err());
    }

    #[test]
    fn representation_et_recours() {
        let r = orienter(&dm("CIVIL_GENERAL", Some(4000.0))).unwrap();
        assert!(r.representation.starts_with("Non obligatoire"));
        assert_eq!(r.code_delai_recours, Some("DELAI_POURVOI_CASSATION"));
        let r = orienter(&dm("CIVIL_GENERAL", Some(15000.0))).unwrap();
        assert!(r.representation.starts_with("Obligatoire"));
        assert_eq!(r.code_delai_recours, Some("DELAI_APPEL_CONTENTIEUX"));
        let r = orienter(&dm("COMMERCE", Some(20000.0))).unwrap();
        assert!(r.representation.contains("853"));
        assert_eq!(orienter(&dm("TRAVAIL", None)).unwrap().code_delai_recours, Some("DELAI_APPEL_PRUDHOMMES"));
        assert_eq!(orienter(&dm("EXECUTION", None)).unwrap().code_delai_recours, Some("DELAI_APPEL_JEX"));
        let mut d = dm("CIVIL_GENERAL", None);
        d.regle_territoriale = "IMMEUBLE".into();
        assert!(orienter(&d).unwrap().territoriale.contains("seule compétente"));
        // Les codes de délai proposés existent dans le catalogue
        for m in ["CIVIL_GENERAL", "COMMERCE", "TRAVAIL", "EXECUTION"] {
            if let Some(c) = orienter(&dm(m, Some(9000.0))).unwrap().code_delai_recours {
                assert!(crate::regles::trouver(c).is_some(), "{}", c);
            }
        }
    }

    #[test]
    fn format_montant() {
        assert_eq!(montant_fr(12000.0), "12\u{202f}000");
        assert_eq!(montant_fr(1234567.5), "1\u{202f}234\u{202f}567,50");
    }
}
