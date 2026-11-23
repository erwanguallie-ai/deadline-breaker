//! Moteur déterministe de computation des délais de procédure civile.
//!
//! Art. 640 CPC : le délai a pour origine la date de l'acte, de l'événement,
//!                de la décision ou de la notification qui le fait courir.
//! Art. 641 CPC : délai en jours -> le jour de l'acte ne compte pas ;
//!                délai en mois/années -> même quantième, à défaut dernier jour du mois ;
//!                délai en mois et en jours -> les mois d'abord, puis les jours.
//! Art. 642 CPC : tout délai expire le dernier jour à 24 h ; prorogation au premier
//!                jour ouvrable suivant si samedi, dimanche, jour férié ou chômé.
//! Art. 643 CPC : juridiction siégeant en métropole -> +1 mois (outre-mer), +2 mois (étranger).
//! Art. 644 CPC : juridiction siégeant outre-mer -> +1 mois (hors de la collectivité), +2 mois (étranger).
//! Jours fériés : art. L. 3133-1 C. trav. ; Alsace-Moselle (Vendredi saint, 26 décembre) ;
//!                abolition de l'esclavage outre-mer (art. L. 3422-2 C. trav.).

use crate::langue::t;

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use serde::{Deserialize, Serialize};

/// Durée d'un délai (art. 641 al. 3 : les mois sont décomptés avant les jours).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Duree {
    pub annees: u32,
    pub mois: u32,
    pub jours: u32,
}

impl Duree {
    pub fn est_nulle(&self) -> bool {
        self.annees == 0 && self.mois == 0 && self.jours == 0
    }

    pub fn libelle(&self) -> String {
        let mut p = Vec::new();
        if self.annees > 0 {
            p.push(crate::tr!("{} an{}", "{} year{}", self.annees, if self.annees > 1 { "s" } else { "" }));
        }
        if self.mois > 0 {
            p.push(crate::tr!("{} mois{}", "{} month{}", self.mois, if crate::langue::en() && self.mois > 1 { "s" } else { "" }));
        }
        if self.jours > 0 {
            p.push(crate::tr!("{} jour{}", "{} day{}", self.jours, if self.jours > 1 { "s" } else { "" }));
        }
        if p.is_empty() {
            t("0 jour").to_string()
        } else {
            p.join(if crate::langue::en() { " and " } else { " et " })
        }
    }
}

/// Calendrier du lieu où siège la juridiction (jours fériés et règle de distance).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Calendrier {
    Metropole,
    AlsaceMoselle,
    Guadeloupe,
    Martinique,
    Guyane,
    Reunion,
    Mayotte,
    SaintMartin,
    SaintBarthelemy,
    SaintPierreMiquelon,
    WallisFutuna,
}

pub const CALENDRIERS: [Calendrier; 11] = [
    Calendrier::Metropole,
    Calendrier::AlsaceMoselle,
    Calendrier::Guadeloupe,
    Calendrier::Martinique,
    Calendrier::Guyane,
    Calendrier::Reunion,
    Calendrier::Mayotte,
    Calendrier::SaintMartin,
    Calendrier::SaintBarthelemy,
    Calendrier::SaintPierreMiquelon,
    Calendrier::WallisFutuna,
];

#[derive(Debug, Clone, Serialize)]
pub struct InfoCalendrier {
    pub code: &'static str,
    pub libelle: &'static str,
    pub outre_mer: bool,
}

impl Calendrier {
    pub fn depuis_code(code: &str) -> Calendrier {
        let c = code.trim().to_uppercase();
        CALENDRIERS.iter().copied().find(|x| x.code() == c).unwrap_or(Calendrier::Metropole)
    }

    pub fn code(self) -> &'static str {
        match self {
            Calendrier::Metropole => "METROPOLE",
            Calendrier::AlsaceMoselle => "ALSACE_MOSELLE",
            Calendrier::Guadeloupe => "GUADELOUPE",
            Calendrier::Martinique => "MARTINIQUE",
            Calendrier::Guyane => "GUYANE",
            Calendrier::Reunion => "REUNION",
            Calendrier::Mayotte => "MAYOTTE",
            Calendrier::SaintMartin => "SAINT_MARTIN",
            Calendrier::SaintBarthelemy => "SAINT_BARTHELEMY",
            Calendrier::SaintPierreMiquelon => "SAINT_PIERRE_MIQUELON",
            Calendrier::WallisFutuna => "WALLIS_FUTUNA",
        }
    }

    pub fn libelle(self) -> &'static str {
        match self {
            Calendrier::Metropole => t("France métropolitaine"),
            Calendrier::AlsaceMoselle => t("Alsace-Moselle (Bas-Rhin, Haut-Rhin, Moselle)"),
            Calendrier::Guadeloupe => "Guadeloupe",
            Calendrier::Martinique => "Martinique",
            Calendrier::Guyane => t("Guyane"),
            Calendrier::Reunion => t("La Réunion"),
            Calendrier::Mayotte => "Mayotte",
            Calendrier::SaintMartin => "Saint-Martin",
            Calendrier::SaintBarthelemy => "Saint-Barthélemy",
            Calendrier::SaintPierreMiquelon => "Saint-Pierre-et-Miquelon",
            Calendrier::WallisFutuna => "Wallis-et-Futuna",
        }
    }

    /// Juridiction siégeant dans une collectivité visée par l'art. 644 CPC.
    pub fn siege_outre_mer(self) -> bool {
        !matches!(self, Calendrier::Metropole | Calendrier::AlsaceMoselle)
    }

    pub fn info(self) -> InfoCalendrier {
        InfoCalendrier { code: self.code(), libelle: self.libelle(), outre_mer: self.siege_outre_mer() }
    }

    /// Jours fériés propres au calendrier (en plus des fériés nationaux).
    fn ferie_local(self, date: NaiveDate, paques: NaiveDate) -> Option<&'static str> {
        let (m, j) = (date.month(), date.day());
        match self {
            Calendrier::AlsaceMoselle => {
                if date == paques - Duration::days(2) {
                    Some(t("Vendredi saint (Alsace-Moselle)"))
                } else if m == 12 && j == 26 {
                    Some(t("Saint-Étienne (Alsace-Moselle)"))
                } else {
                    None
                }
            }
            // Art. L. 3422-2 C. trav. : commémoration de l'abolition de l'esclavage
            Calendrier::Mayotte if (m, j) == (4, 27) => Some(t("Abolition de l'esclavage (Mayotte)")),
            Calendrier::Martinique if (m, j) == (5, 22) => Some(t("Abolition de l'esclavage (Martinique)")),
            Calendrier::Guadeloupe if (m, j) == (5, 27) => Some(t("Abolition de l'esclavage (Guadeloupe)")),
            Calendrier::SaintMartin if (m, j) == (5, 27) => Some(t("Abolition de l'esclavage (Saint-Martin)")),
            Calendrier::Guyane if (m, j) == (6, 10) => Some(t("Abolition de l'esclavage (Guyane)")),
            Calendrier::SaintBarthelemy if (m, j) == (10, 9) => Some(t("Abolition de l'esclavage (Saint-Barthélemy)")),
            Calendrier::Reunion if (m, j) == (12, 20) => Some(t("Abolition de l'esclavage (La Réunion)")),
            _ => None,
        }
    }
}

/// Lieu où demeure la partie, apprécié par rapport au siège de la juridiction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Residence {
    Metropole,
    /// Outre-mer, dans une autre collectivité que celle du siège de la juridiction.
    OutreMer,
    /// Outre-mer, dans la collectivité où siège la juridiction.
    OutreMerLocale,
    Etranger,
}

impl Residence {
    pub fn depuis_code(code: &str) -> Residence {
        match code.trim().to_uppercase().as_str() {
            "DOM_TOM" | "OUTRE_MER" => Residence::OutreMer,
            "DOM_TOM_LOCAL" | "OUTRE_MER_LOCAL" => Residence::OutreMerLocale,
            "ETRANGER" => Residence::Etranger,
            _ => Residence::Metropole,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Residence::Metropole => "FRANCE_METROPOLE",
            Residence::OutreMer => "DOM_TOM",
            Residence::OutreMerLocale => "DOM_TOM_LOCAL",
            Residence::Etranger => "ETRANGER",
        }
    }
}

/// Augmentation de distance applicable : (mois, article).
/// `pourvoi` : le pourvoi est porté devant la Cour de cassation, qui siège en métropole (art. 643).
pub fn augmentation_distance(
    calendrier: Calendrier,
    residence: Residence,
    applicable_643: bool,
    applicable_644: bool,
    pourvoi: bool,
) -> (u32, &'static str) {
    if pourvoi || !calendrier.siege_outre_mer() {
        if !applicable_643 {
            return (0, "");
        }
        match residence {
            Residence::Metropole => (0, ""),
            Residence::OutreMer | Residence::OutreMerLocale => (1, "643"),
            Residence::Etranger => (2, "643"),
        }
    } else {
        if !applicable_644 {
            return (0, "");
        }
        match residence {
            Residence::OutreMerLocale => (0, ""),
            Residence::Metropole | Residence::OutreMer => (1, "644"),
            Residence::Etranger => (2, "644"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultatCalculDelai {
    pub date_declencheur: String,
    pub date_echeance_theorique: String,
    pub date_echeance_legale: String,
    pub jours_restants: i64,
    pub est_expire: bool,
    pub duree: Duree,
    pub mois_distance: u32,
    pub article_distance: String,
    pub calendrier: String,
    pub prorogation_appliquee: bool,
    pub jours_proroges: i64,
    pub etapes: Vec<String>,
    pub justification_pas_a_pas: String,
    pub fondement_legal: String,
}

pub fn paques_gregorien(annee: i32) -> NaiveDate {
    let a = annee % 19;
    let b = annee / 100;
    let c = annee % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let mois = (h + l - 7 * m + 114) / 31;
    let jour = ((h + l - 7 * m + 114) % 31) + 1;
    NaiveDate::from_ymd_opt(annee, mois as u32, jour as u32).expect("date de Pâques valide")
}

/// Libellé du jour férié si la date en est un pour le calendrier donné.
pub fn libelle_jour_ferie(date: NaiveDate, calendrier: Calendrier) -> Option<&'static str> {
    let (m, j) = (date.month(), date.day());
    let fixe = match (m, j) {
        (1, 1) => Some(t("Jour de l'an")),
        (5, 1) => Some(t("Fête du Travail")),
        (5, 8) => Some(t("Victoire 1945")),
        (7, 14) => Some(t("Fête nationale")),
        (8, 15) => Some(t("Assomption")),
        (11, 1) => Some(t("Toussaint")),
        (11, 11) => Some(t("Armistice 1918")),
        (12, 25) => Some("Noël"),
        _ => None,
    };
    if fixe.is_some() {
        return fixe;
    }
    let paques = paques_gregorien(date.year());
    if date == paques + Duration::days(1) {
        return Some(t("Lundi de Pâques"));
    }
    if date == paques + Duration::days(39) {
        return Some(t("Ascension"));
    }
    if date == paques + Duration::days(50) {
        return Some(t("Lundi de Pentecôte"));
    }
    calendrier.ferie_local(date, paques)
}

pub fn est_chome_642(date: NaiveDate, calendrier: Calendrier) -> bool {
    matches!(date.weekday(), Weekday::Sat | Weekday::Sun) || libelle_jour_ferie(date, calendrier).is_some()
}

fn dernier_jour_du_mois(annee: i32, mois: u32) -> u32 {
    let (a, m) = if mois == 12 { (annee + 1, 1) } else { (annee, mois + 1) };
    NaiveDate::from_ymd_opt(a, m, 1).and_then(|d| d.pred_opt()).map(|d| d.day()).unwrap_or(28)
}

/// Art. 641 al. 2 : même quantième ; à défaut, dernier jour du mois.
pub fn ajouter_mois_cpc(date_depart: NaiveDate, nb_mois: u32) -> NaiveDate {
    let total = date_depart.year() * 12 + (date_depart.month() as i32 - 1) + nb_mois as i32;
    let annee = total.div_euclid(12);
    let mois = (total.rem_euclid(12) + 1) as u32;
    let jour = date_depart.day().min(dernier_jour_du_mois(annee, mois));
    NaiveDate::from_ymd_opt(annee, mois, jour).expect("date calculée valide")
}

/// Premier jour ouvrable suivant (Art. 642 al. 2). Retourne (date, nb de jours ajoutés).
pub fn proroger_642(date: NaiveDate, calendrier: Calendrier) -> (NaiveDate, i64) {
    let mut d = date;
    let mut n = 0;
    while est_chome_642(d, calendrier) {
        d += Duration::days(1);
        n += 1;
    }
    (d, n)
}

/// Dernier jour ouvrable précédent (délais « à rebours », par prudence).
pub fn anticiper_jour_ouvrable(date: NaiveDate, calendrier: Calendrier) -> (NaiveDate, i64) {
    let mut d = date;
    let mut n = 0;
    while est_chome_642(d, calendrier) {
        d -= Duration::days(1);
        n += 1;
    }
    (d, n)
}

pub fn fr(d: NaiveDate) -> String {
    const JOURS: [&str; 7] = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];
    const DAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
    let i = d.weekday().num_days_from_monday() as usize;
    format!("{} {}", if crate::langue::en() { DAYS[i] } else { JOURS[i] }, d.format("%d/%m/%Y"))
}

pub fn calculer_echeance(
    date_depart: NaiveDate,
    duree: Duree,
    mois_distance: u32,
    article_distance: &str,
    calendrier: Calendrier,
    date_pivot: NaiveDate,
) -> ResultatCalculDelai {
    let mut etapes: Vec<String> = Vec::new();
    etapes.push(crate::tr!("Origine du délai : {} (Art. 640 CPC).", "Start of the time limit: {} (Art. 640 CPC).", fr(date_depart)));

    let mois_delai = duree.annees * 12 + duree.mois;
    if mois_delai > 0 {
        etapes.push(crate::tr!(
            "Délai de {} : expiration le jour portant le même quantième, à défaut le dernier jour du mois (Art. 641 al. 2 CPC).", "Time limit of {}: expires on the day bearing the same date, failing that the last day of the month (Art. 641 para. 2 CPC).",
            Duree { jours: 0, ..duree }.libelle()
        ));
    }
    if mois_distance > 0 {
        etapes.push(crate::tr!(
            "Augmentation de distance de {} mois (Art. {} CPC){}.", "Distance extension of {} month(s) (Art. {} CPC){}.",
            mois_distance,
            article_distance,
            if duree.jours > 0 { t(", décomptée avec les mois avant les jours (Art. 641 al. 3 CPC)") } else { "" }
        ));
    }
    let total_mois = mois_delai + mois_distance;
    let base = if total_mois > 0 { ajouter_mois_cpc(date_depart, total_mois) } else { date_depart };
    if duree.jours > 0 {
        if total_mois > 0 {
            etapes.push(crate::tr!(
                "Mois décomptés d'abord : {} ; puis {} jour(s) (Art. 641 al. 3 CPC).", "Months counted first: {}; then {} day(s) (Art. 641 para. 3 CPC).",
                fr(base),
                duree.jours
            ));
        } else {
            etapes.push(crate::tr!(
                "Délai de {} jour(s) ; le jour de l'acte ne compte pas (Art. 641 al. 1 CPC).", "Time limit of {} day(s); the day of the triggering act is not counted (Art. 641 para. 1 CPC).",
                duree.jours
            ));
        }
    }
    let theorique = base + Duration::days(duree.jours as i64);
    etapes.push(crate::tr!("Fin théorique du délai : {} à 24 h (Art. 642 al. 1 CPC).", "Theoretical end of the time limit: {} at midnight (Art. 642 para. 1 CPC).", fr(theorique)));

    let (echeance, jours_proroges) = proroger_642(theorique, calendrier);
    if jours_proroges > 0 {
        let motif = libelle_jour_ferie(theorique, calendrier)
            .map(|l| crate::tr!("jour férié : {}", "public holiday: {}", l))
            .unwrap_or_else(|| t("samedi ou dimanche").to_string());
        etapes.push(crate::tr!(
            "Fin théorique non ouvrable ({}) : prorogation au premier jour ouvrable suivant, le {} à 24 h (Art. 642 al. 2 CPC).", "Theoretical end falls on a non-working day ({}): extended to the next working day, {} at midnight (Art. 642 para. 2 CPC).",
            motif,
            fr(echeance)
        ));
    }
    if calendrier != Calendrier::Metropole {
        etapes.push(crate::tr!("Calendrier des jours fériés appliqué : {}.", "Public holiday calendar applied: {}.", calendrier.libelle()));
    }

    let jours_restants = (echeance - date_pivot).num_days();
    let mut fondement = String::from("Art. 640, 641, 642");
    if mois_distance > 0 {
        fondement.push_str(&format!(" et {}", article_distance));
    }
    fondement.push_str(" CPC");

    ResultatCalculDelai {
        date_declencheur: date_depart.format("%Y-%m-%d").to_string(),
        date_echeance_theorique: theorique.format("%Y-%m-%d").to_string(),
        date_echeance_legale: echeance.format("%Y-%m-%d").to_string(),
        jours_restants,
        est_expire: jours_restants < 0,
        duree,
        mois_distance,
        article_distance: article_distance.to_string(),
        calendrier: calendrier.code().to_string(),
        prorogation_appliquee: jours_proroges > 0,
        jours_proroges,
        justification_pas_a_pas: etapes.join(" "),
        etapes,
        fondement_legal: fondement,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }
    fn mois(n: u32) -> Duree {
        Duree { mois: n, ..Default::default() }
    }
    fn jours(n: u32) -> Duree {
        Duree { jours: n, ..Default::default() }
    }
    const M: Calendrier = Calendrier::Metropole;

    #[test]
    fn paques_connues() {
        assert_eq!(paques_gregorien(2024), d("2024-03-31"));
        assert_eq!(paques_gregorien(2025), d("2025-04-20"));
        assert_eq!(paques_gregorien(2026), d("2026-04-05"));
        assert_eq!(paques_gregorien(2027), d("2027-03-28"));
    }

    #[test]
    fn mois_quantieme_et_fin_de_mois() {
        assert_eq!(ajouter_mois_cpc(d("2026-01-31"), 1), d("2026-02-28"));
        assert_eq!(ajouter_mois_cpc(d("2028-01-31"), 1), d("2028-02-29"));
        assert_eq!(ajouter_mois_cpc(d("2026-11-30"), 3), d("2027-02-28"));
        assert_eq!(ajouter_mois_cpc(d("2026-10-15"), 3), d("2027-01-15"));
        assert_eq!(ajouter_mois_cpc(d("2028-02-29"), 12), d("2029-02-28"));
    }

    #[test]
    fn appel_un_mois_simple() {
        let r = calculer_echeance(d("2026-10-18"), mois(1), 0, "", M, d("2026-11-18"));
        assert_eq!(r.date_echeance_legale, "2026-11-18");
        assert!(!r.prorogation_appliquee);
        assert_eq!(r.jours_restants, 0);
        assert!(!r.est_expire);
    }

    #[test]
    fn prorogation_week_end() {
        let r = calculer_echeance(d("2026-08-22"), mois(3), 0, "", M, d("2026-11-18"));
        assert_eq!(r.date_echeance_theorique, "2026-11-22");
        assert_eq!(r.date_echeance_legale, "2026-11-23");
    }

    #[test]
    fn prorogation_ferie() {
        let r = calculer_echeance(d("2026-10-27"), jours(15), 0, "", M, d("2026-11-01"));
        assert_eq!(r.date_echeance_theorique, "2026-11-11");
        assert_eq!(r.date_echeance_legale, "2026-11-12");
    }

    #[test]
    fn alsace_moselle_26_decembre() {
        let g = calculer_echeance(d("2025-12-11"), jours(15), 0, "", M, d("2025-12-01"));
        assert_eq!(g.date_echeance_legale, "2025-12-26");
        let am = calculer_echeance(d("2025-12-11"), jours(15), 0, "", Calendrier::AlsaceMoselle, d("2025-12-01"));
        assert_eq!(am.date_echeance_legale, "2025-12-29");
    }

    #[test]
    fn feries_outre_mer() {
        // 27/05/2026 (mercredi) férié en Guadeloupe uniquement
        let m = calculer_echeance(d("2026-05-12"), jours(15), 0, "", M, d("2026-05-01"));
        assert_eq!(m.date_echeance_legale, "2026-05-27");
        let g = calculer_echeance(d("2026-05-12"), jours(15), 0, "", Calendrier::Guadeloupe, d("2026-05-01"));
        assert_eq!(g.date_echeance_legale, "2026-05-28");
        // 20/12/2027 (lundi) férié à La Réunion
        assert!(est_chome_642(d("2027-12-20"), Calendrier::Reunion));
        assert!(!est_chome_642(d("2027-12-20"), M));
        assert!(est_chome_642(d("2026-06-10"), Calendrier::Guyane));
    }

    #[test]
    fn distance_643_et_644() {
        // Juridiction métropolitaine
        assert_eq!(augmentation_distance(M, Residence::OutreMer, true, true, false), (1, "643"));
        assert_eq!(augmentation_distance(M, Residence::Etranger, true, true, false), (2, "643"));
        assert_eq!(augmentation_distance(M, Residence::Metropole, true, true, false), (0, ""));
        // Juridiction siégeant à La Réunion (art. 644)
        let r = Calendrier::Reunion;
        assert_eq!(augmentation_distance(r, Residence::OutreMerLocale, true, true, false), (0, ""));
        assert_eq!(augmentation_distance(r, Residence::Metropole, true, true, false), (1, "644"));
        assert_eq!(augmentation_distance(r, Residence::Etranger, true, true, false), (2, "644"));
        // Pourvoi : Cour de cassation en métropole -> art. 643, y compris depuis la collectivité
        assert_eq!(augmentation_distance(r, Residence::OutreMerLocale, true, false, true), (1, "643"));
        // Délai non concerné
        assert_eq!(augmentation_distance(M, Residence::Etranger, false, false, false), (0, ""));
    }

    #[test]
    fn distance_mois_puis_jours() {
        // 15 jours + 1 mois depuis le 31/01/2026 : 28/02 puis +15 j = 15/03 (dimanche) -> 16/03
        let r = calculer_echeance(d("2026-01-31"), jours(15), 1, "643", M, d("2026-02-01"));
        assert_eq!(r.date_echeance_theorique, "2026-03-15");
        assert_eq!(r.date_echeance_legale, "2026-03-16");
        // 1 mois + 2 mois : 30/04/2026
        let e = calculer_echeance(d("2026-01-31"), mois(1), 2, "643", M, d("2026-02-01"));
        assert_eq!(e.date_echeance_legale, "2026-04-30");
    }

    #[test]
    fn duree_composee_mois_et_jours() {
        // 2 mois et 10 jours depuis le 31/12/2026 : 28/02/2027 puis +10 = 10/03/2027 (mercredi)
        let r = calculer_echeance(d("2026-12-31"), Duree { annees: 0, mois: 2, jours: 10 }, 0, "", M, d("2027-01-01"));
        assert_eq!(r.date_echeance_legale, "2027-03-10");
        // 1 an depuis le 29/02/2028 -> 28/02/2029 (mercredi)
        let a = calculer_echeance(d("2028-02-29"), Duree { annees: 1, ..Default::default() }, 0, "", M, d("2028-03-01"));
        assert_eq!(a.date_echeance_legale, "2029-02-28");
        assert_eq!(Duree { annees: 0, mois: 2, jours: 10 }.libelle(), "2 mois et 10 jours");
    }

    #[test]
    fn rebours_anticipe() {
        assert_eq!(anticiper_jour_ouvrable(d("2026-11-14"), M).0, d("2026-11-13"));
        assert_eq!(anticiper_jour_ouvrable(d("2026-11-11"), M).0, d("2026-11-10"));
    }
}
