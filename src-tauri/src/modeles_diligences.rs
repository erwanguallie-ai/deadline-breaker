//! Modèles de checklists de diligences par type de procédure (points de contrôle génériques,
//! à adapter au dossier). Aucun montant ni délai chiffré n'y figure : les délais sont gérés
//! par le moteur de calcul et le catalogue des délais.

use crate::langue::t;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ModeleDiligences {
    pub code: &'static str,
    pub libelle: &'static str,
    pub elements: Vec<&'static str>,
}

pub fn modeles() -> Vec<ModeleDiligences> {
    vec![
        ModeleDiligences {
            code: "DEMANDEUR_TJ",
            libelle: t("Demandeur - action devant le tribunal judiciaire"),
            elements: vec![
                t("Vérifier la compétence matérielle et territoriale"),
                t("Vérifier la prescription de l'action et son point de départ"),
                t("Vérifier l'obligation éventuelle de tentative préalable de résolution amiable"),
                t("Rédiger l'assignation avec ses mentions obligatoires"),
                t("Faire signifier l'assignation par commissaire de justice"),
                t("Remettre l'assignation au greffe dans le délai requis"),
                t("Communiquer les pièces avec un bordereau numéroté"),
                t("Reporter le calendrier de mise en état dans Deadline Breaker"),
            ],
        },
        ModeleDiligences {
            code: "DEFENDEUR_TJ",
            libelle: t("Défendeur - instance devant le tribunal judiciaire"),
            elements: vec![
                t("Constituer avocat dans le délai (art. 763 CPC)"),
                t("Contrôler la régularité de l'assignation (mentions, signification)"),
                t("Lister les exceptions de procédure à soulever simultanément in limine litis (art. 74 CPC)"),
                t("Examiner les fins de non-recevoir : prescription, qualité, intérêt à agir (art. 122 CPC)"),
                t("Notifier les conclusions en défense par voie électronique"),
                t("Communiquer les pièces avec un bordereau numéroté"),
            ],
        },
        ModeleDiligences {
            code: "APPELANT_MISE_EN_ETAT",
            libelle: t("Appelant - procédure avec mise en état"),
            elements: vec![
                t("Vérifier la date de signification du jugement et le délai d'appel"),
                t("Déclarer l'appel par voie électronique en visant les chefs du jugement critiqués"),
                t("Vérifier les droits ou taxes éventuellement exigibles"),
                t("Enregistrer l'échéance des conclusions de l'appelant (art. 908 CPC)"),
                t("Remettre les conclusions au greffe et les notifier aux avocats constitués"),
                t("Signifier les actes à l'intimé non constitué dans les délais impartis"),
            ],
        },
        ModeleDiligences {
            code: "INTIME_MISE_EN_ETAT",
            libelle: t("Intimé - procédure avec mise en état"),
            elements: vec![
                t("Constituer avocat devant la cour d'appel"),
                t("Noter la date de notification des conclusions de l'appelant"),
                t("Enregistrer l'échéance des conclusions de l'intimé (art. 909 CPC)"),
                t("Apprécier l'opportunité d'un appel incident ou provoqué"),
                t("Remettre les conclusions au greffe et les notifier"),
            ],
        },
        ModeleDiligences {
            code: "APPEL_BREF_DELAI",
            libelle: t("Appel - procédure à bref délai"),
            elements: vec![
                t("Noter la date de réception de l'avis de fixation à bref délai"),
                t("Enregistrer l'échéance des conclusions (art. 906-2 CPC)"),
                t("Notifier les conclusions aux avocats dans le délai de leur remise au greffe"),
                t("Signifier les conclusions aux parties non constituées dans le délai prévu"),
            ],
        },
        ModeleDiligences {
            code: "REFERE",
            libelle: t("Référé"),
            elements: vec![
                t("Qualifier le fondement : urgence, trouble manifestement illicite, provision, mesure d'instruction"),
                t("Obtenir la date d'audience et faire délivrer l'assignation en temps utile"),
                t("Préparer et communiquer les pièces avant l'audience"),
                t("Faire signifier l'ordonnance et noter la date de signification"),
                t("Enregistrer le délai d'appel de l'ordonnance (art. 490 CPC)"),
            ],
        },
        ModeleDiligences {
            code: "EXECUTION",
            libelle: t("Exécution d'une décision"),
            elements: vec![
                t("Vérifier le caractère exécutoire de la décision"),
                t("Faire signifier la décision et noter la date de signification"),
                t("Surveiller l'expiration du délai de recours de l'adversaire"),
                t("Mandater le commissaire de justice pour l'exécution"),
                t("Suivre les éventuelles contestations devant le juge de l'exécution"),
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn modeles_non_vides_et_uniques() {
        let m = super::modeles();
        let mut codes: Vec<_> = m.iter().map(|x| x.code).collect();
        codes.sort();
        codes.dedup();
        assert_eq!(codes.len(), m.len());
        assert!(m.iter().all(|x| x.elements.len() >= 4));
    }
}
