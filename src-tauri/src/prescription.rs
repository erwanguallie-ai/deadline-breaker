//! Simulateur de prescription extinctive et de forclusion (Code civil, titre XX).
//! Interruption : art. 2231, 2241, 2242, 2243. Suspension : art. 2238, 2239.
//! Forclusion : art. 2220. Délai butoir : art. 2232.

use crate::langue::t;

use crate::cpc_engine::{ajouter_mois_cpc, fr};
use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct RegimePrescription {
    pub code: &'static str,
    pub libelle: &'static str,
    pub annees: u32,
    pub fondement: &'static str,
    pub article_code: &'static str,
    pub article_num: &'static str,
    /// Le délai butoir de 20 ans (art. 2232 al. 1) ne s'applique pas (art. 2232 al. 2).
    pub butoir_exclu: bool,
}

pub fn regimes() -> Vec<RegimePrescription> {
    vec![
        RegimePrescription { code: "5_DROIT_COMMUN", libelle: t("Actions personnelles ou mobilières - 5 ans"), annees: 5, fondement: "Art. 2224 C. civ.", article_code: "CCIV", article_num: "2224", butoir_exclu: false },
        RegimePrescription { code: "10_CORPOREL", libelle: t("Dommage corporel - 10 ans à compter de la consolidation"), annees: 10, fondement: "Art. 2226 C. civ.", article_code: "CCIV", article_num: "2226", butoir_exclu: true },
        RegimePrescription { code: "30_REEL_IMMOBILIER", libelle: t("Actions réelles immobilières - 30 ans"), annees: 30, fondement: "Art. 2227 C. civ.", article_code: "CCIV", article_num: "2227", butoir_exclu: true },
        RegimePrescription { code: "2_VICES_CACHES", libelle: t("Garantie des vices cachés - 2 ans à compter de la découverte"), annees: 2, fondement: "Art. 1648 C. civ.", article_code: "CCIV", article_num: "1648", butoir_exclu: false },
        RegimePrescription { code: "2_CONSO", libelle: t("Action des professionnels contre les consommateurs - 2 ans"), annees: 2, fondement: "Art. L. 218-2 C. conso.", article_code: "CCONSO", article_num: "L218-2", butoir_exclu: false },
        RegimePrescription { code: "3_SALAIRE", libelle: t("Paiement ou répétition du salaire - 3 ans"), annees: 3, fondement: "Art. L. 3245-1 C. trav.", article_code: "CTRAV", article_num: "L3245-1", butoir_exclu: false },
    ]
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Evenement {
    /// "DEMANDE_JUSTICE", "REFERE_EXPERTISE" ou "MEDIATION".
    #[serde(rename = "type")]
    pub type_evenement: String,
    pub date_assign: Option<String>,
    pub date_ord: Option<String>,
    pub date_depot: Option<String>,
    pub date_debut: Option<String>,
    pub date_fin: Option<String>,
    pub non_avenue: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Demande {
    pub regime: String,
    /// "PRESCRIPTION" ou "FORCLUSION".
    pub qualification: String,
    pub date_depart: String,
    /// Jour de la naissance du droit (point de départ du délai butoir). Défaut : date de départ.
    pub date_naissance_droit: Option<String>,
    pub date_cible: String,
    pub evenements: Vec<Evenement>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Resultat {
    pub date_expiration: Option<String>,
    pub en_suspens: Option<String>,
    pub acquise: Option<bool>,
    pub marge_jours: Option<i64>,
    pub date_butoir: Option<String>,
    pub verdict: String,
    pub etapes: Vec<String>,
    pub vigilance: Vec<String>,
    pub fondement: String,
    /// Pour la frise : jours écoulés avant le premier événement, jours neutralisés, total.
    pub frise_initial: i64,
    pub frise_neutralise: i64,
    pub frise_total: i64,
}

fn d(s: &Option<String>) -> Option<NaiveDate> {
    s.as_ref().and_then(|x| NaiveDate::parse_from_str(x.trim(), "%Y-%m-%d").ok())
}
fn f(x: NaiveDate) -> String {
    x.format("%d/%m/%Y").to_string()
}
fn premiere_date(e: &Evenement) -> Option<NaiveDate> {
    d(&e.date_assign).or_else(|| d(&e.date_debut))
}

pub fn analyser(demande: &Demande) -> Result<Resultat, String> {
    let regime = regimes()
        .into_iter()
        .find(|r| r.code == demande.regime)
        .ok_or_else(|| crate::tr!("Régime de prescription inconnu : {}", "Unknown limitation regime: {}", demande.regime))?;
    let t0 = NaiveDate::parse_from_str(demande.date_depart.trim(), "%Y-%m-%d")
        .map_err(|_| t("Point de départ : date invalide (AAAA-MM-JJ).").to_string())?;
    let cible = NaiveDate::parse_from_str(demande.date_cible.trim(), "%Y-%m-%d")
        .map_err(|_| t("Date cible : date invalide (AAAA-MM-JJ).").to_string())?;
    let forclusion = demande.qualification.eq_ignore_ascii_case("FORCLUSION");
    let mois = regime.annees * 12;

    let mut r = Resultat { fondement: regime.fondement.to_string(), ..Default::default() };
    let mut n = 1;
    let mut etape = |r: &mut Resultat, texte: String| {
        r.etapes.push(format!("{}. {}", n, texte));
        n += 1;
    };

    let mut fin = ajouter_mois_cpc(t0, mois);
    etape(
        &mut r,
        crate::tr!(
            "Point de départ : {}. {} ({}). Délai qualifié de {}. Expiration initiale : {}.", "Starting point: {}. {} ({}). Period classified as {}. Initial expiry: {}.",
            f(t0),
            regime.libelle,
            regime.fondement,
            if forclusion { t("forclusion") } else { t("prescription") },
            f(fin)
        ),
    );

    let mut evenements: Vec<&Evenement> = demande.evenements.iter().collect();
    evenements.sort_by_key(|e| premiere_date(e).unwrap_or(NaiveDate::MAX));

    let mut en_suspens: Option<String> = None;
    let mut neutralise: i64 = 0;
    let mut interruption_judiciaire = false;
    let mut suspension = false;

    for ev in &evenements {
        match ev.type_evenement.as_str() {
            "DEMANDE_JUSTICE" => {
                let Some(a) = d(&ev.date_assign) else {
                    etape(&mut r, t("Demande en justice : date non renseignée, événement ignoré.").into());
                    continue;
                };
                if a > fin {
                    etape(&mut r, crate::tr!("Demande en justice du {} : postérieure à l'expiration du {}, sans effet interruptif.", "Legal action of {}: after expiry on {}, no interrupting effect.", f(a), f(fin)));
                    continue;
                }
                if ev.non_avenue {
                    etape(&mut r, crate::tr!("Demande en justice du {} : interruption non avenue (désistement, péremption ou rejet définitif - art. 2243 C. civ.).", "Legal action of {}: interruption void (withdrawal, lapse or final dismissal - art. 2243 C. civ.).", f(a)));
                    continue;
                }
                let Some(fi) = d(&ev.date_fin) else {
                    let t = crate::tr!("Instance introduite le {} toujours en cours : l'effet interruptif se poursuit jusqu'à son extinction (art. 2242 C. civ.).", "Proceedings started on {} still pending: the interruption continues until they end (art. 2242 C. civ.).", f(a));
                    etape(&mut r, t.clone());
                    en_suspens = Some(t);
                    break;
                };
                if fi < a {
                    return Err(t("Demande en justice : l'extinction de l'instance précède la demande.").into());
                }
                neutralise += (fi - a).num_days();
                interruption_judiciaire = true;
                fin = ajouter_mois_cpc(fi, mois);
                etape(&mut r, crate::tr!(
                    "Demande en justice du {} : interruption (art. 2241), effets jusqu'à l'extinction de l'instance le {} (art. 2242). Nouveau délai de même durée (art. 2231) : expiration le {}.", "Legal action of {}: interruption (art. 2241), effective until the proceedings ended on {} (art. 2242). New period of the same length (art. 2231): expires on {}.",
                    f(a), f(fi), f(fin)
                ));
            }
            "REFERE_EXPERTISE" => {
                let (Some(a), Some(o)) = (d(&ev.date_assign), d(&ev.date_ord)) else {
                    etape(&mut r, t("Référé-expertise : dates d'assignation et d'ordonnance requises, événement ignoré.").into());
                    continue;
                };
                if a > fin {
                    etape(&mut r, crate::tr!("Assignation en référé du {} : postérieure à l'expiration du {}, sans effet.", "Summary writ of {}: after expiry on {}, no effect.", f(a), f(fin)));
                    continue;
                }
                if o < a {
                    return Err(t("Référé-expertise : l'ordonnance précède l'assignation.").into());
                }
                neutralise += (o - a).num_days();
                interruption_judiciaire = true;
                etape(&mut r, crate::tr!(
                    "Assignation en référé du {} : interruption (art. 2241, « même en référé »), effets jusqu'à l'ordonnance du {} qui met fin à l'instance (art. 2242).", "Summary writ of {}: interruption (art. 2241, \"even in summary proceedings\"), effective until the order of {} ending the proceedings (art. 2242).",
                    f(a), f(o)
                ));
                if forclusion {
                    fin = ajouter_mois_cpc(o, mois);
                    etape(&mut r, crate::tr!(
                        "Délai de forclusion : la suspension de l'art. 2239 n'est pas retenue (art. 2220 C. civ.). Nouveau délai à compter de l'ordonnance : expiration le {}.", "Foreclosure period: the suspension under art. 2239 does not apply (art. 2220 C. civ.). New period from the order: expires on {}.",
                        f(fin)
                    ));
                } else if let Some(depot) = d(&ev.date_depot) {
                    if depot < o {
                        return Err(t("Référé-expertise : l'exécution de la mesure précède l'ordonnance.").into());
                    }
                    neutralise += (depot - o).num_days();
                    suspension = true;
                    let reprise = ajouter_mois_cpc(depot, mois);
                    let plancher = ajouter_mois_cpc(depot, 6);
                    fin = reprise.max(plancher);
                    etape(&mut r, crate::tr!(
                        "Nouveau délai ouvert le {}, aussitôt suspendu jusqu'à l'exécution de la mesure le {} (art. 2239). Il recommence à courir pour sa durée restante, au minimum six mois (art. 2239 al. 2) : expiration le {}.", "New period opened on {}, immediately suspended until the measure was completed on {} (art. 2239). It resumes for its remaining duration, at least six months (art. 2239 para. 2): expires on {}.",
                        f(o), f(depot), f(fin)
                    ));
                } else {
                    let t = crate::tr!("Mesure d'instruction ordonnée le {} non encore exécutée : prescription suspendue (art. 2239 al. 1).", "Investigative measure ordered on {} not yet completed: limitation suspended (art. 2239 para. 1).", f(o));
                    etape(&mut r, t.clone());
                    en_suspens = Some(t);
                    break;
                }
            }
            "MEDIATION" => {
                let Some(debut) = d(&ev.date_debut) else {
                    etape(&mut r, t("Médiation : date de début non renseignée, événement ignoré.").into());
                    continue;
                };
                if forclusion {
                    etape(&mut r, crate::tr!("Médiation du {} : sans effet suspensif sur un délai de forclusion (art. 2220 C. civ.), sauf texte spécial.", "Mediation of {}: no suspensive effect on a foreclosure period (art. 2220 C. civ.), unless a special rule applies.", f(debut)));
                    continue;
                }
                if debut > fin {
                    etape(&mut r, crate::tr!("Médiation du {} : postérieure à l'expiration du {}, sans effet.", "Mediation of {}: after expiry on {}, no effect.", f(debut), f(fin)));
                    continue;
                }
                let Some(fi) = d(&ev.date_fin) else {
                    let t = crate::tr!("Médiation engagée le {} non terminée : prescription suspendue (art. 2238).", "Mediation started on {} not yet ended: limitation suspended (art. 2238).", f(debut));
                    etape(&mut r, t.clone());
                    en_suspens = Some(t);
                    break;
                };
                if fi < debut {
                    return Err(t("Médiation : la date de fin précède la date de début.").into());
                }
                let restant = (fin - debut).num_days();
                neutralise += (fi - debut).num_days();
                suspension = true;
                let reprise = fi + Duration::days(restant);
                let plancher = ajouter_mois_cpc(fi, 6);
                fin = reprise.max(plancher);
                etape(&mut r, crate::tr!(
                    "Médiation du {} au {} : suspension (art. 2238). Durée restante au jour de la suspension : {} j ; reprise pour au moins six mois : expiration le {}.", "Mediation from {} to {}: suspension (art. 2238). Remaining period on the day of suspension: {} days; resumes for at least six months: expires on {}.",
                    f(debut), f(fi), restant, f(fin)
                ));
            }
            autre => return Err(crate::tr!("Type d'événement inconnu : {}", "Unknown event type: {}", autre)),
        }
    }

    // Délai butoir (art. 2232)
    let naissance = d(&demande.date_naissance_droit).unwrap_or(t0);
    if naissance > t0 {
        return Err(t("La naissance du droit ne peut être postérieure au point de départ retenu.").into());
    }
    if !forclusion {
        let butoir = ajouter_mois_cpc(naissance, 240);
        r.date_butoir = Some(butoir.format("%Y-%m-%d").to_string());
        if regime.butoir_exclu {
            r.vigilance.push(crate::tr!("Délai butoir de vingt ans écarté pour ce régime ({} - art. 2232 al. 2).", "Twenty-year long-stop does not apply to this regime ({} - art. 2232 para. 2).", regime.fondement));
        } else if en_suspens.is_none() && fin > butoir {
            if interruption_judiciaire {
                r.vigilance.push(crate::tr!(
                    "L'expiration calculée dépasse le délai butoir du {} : celui-ci est écarté en cas d'interruption par une demande en justice (art. 2232 al. 2, renvoyant à l'art. 2241 al. 1). À vérifier selon la chronologie exacte.", "The computed expiry goes beyond the long-stop date of {}: the long-stop does not apply when interrupted by legal action (art. 2232 para. 2, referring to art. 2241 para. 1). Check against the exact chronology.",
                    f(butoir)
                ));
            } else if suspension || naissance < t0 {
                etape(&mut r, crate::tr!(
                    "Délai butoir : le report du point de départ ou la suspension ne peut porter le délai au-delà de vingt ans à compter de la naissance du droit le {} (art. 2232 al. 1). Expiration ramenée au {}.", "Long-stop: deferring the starting point or suspension cannot extend the period beyond twenty years from when the right arose on {} (art. 2232 para. 1). Expiry brought back to {}.",
                    f(naissance), f(butoir)
                ));
                fin = butoir;
            }
        }
    }

    // Conclusion
    let fin_frise = if let Some(t) = &en_suspens {
        r.en_suspens = Some(t.clone());
        r.verdict = "EN_SUSPENS".into();
        r.etapes.push(crate::tr!("\nCONCLUSION : à ce jour, aucune date d'expiration ne peut être fixée. {}", "\nCONCLUSION: no expiry date can be set at this stage. {}", t));
        cible
    } else {
        let marge = (fin - cible).num_days();
        r.date_expiration = Some(fin.format("%Y-%m-%d").to_string());
        r.marge_jours = Some(marge);
        r.acquise = Some(marge < 0);
        if marge >= 0 {
            r.verdict = "NON_ACQUISE".into();
            r.etapes.push(crate::tr!("\nCONCLUSION : au {}, le délai n'est pas expiré (expiration le {} à minuit).", "\nCONCLUSION: as at {}, the period has not expired (expires on {} at midnight).", f(cible), fr(fin)));
        } else {
            r.verdict = "ACQUISE".into();
            r.etapes.push(crate::tr!(
                "\nCONCLUSION : au {}, le délai a expiré le {}. Fin de non-recevoir susceptible d'être opposée (art. 122 CPC).", "\nCONCLUSION: as at {}, the period expired on {}. A plea of inadmissibility may be raised (art. 122 CPC).",
                f(cible), f(fin)
            ));
        }
        fin.max(cible)
    };

    r.frise_total = (fin_frise - t0).num_days().max(1);
    r.frise_initial = evenements
        .iter()
        .find_map(|e| premiere_date(e))
        .map(|p| (p - t0).num_days().clamp(0, r.frise_total))
        .unwrap_or(r.frise_total);
    r.frise_neutralise = neutralise.min(r.frise_total - r.frise_initial).max(0);
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(t: &str, a: Option<&str>, o: Option<&str>, dep: Option<&str>, deb: Option<&str>, fin: Option<&str>) -> Evenement {
        let s = |x: Option<&str>| x.map(|v| v.to_string());
        Evenement { type_evenement: t.into(), date_assign: s(a), date_ord: s(o), date_depot: s(dep), date_debut: s(deb), date_fin: s(fin), non_avenue: false }
    }
    fn demande(regime: &str, qualif: &str, t0: &str, cible: &str, evs: Vec<Evenement>) -> Demande {
        Demande { regime: regime.into(), qualification: qualif.into(), date_depart: t0.into(), date_naissance_droit: None, date_cible: cible.into(), evenements: evs }
    }

    #[test]
    fn sans_evenement() {
        let r = analyser(&demande("5_DROIT_COMMUN", "PRESCRIPTION", "2020-03-15", "2026-11-18", vec![])).unwrap();
        assert_eq!(r.date_expiration.as_deref(), Some("2025-03-15"));
        assert_eq!(r.verdict, "ACQUISE");
    }

    #[test]
    fn refere_expertise() {
        let r = analyser(&demande("5_DROIT_COMMUN", "PRESCRIPTION", "2018-05-15", "2026-11-18",
            vec![ev("REFERE_EXPERTISE", Some("2020-03-10"), Some("2020-06-25"), Some("2024-01-15"), None, None)])).unwrap();
        assert_eq!(r.date_expiration.as_deref(), Some("2029-01-15"));
        assert_eq!(r.verdict, "NON_ACQUISE");
    }

    #[test]
    fn refere_expertise_forclusion() {
        let r = analyser(&demande("2_VICES_CACHES", "FORCLUSION", "2020-01-10", "2026-11-18",
            vec![ev("REFERE_EXPERTISE", Some("2021-06-01"), Some("2021-09-01"), Some("2023-01-01"), None, None)])).unwrap();
        assert_eq!(r.date_expiration.as_deref(), Some("2023-09-01"));
    }

    #[test]
    fn mediation_plancher_six_mois() {
        let r = analyser(&demande("5_DROIT_COMMUN", "PRESCRIPTION", "2020-01-01", "2025-03-01",
            vec![ev("MEDIATION", None, None, None, Some("2024-12-01"), Some("2025-02-01"))])).unwrap();
        assert_eq!(r.date_expiration.as_deref(), Some("2025-08-01"));
    }

    #[test]
    fn demande_en_justice() {
        let en_cours = analyser(&demande("5_DROIT_COMMUN", "PRESCRIPTION", "2020-01-01", "2026-11-18",
            vec![ev("DEMANDE_JUSTICE", Some("2023-06-01"), None, None, None, None)])).unwrap();
        assert_eq!(en_cours.verdict, "EN_SUSPENS");
        let eteinte = analyser(&demande("5_DROIT_COMMUN", "PRESCRIPTION", "2020-01-01", "2026-11-18",
            vec![ev("DEMANDE_JUSTICE", Some("2023-06-01"), None, None, None, Some("2025-03-01"))])).unwrap();
        assert_eq!(eteinte.date_expiration.as_deref(), Some("2030-03-01"));
        let mut non_avenue = ev("DEMANDE_JUSTICE", Some("2023-06-01"), None, None, None, Some("2025-03-01"));
        non_avenue.non_avenue = true;
        let r = analyser(&demande("5_DROIT_COMMUN", "PRESCRIPTION", "2020-01-01", "2026-11-18", vec![non_avenue])).unwrap();
        assert_eq!(r.date_expiration.as_deref(), Some("2025-01-01"));
    }

    #[test]
    fn demande_tardive_sans_effet() {
        let r = analyser(&demande("2_CONSO", "PRESCRIPTION", "2020-01-01", "2026-11-18",
            vec![ev("DEMANDE_JUSTICE", Some("2023-06-01"), None, None, None, Some("2024-01-01"))])).unwrap();
        assert_eq!(r.date_expiration.as_deref(), Some("2022-01-01"));
        assert_eq!(r.verdict, "ACQUISE");
    }

    #[test]
    fn butoir_vingt_ans() {
        // Naissance du droit en 2000, connaissance des faits en 2019 : 2019 + 5 = 2024 > 2020 (butoir)
        let mut dm = demande("5_DROIT_COMMUN", "PRESCRIPTION", "2019-06-01", "2021-01-01", vec![]);
        dm.date_naissance_droit = Some("2000-03-01".into());
        let r = analyser(&dm).unwrap();
        assert_eq!(r.date_expiration.as_deref(), Some("2020-03-01"));
        assert_eq!(r.verdict, "ACQUISE");
        // Dommage corporel : butoir écarté
        let mut dc = demande("10_CORPOREL", "PRESCRIPTION", "2019-06-01", "2021-01-01", vec![]);
        dc.date_naissance_droit = Some("2000-03-01".into());
        assert_eq!(analyser(&dc).unwrap().date_expiration.as_deref(), Some("2029-06-01"));
    }

    #[test]
    fn erreurs_de_saisie() {
        assert!(analyser(&demande("INCONNU", "PRESCRIPTION", "2020-01-01", "2026-01-01", vec![])).is_err());
        assert!(analyser(&demande("5_DROIT_COMMUN", "PRESCRIPTION", "", "2026-01-01", vec![])).is_err());
        assert!(analyser(&demande("5_DROIT_COMMUN", "PRESCRIPTION", "2020-01-01", "2026-01-01",
            vec![ev("MEDIATION", None, None, None, Some("2022-01-01"), Some("2021-01-01"))])).is_err());
    }
}
