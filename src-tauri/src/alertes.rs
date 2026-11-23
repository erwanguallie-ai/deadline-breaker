//! Alertes e-mail automatiques : sélection des échéances qui franchissent un seuil
//! (par défaut J-7 et J-2) et rédaction du récapitulatif groupé.
//! Chaque seuil n'est notifié qu'une fois par échéance ; si l'application était fermée
//! pendant plusieurs seuils, un seul message les couvre.

use crate::modeles::DonneesApplication;
use chrono::NaiveDate;

#[derive(Debug, Clone, PartialEq)]
pub struct EcheanceAlerte {
    pub id_affaire: String,
    pub nom_dossier: String,
    pub juridiction: String,
    pub numero_rg: String,
    pub id_acte: String,
    pub nature: String,
    pub date_echeance: NaiveDate,
    pub jours: i64,
    pub fondement: String,
    pub sanction: String,
    /// Seuils franchis à marquer comme notifiés.
    pub seuils: Vec<i64>,
}

/// Seuils valides, du plus lointain au plus proche, sans doublon (0 à 90 jours).
pub fn seuils_normalises(seuils: &[i64]) -> Vec<i64> {
    let mut s: Vec<i64> = seuils.iter().copied().filter(|j| (0..=90).contains(j)).collect();
    s.sort_unstable_by(|a, b| b.cmp(a));
    s.dedup();
    s
}

pub fn selectionner(d: &DonneesApplication, aujourd_hui: NaiveDate) -> Vec<EcheanceAlerte> {
    let seuils = seuils_normalises(&d.parametres.alertes_jours);
    let mut liste = Vec::new();
    if seuils.is_empty() {
        return liste;
    }
    for aff in d.affaires.iter().filter(|a| a.statut != "ARCHIVEE") {
        for acte in aff.actes.iter().filter(|x| x.statut != "ACCOMPLI") {
            let Ok(date) = NaiveDate::parse_from_str(&acte.date_echeance_calculee, "%Y-%m-%d") else { continue };
            let jours = (date - aujourd_hui).num_days();
            if jours < 0 {
                continue;
            }
            let franchis: Vec<i64> = seuils.iter().copied().filter(|t| jours <= *t && !acte.alertes_auto_envoyees.contains(t)).collect();
            if franchis.is_empty() {
                continue;
            }
            liste.push(EcheanceAlerte {
                id_affaire: aff.id.clone(),
                nom_dossier: aff.nom_dossier.clone(),
                juridiction: aff.juridiction.clone(),
                numero_rg: aff.numero_rg.clone(),
                id_acte: acte.id.clone(),
                nature: acte.nature.clone(),
                date_echeance: date,
                jours,
                fondement: acte.fondement_cpc.clone(),
                sanction: acte.sanction.clone(),
                seuils: franchis,
            });
        }
    }
    liste.sort_by(|a, b| a.date_echeance.cmp(&b.date_echeance).then(a.nom_dossier.cmp(&b.nom_dossier)));
    liste
}

fn echapper(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn reste(j: i64) -> String {
    match (j, crate::langue::en()) {
        (0, false) => "aujourd'hui".into(),
        (1, false) => "demain".into(),
        (0, true) => "today".into(),
        (1, true) => "tomorrow".into(),
        (_, false) => format!("J-{}", j),
        (_, true) => format!("D-{}", j),
    }
}

pub fn sujet(liste: &[EcheanceAlerte]) -> String {
    let plus_proche = liste.iter().map(|e| e.jours).min().unwrap_or(0);
    if liste.len() == 1 {
        let e = &liste[0];
        crate::tr!("[Deadline Breaker] {} - {} : {}", "[Deadline Breaker] {} - {}: {}", reste(e.jours), e.nom_dossier, e.nature)
    } else {
        crate::tr!("[Deadline Breaker] {} échéances à surveiller (la plus proche : {})", "[Deadline Breaker] {} deadlines to watch (nearest: {})", liste.len(), reste(plus_proche))
    }
}

pub fn html(liste: &[EcheanceAlerte], cabinet: &str) -> String {
    let lignes: String = liste
        .iter()
        .map(|e| {
            let couleur = if e.jours <= 2 { "#be123c" } else { "#b45309" };
            format!(
                "<tr><td style=\"padding:8px;border-top:1px solid #e2e8f0;white-space:nowrap\"><b>{date}</b><br><span style=\"color:{c};font-weight:600\">{reste}</span></td>\
                 <td style=\"padding:8px;border-top:1px solid #e2e8f0\"><b>{dossier}</b><br><span style=\"color:#475569\">{jur}</span></td>\
                 <td style=\"padding:8px;border-top:1px solid #e2e8f0\">{acte}<br><span style=\"color:#475569\">{fond}</span></td>\
                 <td style=\"padding:8px;border-top:1px solid #e2e8f0;color:#475569\">{sanction}</td></tr>",
                date = e.date_echeance.format("%d/%m/%Y"),
                c = couleur,
                reste = reste(e.jours),
                dossier = echapper(&e.nom_dossier),
                jur = echapper(&[e.juridiction.as_str(), e.numero_rg.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" - ")),
                acte = echapper(&e.nature),
                fond = echapper(&e.fondement),
                sanction = echapper(&e.sanction),
            )
        })
        .collect();
    format!(
        "<div style=\"font-family:Segoe UI,Arial,sans-serif;font-size:14px;color:#0f172a\">\
         <h2 style=\"margin:0 0 4px;font-size:18px\">{titre}</h2>\
         <p style=\"margin:0 0 16px;color:#475569\">{intro}</p>\
         <table cellspacing=\"0\" style=\"border-collapse:collapse;width:100%\">\
         <tr style=\"text-align:left;color:#64748b;font-size:12px\"><th style=\"padding:8px\">{c1}</th><th style=\"padding:8px\">{c2}</th><th style=\"padding:8px\">{c3}</th><th style=\"padding:8px\">{c4}</th></tr>\
         {lignes}</table>\
         <p style=\"color:#64748b;font-size:12px;margin-top:16px\">{pied}{cab}. \
         {verif}</p></div>",
        intro = match (liste.len(), crate::langue::en()) {
            (1, false) => "Une échéance vient de franchir un seuil d'alerte.".to_string(),
            (1, true) => "A deadline has just crossed an alert threshold.".to_string(),
            (n, false) => format!("{} échéances viennent de franchir un seuil d'alerte.", n),
            (n, true) => format!("{} deadlines have just crossed an alert threshold.", n),
        },
        titre = if crate::langue::en() { "Deadlines to watch" } else { "Échéances à surveiller" },
        c1 = if crate::langue::en() { "Deadline" } else { "Échéance" },
        c2 = if crate::langue::en() { "Matter" } else { "Dossier" },
        c3 = if crate::langue::en() { "Step" } else { "Acte" },
        c4 = if crate::langue::en() { "Sanction" } else { "Sanction" },
        pied = if crate::langue::en() { "Automatic summary sent by Deadline Breaker" } else { "Récapitulatif automatique envoyé par Deadline Breaker" },
        verif = if crate::langue::en() { "Always check the computation against the texts and the circumstances of the matter." } else { "Vérifiez toujours le calcul au regard des textes et des circonstances du dossier." },
        lignes = lignes,
        cab = if cabinet.trim().is_empty() { String::new() } else { format!(" ({})", echapper(cabinet.trim())) },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modeles::{ActeDossier, Affaire};

    fn d(n: &str) -> NaiveDate {
        NaiveDate::parse_from_str(n, "%Y-%m-%d").unwrap()
    }

    fn base() -> DonneesApplication {
        let mut donnees = DonneesApplication::default();
        donnees.parametres.alertes_jours = vec![2, 7, 7, -1];
        let acte = |id: &str, ech: &str, statut: &str, envoyes: Vec<i64>| ActeDossier {
            id: id.into(),
            nature: format!("Acte {}", id),
            date_echeance_calculee: ech.into(),
            statut: statut.into(),
            alertes_auto_envoyees: envoyes,
            ..Default::default()
        };
        donnees.affaires.push(Affaire {
            id: "aff-1".into(),
            nom_dossier: "SCI A c/ B".into(),
            statut: "ACTIVE".into(),
            actes: vec![
                acte("loin", "2026-12-31", "EN_COURS", vec![]),
                acte("j6", "2026-11-24", "EN_COURS", vec![]),
                acte("j1", "2026-11-19", "EN_COURS", vec![]),
                acte("j1-deja7", "2026-11-19", "EN_COURS", vec![7]),
                acte("j5-deja7", "2026-11-23", "EN_COURS", vec![7]),
                acte("fait", "2026-11-20", "ACCOMPLI", vec![]),
                acte("passe", "2026-11-10", "EN_COURS", vec![]),
            ],
            ..Default::default()
        });
        donnees.affaires.push(Affaire {
            id: "aff-2".into(),
            nom_dossier: "Archivé".into(),
            statut: "ARCHIVEE".into(),
            actes: vec![acte("arch", "2026-11-19", "EN_COURS", vec![])],
            ..Default::default()
        });
        donnees
    }

    #[test]
    fn seuils_franchis_une_seule_fois() {
        assert_eq!(seuils_normalises(&[2, 7, 7, -1, 120]), vec![7, 2]);
        let l = selectionner(&base(), d("2026-11-18"));
        let ids: Vec<(&str, Vec<i64>)> = l.iter().map(|e| (e.id_acte.as_str(), e.seuils.clone())).collect();
        // Trié par date ; J-1 franchit les deux seuils d'un coup ; J-5 déjà prévenu à J-7 : rien.
        assert_eq!(ids, vec![("j1", vec![7, 2]), ("j1-deja7", vec![2]), ("j6", vec![7])]);
        assert_eq!(l[0].jours, 1);
    }

    #[test]
    fn redaction_du_recapitulatif() {
        let l = selectionner(&base(), d("2026-11-18"));
        assert_eq!(sujet(&l), "[Deadline Breaker] 3 échéances à surveiller (la plus proche : demain)");
        assert_eq!(sujet(&l[2..]), "[Deadline Breaker] J-6 - SCI A c/ B : Acte j6");
        crate::langue::definir("en");
        assert_eq!(sujet(&l), "[Deadline Breaker] 3 deadlines to watch (nearest: tomorrow)");
        assert!(html(&l, "").contains("Deadlines to watch"));
        crate::langue::definir("fr");
        let h = html(&l, "Cabinet <X>");
        assert!(h.contains("19/11/2026") && h.contains("Cabinet &lt;X&gt;") && h.contains("3 échéances"));
    }
}
