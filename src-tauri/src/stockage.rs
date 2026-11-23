//! Persistance locale : lecture tolérante, écriture atomique, copie de secours,
//! sauvegardes horodatées et migration des anciennes bases (purge de la démonstration).

use crate::langue::t;

use crate::modeles::*;
use chrono::Local;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static COMPTEUR_ID: AtomicU64 = AtomicU64::new(0);

/// Identifiant unique : préfixe + horodatage ms + compteur (pas de collision intra-ms).
pub fn nouvel_id(prefixe: &str) -> String {
    let n = COMPTEUR_ID.fetch_add(1, Ordering::Relaxed);
    format!("{}-{}-{}", prefixe, chrono::Utc::now().timestamp_millis(), n)
}

pub fn horodatage_affiche() -> String {
    Local::now().format("%d/%m/%Y %H:%M:%S").to_string()
}

pub fn horodatage_iso() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string()
}

pub struct Chemins {
    pub fichier_donnees: PathBuf,
    pub fichier_cache_articles: PathBuf,
    pub dossier_sauvegardes: PathBuf,
    pub dossier_exports: PathBuf,
}

impl Chemins {
    pub fn initialiser() -> Chemins {
        // %APPDATA%\LexProcedura\data.json (même emplacement que les versions précédentes)
        let mut dossier_app = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
        dossier_app.push("LexProcedura");
        let _ = fs::create_dir_all(&dossier_app);

        // Documents\Deadline Breaker\{Sauvegardes, Exports} : visibles par l'utilisateur.
        // L'ancien dossier Documents\LexProcedura est renommé une fois (sauvegardes et exports conservés).
        let base_docs = dirs::document_dir().unwrap_or_else(|| dossier_app.clone());
        let docs = base_docs.join("Deadline Breaker");
        let ancien = base_docs.join("LexProcedura");
        if !docs.exists() && ancien.is_dir() {
            let _ = fs::rename(&ancien, &docs);
        }
        let sauvegardes = docs.join("Sauvegardes");
        let exports = docs.join("Exports");
        let _ = fs::create_dir_all(&sauvegardes);
        let _ = fs::create_dir_all(&exports);

        Chemins {
            fichier_donnees: dossier_app.join("data.json"),
            fichier_cache_articles: dossier_app.join("articles_legifrance.json"),
            dossier_sauvegardes: sauvegardes,
            dossier_exports: exports,
        }
    }

    fn fichier_secours(&self) -> PathBuf {
        self.fichier_donnees.with_extension("json.bak")
    }

    fn fichier_temporaire(&self) -> PathBuf {
        self.fichier_donnees.with_extension("json.tmp")
    }
}

fn lire_json(chemin: &Path) -> Result<DonneesApplication, String> {
    let txt = fs::read_to_string(chemin).map_err(|e| format!("lecture impossible : {}", e))?;
    let propre = txt.trim_start_matches('\u{feff}').trim();
    if propre.is_empty() {
        return Err("fichier vide".to_string());
    }
    serde_json::from_str::<DonneesApplication>(propre).map_err(|e| format!("JSON invalide : {}", e))
}

/// Charge la base. Ne détruit jamais un fichier illisible : il est mis de côté.
pub fn charger(chemins: &Chemins) -> (DonneesApplication, Vec<String>) {
    let mut avertissements = Vec::new();
    let principal = &chemins.fichier_donnees;

    if !principal.exists() {
        let mut d = DonneesApplication::default();
        journaliser(
            &mut d,
            "SYSTEME",
            t("Création d'une base cabinet vierge"),
            &format!("Fichier de données : {}", principal.display()),
            "",
            None,
        );
        return (d, avertissements);
    }

    match lire_json(principal) {
        Ok(mut d) => {
            migrer(&mut d);
            (d, avertissements)
        }
        Err(e) => {
            let quarantaine = principal.with_file_name(format!(
                "data.corrompu-{}.json",
                Local::now().format("%Y%m%d-%H%M%S")
            ));
            let _ = fs::copy(principal, &quarantaine);
            avertissements.push(crate::tr!(
                "Le fichier de données est illisible ({}). Une copie a été conservée : {}", "The data file is unreadable ({}). A copy was kept: {}",
                e,
                quarantaine.display()
            ));
            match lire_json(&chemins.fichier_secours()) {
                Ok(mut d) => {
                    migrer(&mut d);
                    avertissements.push(t("La copie de secours (data.json.bak) a été restaurée.").to_string());
                    journaliser(&mut d, "SYSTEME", t("Restauration automatique depuis la copie de secours"), &e, "", None);
                    (d, avertissements)
                }
                Err(_) => {
                    avertissements.push(
                        t("Aucune copie de secours exploitable : démarrage sur une base vierge. Importez une sauvegarde si nécessaire.")
                            .to_string(),
                    );
                    let mut d = DonneesApplication::default();
                    journaliser(&mut d, "SYSTEME", t("Base illisible : démarrage sur base vierge"), &e, "", None);
                    (d, avertissements)
                }
            }
        }
    }
}

/// Écriture atomique : fichier temporaire + fsync, copie de secours de la version précédente, puis remplacement.
/// Les clés API sont d'abord écrites dans le coffre Windows ; data.json ne les contient alors plus.
/// Si le coffre est indisponible (ou en échec), elles restent dans data.json comme auparavant.
pub fn enregistrer(chemins: &Chemins, donnees: &DonneesApplication) -> Result<(), String> {
    let sans_cles;
    let donnees = if crate::coffre::persister(&donnees.parametres).is_ok() {
        sans_cles = DonneesApplication { parametres: donnees.parametres.sans_secrets(), ..donnees.clone() };
        &sans_cles
    } else {
        donnees
    };
    let json = serde_json::to_string_pretty(donnees).map_err(|e| format!("Sérialisation impossible : {}", e))?;
    let tmp = chemins.fichier_temporaire();
    {
        let mut f = File::create(&tmp).map_err(|e| format!("Écriture impossible ({}) : {}", tmp.display(), e))?;
        f.write_all(json.as_bytes()).map_err(|e| format!("Écriture impossible : {}", e))?;
        f.sync_all().map_err(|e| format!("Synchronisation disque impossible : {}", e))?;
    }
    if chemins.fichier_donnees.exists() {
        let _ = fs::copy(&chemins.fichier_donnees, chemins.fichier_secours());
    }
    fs::rename(&tmp, &chemins.fichier_donnees).map_err(|e| format!("Remplacement du fichier de données impossible : {}", e))
}

pub fn journaliser(
    d: &mut DonneesApplication,
    categorie: &str,
    titre: &str,
    details: &str,
    fondement: &str,
    id_affaire: Option<String>,
) {
    d.historique.insert(
        0,
        EntreeHistorique {
            id: nouvel_id("hist"),
            horodatage: horodatage_affiche(),
            horodatage_iso: horodatage_iso(),
            categorie: categorie.to_string(),
            titre: titre.to_string(),
            details: details.to_string(),
            fondement: fondement.to_string(),
            id_affaire,
        },
    );
}

/// Nettoie les bases produites par les versions « maquette » (données de démonstration codées en dur).
pub fn migrer(d: &mut DonneesApplication) {
    let mut actions: Vec<String> = Vec::new();

    const IDS_DEMO: [&str; 5] = ["demo-001", "demo-002", "aff-001", "aff-002", "aff-003"];
    let avant = d.affaires.len();
    d.affaires.retain(|a| !IDS_DEMO.contains(&a.id.as_str()));
    let retirees = avant - d.affaires.len();
    if retirees > 0 {
        actions.push(crate::tr!("{} affaire(s) de démonstration supprimée(s)", "{} demo matter(s) removed", retirees));
    }

    const EMAILS_DEMO: [&str; 2] = ["englishlawyerm2@gmail.com", "maitrsimonin@gmail.com"];
    let avant_mails = d.parametres.emails_destinataires.len();
    d.parametres
        .emails_destinataires
        .retain(|e| !e.trim().is_empty() && !EMAILS_DEMO.contains(&e.trim().to_lowercase().as_str()));
    if d.parametres.emails_destinataires.len() != avant_mails {
        actions.push("adresses e-mail de démonstration retirées des destinataires".to_string());
    }

    if d.parametres.resend_api_key.trim() == "re_votre_cle_api_ici" {
        d.parametres.resend_api_key.clear();
        actions.push("clé Resend factice effacée".to_string());
    }
    if d.parametres.resend_expediteur.contains("alertes@resend.dev") {
        d.parametres.resend_expediteur = "Deadline Breaker <onboarding@resend.dev>".to_string();
    }


    if d.parametres.resend_expediteur.trim() == "LexProcedura <onboarding@resend.dev>" {
        d.parametres.resend_expediteur = "Deadline Breaker <onboarding@resend.dev>".to_string();
    }
    if d.parametres.gemini_model.trim().is_empty() || d.parametres.gemini_model.trim() == "gemini-1.5-flash" {
        d.parametres.gemini_model = "gemini-2.5-flash".to_string();
    }

    if d.parametres.seuil_orange_jours <= d.parametres.seuil_rouge_jours {
        d.parametres.seuil_rouge_jours = 3;
        d.parametres.seuil_orange_jours = 7;
    }

    // Identifiants manquants / statut « EXPIRE » figé (désormais calculé dynamiquement)
    for a in d.affaires.iter_mut() {
        if a.id.trim().is_empty() {
            a.id = nouvel_id("aff");
        }
        for acte in a.actes.iter_mut() {
            if acte.id.trim().is_empty() {
                acte.id = nouvel_id("act");
            }
            if acte.statut != "ACCOMPLI" {
                acte.statut = "EN_COURS".to_string();
            }
        }
        for dil in a.diligences.iter_mut() {
            if dil.id.trim().is_empty() {
                dil.id = nouvel_id("dil");
            }
        }
    }

    // v3 : calendrier par défaut (remplace le simple booléen Alsace-Moselle)
    if d.version_schema < 3 && d.parametres.regime_alsace_moselle {
        d.parametres.calendrier_defaut = "ALSACE_MOSELLE".to_string();
        actions.push("régime Alsace-Moselle repris comme calendrier par défaut".to_string());
    }
    let cal = crate::cpc_engine::Calendrier::depuis_code(&d.parametres.calendrier_defaut);
    d.parametres.calendrier_defaut = cal.code().to_string();
    d.parametres.regime_alsace_moselle = cal == crate::cpc_engine::Calendrier::AlsaceMoselle;
    for a in d.affaires.iter_mut() {
        if !a.calendrier_juridiction.trim().is_empty() {
            a.calendrier_juridiction = crate::cpc_engine::Calendrier::depuis_code(&a.calendrier_juridiction).code().to_string();
        }
        for acte in a.actes.iter_mut() {
            if acte.duree.est_nulle() {
                if let Some(r) = crate::regles::trouver(&acte.type_delai) {
                    acte.duree = r.duree;
                }
            }
        }
    }

    if d.version_schema < VERSION_SCHEMA {
        actions.push(format!("schéma de données v{} -> v{}", d.version_schema, VERSION_SCHEMA));
        d.version_schema = VERSION_SCHEMA;
    }

    if !actions.is_empty() {
        let details = actions.join(" ; ");
        journaliser(d, "SYSTEME", t("Migration de la base de données"), &details, "", None);
    }
}

/// Nom de fichier sûr (pas de séparateur de chemin ni de caractère interdit sous Windows).
pub fn nom_fichier_sur(nom: &str) -> String {
    let mut s: String = nom
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    s = s.trim().trim_matches('.').to_string();
    if s.is_empty() {
        s = "export".to_string();
    }
    if s.chars().count() > 120 {
        s = s.chars().take(120).collect();
    }
    s
}

pub fn ecrire_fichier(dossier: &Path, nom: &str, contenu: &[u8]) -> Result<PathBuf, String> {
    fs::create_dir_all(dossier).map_err(|e| format!("Création du dossier impossible : {}", e))?;
    let chemin = dossier.join(nom_fichier_sur(nom));
    fs::write(&chemin, contenu).map_err(|e| format!("Écriture impossible ({}) : {}", chemin.display(), e))?;
    Ok(chemin)
}

/// Sauvegarde horodatée dans Documents : les clés API n'y figurent jamais (elles restent dans data.json).
pub fn sauvegarde_horodatee(chemins: &Chemins, donnees: &DonneesApplication, motif: &str) -> Result<PathBuf, String> {
    let copie = DonneesApplication { parametres: donnees.parametres.sans_secrets(), ..donnees.clone() };
    let json = serde_json::to_string_pretty(&copie).map_err(|e| e.to_string())?;
    let nom = format!(
        "DeadlineBreaker_sauvegarde_{}_{}.json",
        Local::now().format("%Y%m%d_%H%M%S"),
        nom_fichier_sur(motif)
    );
    ecrire_fichier(&chemins.dossier_sauvegardes, &nom, json.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_purge_la_demo() {
        let json = r#"{
            "parametres": {"seuil_rouge_jours":3,"seuil_orange_jours":7,"resend_api_key":"re_votre_cle_api_ici",
              "emails_destinataires":["englishlawyerM2@gmail.com","Maitrsimonin@gmail.com","contact@cabinet.fr"],
              "piste_client_id":"","piste_client_secret":"","regime_alsace_moselle":false},
            "affaires": [
              {"id":"demo-001","reference_cabinet":"DOS","nom_dossier":"SARL BATIPRO","juridiction":"TJ","numero_rg":"RG","lieu_residence_partie":"FRANCE_METROPOLE","actes":[]},
              {"id":"aff-1700000000000","reference_cabinet":"R1","nom_dossier":"Vrai dossier","juridiction":"TJ","numero_rg":"RG","lieu_residence_partie":"FRANCE_METROPOLE",
               "actes":[{"id":"a1","nature":"x","date_evenement":"2026-01-01","type_delai":"DELAI_APPEL_CONTENTIEUX","fondement_cpc":"Art. 538","date_echeance_calculee":"2026-02-02","details_calcul":"","statut":"EXPIRE","alerte_envoyee":false}]}
            ],
            "historique": []
        }"#;
        let mut d: DonneesApplication = serde_json::from_str(json).unwrap();
        assert_eq!(d.version_schema, 0);
        migrer(&mut d);
        assert_eq!(d.affaires.len(), 1);
        assert_eq!(d.affaires[0].nom_dossier, "Vrai dossier");
        assert_eq!(d.affaires[0].actes[0].statut, "EN_COURS");
        assert_eq!(d.parametres.emails_destinataires, vec!["contact@cabinet.fr".to_string()]);
        assert!(d.parametres.resend_api_key.is_empty());
        assert_eq!(d.version_schema, VERSION_SCHEMA);
        assert_eq!(d.historique.len(), 1);
        assert_eq!(d.affaires[0].actes[0].duree.mois, 1);
        assert_eq!(d.parametres.calendrier_defaut, "METROPOLE");
    }

    #[test]
    fn migration_alsace_vers_calendrier() {
        let mut d: DonneesApplication = serde_json::from_str(
            r#"{"version_schema":2,"parametres":{"regime_alsace_moselle":true},"affaires":[],"historique":[]}"#,
        )
        .unwrap();
        migrer(&mut d);
        assert_eq!(d.parametres.calendrier_defaut, "ALSACE_MOSELLE");
        assert!(d.parametres.regime_alsace_moselle);
        assert_eq!(d.version_schema, VERSION_SCHEMA);
    }

    #[test]
    fn cles_rangees_dans_le_coffre() {
        crate::coffre::backend::activer();
        let dossier = std::env::temp_dir().join(format!("lexprocedura-coffre-{}", std::process::id()));
        std::fs::create_dir_all(&dossier).unwrap();
        let chemins = Chemins {
            fichier_donnees: dossier.join("data.json"),
            dossier_sauvegardes: dossier.clone(),
            dossier_exports: dossier.clone(),
            fichier_cache_articles: dossier.join("cache.json"),
        };
        // Ancienne base : la clé est encore dans data.json.
        let mut d = DonneesApplication::default();
        d.parametres.gemini_api_key = "AIza-ANCIENNE".into();
        std::fs::write(&chemins.fichier_donnees, serde_json::to_string(&d).unwrap()).unwrap();
        let (mut charge, mut avert) = charger(&chemins);
        assert_eq!(crate::coffre::charger(&mut charge.parametres, &mut avert), 1);
        assert_eq!(crate::coffre::backend::contenu("gemini_api_key").as_deref(), Some("AIza-ANCIENNE"));
        enregistrer(&chemins, &charge).unwrap();
        let brut = std::fs::read_to_string(&chemins.fichier_donnees).unwrap();
        assert!(!brut.contains("AIza-ANCIENNE"), "la clé ne doit plus figurer dans data.json");
        // Redémarrage : la clé revient du coffre.
        let (mut relu, mut avert2) = charger(&chemins);
        assert_eq!(crate::coffre::charger(&mut relu.parametres, &mut avert2), 0);
        assert_eq!(relu.parametres.gemini_api_key, "AIza-ANCIENNE");
        // Clé vidée dans les Paramètres : retirée du coffre.
        relu.parametres.gemini_api_key.clear();
        relu.parametres.claude_api_key = "sk-ant-NOUVELLE".into();
        enregistrer(&chemins, &relu).unwrap();
        assert!(crate::coffre::backend::contenu("gemini_api_key").is_none());
        assert_eq!(crate::coffre::backend::contenu("claude_api_key").as_deref(), Some("sk-ant-NOUVELLE"));
        assert!(!std::fs::read_to_string(&chemins.fichier_donnees).unwrap().contains("sk-ant-NOUVELLE"));
        let _ = std::fs::remove_dir_all(&dossier);
    }

    #[test]
    fn noms_de_fichier() {
        assert_eq!(nom_fichier_sur("..\\..\\Windows\\a.txt"), "_.._Windows_a.txt");
        assert_eq!(nom_fichier_sur("Échéance : SCI/X?.ics"), "Échéance _ SCI_X_.ics");
    }
}
