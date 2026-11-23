//! Modèle de données persistant (data.json). Tous les champs ont une valeur par défaut :
//! un fichier produit par une version antérieure reste lisible sans perte.

use crate::cpc_engine::Duree;
use serde::{Deserialize, Serialize};

pub const VERSION_SCHEMA: u32 = 3;

/// Valeur affichée dans l'interface à la place d'une clé API enregistrée (jamais la clé elle-même).
pub const MASQUE_SECRET: &str = "••••••••";

fn vrai() -> bool {
    true
}
fn seuil_rouge() -> i64 {
    3
}
fn seuil_orange() -> i64 {
    7
}
fn modele_gemini() -> String {
    "gemini-2.5-flash".to_string()
}
fn modele_claude() -> String {
    "claude-opus-5-5".to_string()
}
fn langue_defaut() -> String {
    "en".to_string()
}
fn jours_alerte() -> Vec<i64> {
    vec![7, 2]
}
fn fournisseur_ia() -> String {
    "GEMINI".to_string()
}
fn expediteur_resend() -> String {
    "Deadline Breaker <onboarding@resend.dev>".to_string()
}
fn calendrier_metropole() -> String {
    "METROPOLE".to_string()
}
fn residence() -> String {
    "FRANCE_METROPOLE".to_string()
}
fn statut_en_cours() -> String {
    "EN_COURS".to_string()
}
fn statut_active() -> String {
    "ACTIVE".to_string()
}
fn un() -> u8 {
    1
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct ParametresApp {
    /// Langue de l'interface et des textes produits : "en" (défaut) ou "fr". Le droit reste français.
    #[serde(default = "langue_defaut")]
    pub langue: String,
    pub nom_cabinet: String,
    #[serde(default = "seuil_rouge")]
    pub seuil_rouge_jours: i64,
    #[serde(default = "seuil_orange")]
    pub seuil_orange_jours: i64,
    /// Calendrier par défaut des juridictions (jours fériés, art. 643/644). Ex. "METROPOLE", "ALSACE_MOSELLE".
    #[serde(default = "calendrier_metropole")]
    pub calendrier_defaut: String,
    /// Conservé pour compatibilité (vrai si le calendrier par défaut est Alsace-Moselle).
    pub regime_alsace_moselle: bool,
    // Resend
    pub resend_api_key: String,
    #[serde(default = "expediteur_resend")]
    pub resend_expediteur: String,
    pub emails_destinataires: Vec<String>,
    /// Récapitulatif e-mail automatique lorsqu'une échéance franchit un seuil (J-7, J-2...).
    #[serde(default = "vrai")]
    pub alertes_auto: bool,
    #[serde(default = "jours_alerte")]
    pub alertes_jours: Vec<i64>,
    /// Lancement automatique à l'ouverture de session Windows (reflet de la clé « Run »).
    pub demarrage_windows: bool,
    // OpenLegi (accès aux données Légifrance)
    /// Adresse du serveur MCP OpenLegi (peut contenir le jeton, selon le compte).
    pub openlegi_url: String,
    /// Jeton d'accès transmis en « Authorization: Bearer » (facultatif si inclus dans l'adresse).
    pub openlegi_jeton: String,
    // Copilote IA : fournisseur "GEMINI" (Google) ou "CLAUDE" (Anthropic)
    #[serde(default = "fournisseur_ia")]
    pub ia_fournisseur: String,
    // Gemini
    pub gemini_api_key: String,
    #[serde(default = "modele_gemini")]
    pub gemini_model: String,
    /// Anonymisation avant envoi, quel que soit le fournisseur (nom historique conservé).
    #[serde(default = "vrai")]
    pub gemini_anonymize: bool,
    // Claude (Anthropic)
    pub claude_api_key: String,
    #[serde(default = "modele_claude")]
    pub claude_model: String,
}

impl Default for ParametresApp {
    fn default() -> Self {
        ParametresApp {
            langue: langue_defaut(),
            nom_cabinet: String::new(),
            seuil_rouge_jours: seuil_rouge(),
            seuil_orange_jours: seuil_orange(),
            calendrier_defaut: calendrier_metropole(),
            regime_alsace_moselle: false,
            resend_api_key: String::new(),
            resend_expediteur: expediteur_resend(),
            emails_destinataires: Vec::new(),
            alertes_auto: true,
            alertes_jours: jours_alerte(),
            demarrage_windows: false,
            openlegi_url: String::new(),
            openlegi_jeton: String::new(),
            ia_fournisseur: fournisseur_ia(),
            gemini_api_key: String::new(),
            gemini_model: modele_gemini(),
            gemini_anonymize: true,
            claude_api_key: String::new(),
            claude_model: modele_claude(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Diligence {
    pub id: String,
    pub libelle: String,
    pub fait: bool,
    pub date_fait: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct ActeDossier {
    pub id: String,
    pub nature: String,
    pub date_evenement: String,
    pub type_delai: String,
    pub libelle_delai: String,
    pub fondement_cpc: String,
    pub article_code: String,
    pub article_num: String,
    pub sanction: String,
    pub date_echeance_theorique: String,
    pub date_echeance_calculee: String,
    pub details_calcul: String,
    /// Durée du délai (utile au recalcul des délais libres).
    pub duree: Duree,
    /// Choix explicite d'appliquer ou non l'augmentation de distance (None : règle du catalogue).
    pub distance_forcee: Option<bool>,
    pub article_distance: String,
    pub calendrier_applique: String,
    /// "EN_COURS" ou "ACCOMPLI". L'expiration est calculée à l'affichage (date système).
    #[serde(default = "statut_en_cours")]
    pub statut: String,
    pub date_accomplissement: Option<String>,
    pub distance_appliquee_mois: u32,
    pub saisie_manuelle: bool,
    pub alerte_envoyee: bool,
    pub date_alerte: Option<String>,
    /// Seuils (en jours) déjà notifiés par les alertes automatiques.
    pub alertes_auto_envoyees: Vec<i64>,
    pub date_creation: String,
}

impl Default for ActeDossier {
    fn default() -> Self {
        ActeDossier {
            id: String::new(),
            nature: String::new(),
            date_evenement: String::new(),
            type_delai: String::new(),
            libelle_delai: String::new(),
            fondement_cpc: String::new(),
            article_code: String::new(),
            article_num: String::new(),
            sanction: String::new(),
            date_echeance_theorique: String::new(),
            date_echeance_calculee: String::new(),
            details_calcul: String::new(),
            duree: Duree::default(),
            distance_forcee: None,
            article_distance: String::new(),
            calendrier_applique: String::new(),
            statut: statut_en_cours(),
            date_accomplissement: None,
            distance_appliquee_mois: 0,
            saisie_manuelle: false,
            alerte_envoyee: false,
            date_alerte: None,
            alertes_auto_envoyees: Vec::new(),
            date_creation: String::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Affaire {
    pub id: String,
    pub reference_cabinet: String,
    pub nom_dossier: String,
    pub client: String,
    pub partie_adverse: String,
    pub juridiction: String,
    pub numero_rg: String,
    #[serde(default = "residence")]
    pub lieu_residence_partie: String,
    /// Calendrier du siège de la juridiction ; vide = calendrier par défaut des paramètres.
    pub calendrier_juridiction: String,
    /// Noms ou mentions à masquer avant tout envoi au copilote IA (en plus des parties et du n° RG).
    pub termes_confidentiels: Vec<String>,
    pub enjeu_financier: f64,
    #[serde(default = "un")]
    pub etape_procedure: u8,
    /// "ACTIVE" ou "ARCHIVEE".
    #[serde(default = "statut_active")]
    pub statut: String,
    pub notes: String,
    pub date_creation: String,
    pub date_modification: String,
    pub diligences: Vec<Diligence>,
    pub actes: Vec<ActeDossier>,
}

impl Default for Affaire {
    fn default() -> Self {
        Affaire {
            id: String::new(),
            reference_cabinet: String::new(),
            nom_dossier: String::new(),
            client: String::new(),
            partie_adverse: String::new(),
            juridiction: String::new(),
            numero_rg: String::new(),
            lieu_residence_partie: residence(),
            calendrier_juridiction: String::new(),
            termes_confidentiels: Vec::new(),
            enjeu_financier: 0.0,
            etape_procedure: 1,
            statut: statut_active(),
            notes: String::new(),
            date_creation: String::new(),
            date_modification: String::new(),
            diligences: Vec::new(),
            actes: Vec::new(),
        }
    }
}

/// Champs saisis dans la fiche « Nouvelle affaire / Modifier l'affaire ».
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct SaisieAffaire {
    pub reference_cabinet: String,
    pub nom_dossier: String,
    pub client: String,
    pub partie_adverse: String,
    pub juridiction: String,
    pub numero_rg: String,
    pub lieu_residence_partie: String,
    pub calendrier_juridiction: String,
    pub termes_confidentiels: Vec<String>,
    pub enjeu_financier: f64,
    pub etape_procedure: u8,
    pub statut: String,
    pub notes: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct EntreeHistorique {
    pub id: String,
    pub horodatage: String,
    pub horodatage_iso: String,
    pub categorie: String,
    pub titre: String,
    pub details: String,
    pub fondement: String,
    pub id_affaire: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct DonneesApplication {
    /// Absent des fichiers antérieurs : vaut alors 0, ce qui déclenche la migration.
    #[serde(default)]
    pub version_schema: u32,
    pub parametres: ParametresApp,
    pub affaires: Vec<Affaire>,
    pub historique: Vec<EntreeHistorique>,
}

impl Default for DonneesApplication {
    fn default() -> Self {
        DonneesApplication {
            version_schema: VERSION_SCHEMA,
            parametres: ParametresApp::default(),
            affaires: Vec::new(),
            historique: Vec::new(),
        }
    }
}

/// Informations de stockage renvoyées à l'interface (chemins, avertissements de chargement).
#[derive(Serialize, Clone, Debug, Default)]
pub struct InfosStockage {
    pub chemin_donnees: String,
    pub dossier_sauvegardes: String,
    pub dossier_exports: String,
    pub avertissements: Vec<String>,
    /// Vrai si les clés API sont rangées dans le Gestionnaire d'identification Windows.
    pub cles_dans_coffre: bool,
    pub demarrage_windows: bool,
}

impl ParametresApp {
    /// Copie destinée à l'interface : les clés API sont remplacées par un masque.
    pub fn masques(&self) -> ParametresApp {
        let m = |v: &str| if v.trim().is_empty() { String::new() } else { MASQUE_SECRET.to_string() };
        ParametresApp {
            resend_api_key: m(&self.resend_api_key),
            openlegi_url: m(&self.openlegi_url),
            openlegi_jeton: m(&self.openlegi_jeton),
            gemini_api_key: m(&self.gemini_api_key),
            claude_api_key: m(&self.claude_api_key),
            ..self.clone()
        }
    }

    /// Fusionne une saisie de l'interface : un champ resté masqué conserve la clé enregistrée.
    pub fn fusionner_secrets(&mut self, actuels: &ParametresApp) {
        if self.resend_api_key == MASQUE_SECRET {
            self.resend_api_key = actuels.resend_api_key.clone();
        }
        if self.openlegi_url == MASQUE_SECRET {
            self.openlegi_url = actuels.openlegi_url.clone();
        }
        if self.openlegi_jeton == MASQUE_SECRET {
            self.openlegi_jeton = actuels.openlegi_jeton.clone();
        }
        if self.gemini_api_key == MASQUE_SECRET {
            self.gemini_api_key = actuels.gemini_api_key.clone();
        }
        if self.claude_api_key == MASQUE_SECRET {
            self.claude_api_key = actuels.claude_api_key.clone();
        }
    }

    /// Copie sans aucune clé (sauvegardes exportées hors du dossier de l'application).
    pub fn sans_secrets(&self) -> ParametresApp {
        ParametresApp {
            resend_api_key: String::new(),
            openlegi_url: String::new(),
            openlegi_jeton: String::new(),
            gemini_api_key: String::new(),
            claude_api_key: String::new(),
            ..self.clone()
        }
    }
}

impl DonneesApplication {
    pub fn pour_interface(&self) -> DonneesApplication {
        DonneesApplication { parametres: self.parametres.masques(), ..self.clone() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_masques_et_fusionnes() {
        let mut p = ParametresApp { gemini_api_key: "AIza-secret".into(), resend_api_key: String::new(), ..Default::default() };
        let m = p.masques();
        assert_eq!(m.gemini_api_key, MASQUE_SECRET);
        assert_eq!(m.resend_api_key, "");
        let mut saisie = m.clone();
        saisie.resend_api_key = "re_nouvelle".into();
        saisie.fusionner_secrets(&p);
        assert_eq!(saisie.gemini_api_key, "AIza-secret");
        assert_eq!(saisie.resend_api_key, "re_nouvelle");
        p.gemini_api_key = "x".into();
        assert!(p.sans_secrets().gemini_api_key.is_empty());
        let mut c = ParametresApp { claude_api_key: "sk-ant-secret".into(), ..Default::default() };
        assert_eq!(c.masques().claude_api_key, MASQUE_SECRET);
        let mut saisie_c = c.masques();
        saisie_c.fusionner_secrets(&c);
        assert_eq!(saisie_c.claude_api_key, "sk-ant-secret");
        c.claude_api_key = "y".into();
        assert!(c.sans_secrets().claude_api_key.is_empty());
    }
}
