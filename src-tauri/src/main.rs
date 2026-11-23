#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
mod langue;
mod dictionnaire;

use crate::langue::t;

mod alertes;
mod coffre;
mod competence;
mod cpc_engine;
mod demarrage;
mod documents;
mod ia;
mod legifrance;
mod modeles;
mod modeles_diligences;
mod openlegi;
mod pdf;
mod prescription;
mod regles;
mod resend_client;
mod stockage;
#[cfg(test)]
mod tests_http;

use chrono::{Local, NaiveDate};
use cpc_engine::{Calendrier, Duree, Residence};
use modeles::*;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use stockage::{journaliser, nouvel_id, Chemins};
use tauri::{Emitter, Manager, State};

pub struct AppState {
    chemins: Chemins,
    donnees: Mutex<DonneesApplication>,
    avertissements: Mutex<Vec<String>>,
    /// Dernière erreur d'envoi automatique déjà journalisée (évite de la répéter chaque heure).
    derniere_erreur_alerte: Mutex<String>,
}

const ETAPES: [&str; 5] = ["Pré-contentieux", "Introduction", "Mise en état", "Audience / délibéré", "Recours / exécution"];

/// Réponse standard des commandes qui modifient la base.
#[derive(Serialize)]
struct Reponse {
    donnees: DonneesApplication,
    id: Option<String>,
    message: Option<String>,
}

// -----------------------------------------------------------------------------
// OUTILS INTERNES
// -----------------------------------------------------------------------------

fn verrou(state: &AppState) -> Result<std::sync::MutexGuard<'_, DonneesApplication>, String> {
    state.donnees.lock().map_err(|_| "Verrou des données indisponible (état interne corrompu).".to_string())
}

/// Transaction : la modification est appliquée sur une copie, écrite sur disque,
/// puis seulement validée en mémoire. Un échec d'écriture ne laisse aucun état divergent.
fn transaction<F>(state: &AppState, f: F) -> Result<Reponse, String>
where
    F: FnOnce(&mut DonneesApplication) -> Result<(Option<String>, Option<String>), String>,
{
    let mut lock = verrou(state)?;
    let mut copie = lock.clone();
    let (id, message) = f(&mut copie)?;
    stockage::enregistrer(&state.chemins, &copie)?;
    *lock = copie;
    Ok(Reponse { donnees: lock.pour_interface(), id, message })
}

fn aujourd_hui() -> NaiveDate {
    Local::now().date_naive()
}

fn parse_date(s: &str, champ: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").map_err(|_| crate::tr!("{} : date invalide (format attendu AAAA-MM-JJ).", "{}: invalid date (expected format YYYY-MM-DD).", champ))
}

fn date_fr(iso: &str) -> String {
    NaiveDate::parse_from_str(iso, "%Y-%m-%d")
        .map(|d| d.format("%d/%m/%Y").to_string())
        .unwrap_or_else(|_| iso.to_string())
}

fn trouver_affaire<'a>(d: &'a mut DonneesApplication, id: &str) -> Result<&'a mut Affaire, String> {
    d.affaires.iter_mut().find(|a| a.id == id).ok_or_else(|| t("Dossier introuvable.").to_string())
}

fn normaliser_residence(code: &str) -> String {
    Residence::depuis_code(code).code().to_string()
}

/// Calendrier applicable à un dossier : celui du siège de sa juridiction, sinon celui par défaut.
fn calendrier_de(p: &ParametresApp, a: &Affaire) -> Calendrier {
    if a.calendrier_juridiction.trim().is_empty() {
        Calendrier::depuis_code(&p.calendrier_defaut)
    } else {
        Calendrier::depuis_code(&a.calendrier_juridiction)
    }
}

fn appliquer_saisie(a: &mut Affaire, s: &SaisieAffaire) -> Result<(), String> {
    let nom = s.nom_dossier.trim();
    if nom.is_empty() {
        return Err(t("Le nom du dossier (parties) est obligatoire.").to_string());
    }
    if !s.enjeu_financier.is_finite() || s.enjeu_financier < 0.0 {
        return Err(t("L'enjeu financier doit être un montant positif.").to_string());
    }
    a.nom_dossier = nom.to_string();
    a.reference_cabinet = s.reference_cabinet.trim().to_string();
    a.client = s.client.trim().to_string();
    a.partie_adverse = s.partie_adverse.trim().to_string();
    a.juridiction = s.juridiction.trim().to_string();
    a.numero_rg = s.numero_rg.trim().to_string();
    a.lieu_residence_partie = normaliser_residence(&s.lieu_residence_partie);
    a.calendrier_juridiction = if s.calendrier_juridiction.trim().is_empty() {
        String::new()
    } else {
        Calendrier::depuis_code(&s.calendrier_juridiction).code().to_string()
    };
    a.enjeu_financier = s.enjeu_financier;
    a.etape_procedure = s.etape_procedure.clamp(1, 5);
    a.statut = if s.statut.trim().eq_ignore_ascii_case("ARCHIVEE") { "ARCHIVEE".into() } else { "ACTIVE".into() };
    a.notes = s.notes.trim().to_string();
    let mut termes: Vec<String> = Vec::new();
    for t in s.termes_confidentiels.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
        if !termes.iter().any(|x| x.eq_ignore_ascii_case(t)) && termes.len() < 50 {
            termes.push(t.to_string());
        }
    }
    a.termes_confidentiels = termes;
    a.date_modification = stockage::horodatage_iso();
    Ok(())
}

#[derive(Serialize)]
struct ReponseCalcul {
    resultat: cpc_engine::ResultatCalculDelai,
    regle: regles::RegleDelai,
}

fn calculer(
    date_depart: &str,
    type_delai: &str,
    residence: &str,
    calendrier: Calendrier,
    appliquer_distance: Option<bool>,
    duree_libre: Option<Duree>,
) -> Result<ReponseCalcul, String> {
    let regle = regles::trouver(type_delai).ok_or_else(|| crate::tr!("Type de délai non reconnu : {}", "Unrecognised time-limit type: {}", type_delai))?;
    let duree = match regle.mode {
        "CALCULE" => regle.duree,
        "LIBRE" => {
            let d = duree_libre.unwrap_or_default();
            if d.est_nulle() {
                return Err(t("Délai libre : saisissez une durée (années, mois ou jours).").to_string());
            }
            if d.annees > 100 || d.mois > 1200 || d.jours > 36_500 {
                return Err(t("Délai libre : durée manifestement erronée.").to_string());
            }
            d
        }
        _ => return Err(t("Ce type d'échéance n'est pas calculé : saisissez la date fixée.").to_string()),
    };
    let depart = parse_date(date_depart, t("Date de départ"))?;
    let res = Residence::depuis_code(residence);
    let (mois_distance, article) = match appliquer_distance {
        Some(false) => (0, ""),
        Some(true) => cpc_engine::augmentation_distance(calendrier, res, true, true, regle.pourvoi),
        None => cpc_engine::augmentation_distance(calendrier, res, regle.distance_643, regle.distance_644, regle.pourvoi),
    };
    let resultat = cpc_engine::calculer_echeance(depart, duree, mois_distance, article, calendrier, aujourd_hui());
    Ok(ReponseCalcul { resultat, regle })
}

// -----------------------------------------------------------------------------
// COMMANDES : LECTURE
// -----------------------------------------------------------------------------

#[tauri::command]
fn charger_donnees(state: State<AppState>) -> Result<DonneesApplication, String> {
    Ok(verrou(&state)?.pour_interface())
}

#[tauri::command]
fn obtenir_infos_stockage(state: State<AppState>) -> InfosStockage {
    InfosStockage {
        chemin_donnees: state.chemins.fichier_donnees.display().to_string(),
        dossier_sauvegardes: state.chemins.dossier_sauvegardes.display().to_string(),
        dossier_exports: state.chemins.dossier_exports.display().to_string(),
        avertissements: state.avertissements.lock().map(|v| v.clone()).unwrap_or_default(),
        cles_dans_coffre: coffre::disponible(),
        demarrage_windows: demarrage::actif(),
    }
}

#[tauri::command]
fn lister_regles_delais() -> Vec<regles::RegleDelai> {
    regles::catalogue()
}

#[derive(Serialize)]
struct Referentiels {
    calendriers: Vec<cpc_engine::InfoCalendrier>,
    regimes_prescription: Vec<prescription::RegimePrescription>,
    matieres_competence: Vec<competence::Matiere>,
    modeles_diligences: Vec<modeles_diligences::ModeleDiligences>,
}

#[tauri::command]
fn lister_referentiels() -> Referentiels {
    Referentiels {
        calendriers: cpc_engine::CALENDRIERS.iter().map(|c| c.info()).collect(),
        regimes_prescription: prescription::regimes(),
        matieres_competence: competence::matieres(),
        modeles_diligences: modeles_diligences::modeles(),
    }
}

// -----------------------------------------------------------------------------
// COMMANDES : AFFAIRES
// -----------------------------------------------------------------------------

#[tauri::command]
fn creer_affaire(affaire: SaisieAffaire, state: State<AppState>) -> Result<Reponse, String> {
    transaction(&state, |d| {
        let mut a = Affaire { id: nouvel_id("aff"), date_creation: stockage::horodatage_iso(), ..Default::default() };
        appliquer_saisie(&mut a, &affaire)?;
        let id = a.id.clone();
        let titre = crate::tr!("Ouverture du dossier : {}", "Matter opened: {}", a.nom_dossier);
        let details = format!(
            "Réf. {} | {} | RG {}",
            if a.reference_cabinet.is_empty() { "-" } else { &a.reference_cabinet },
            if a.juridiction.is_empty() { "-" } else { &a.juridiction },
            if a.numero_rg.is_empty() { "-" } else { &a.numero_rg }
        );
        d.affaires.insert(0, a);
        journaliser(d, "DOSSIER", &titre, &details, "Gestion de cabinet", Some(id.clone()));
        Ok((Some(id), Some(t("Dossier créé.").into())))
    })
}

#[tauri::command]
fn modifier_affaire(id_affaire: String, affaire: SaisieAffaire, state: State<AppState>) -> Result<Reponse, String> {
    transaction(&state, |d| {
        let a = trouver_affaire(d, &id_affaire)?;
        appliquer_saisie(a, &affaire)?;
        let titre = crate::tr!("Modification du dossier : {}", "Matter edited: {}", a.nom_dossier);
        journaliser(d, "DOSSIER", &titre, t("Informations générales mises à jour"), "", Some(id_affaire.clone()));
        Ok((Some(id_affaire.clone()), Some(t("Dossier mis à jour.").into())))
    })
}

#[tauri::command]
fn supprimer_affaire(id_affaire: String, state: State<AppState>) -> Result<Reponse, String> {
    // Filet de sécurité : sauvegarde horodatée avant toute suppression de dossier.
    {
        let lock = verrou(&state)?;
        stockage::sauvegarde_horodatee(&state.chemins, &lock, "avant_suppression")?;
    }
    transaction(&state, |d| {
        let pos = d.affaires.iter().position(|a| a.id == id_affaire).ok_or_else(|| t("Dossier introuvable.").to_string())?;
        let a = d.affaires.remove(pos);
        journaliser(
            d,
            "DOSSIER",
            &crate::tr!("Suppression du dossier : {}", "Matter deleted: {}", a.nom_dossier),
            &crate::tr!("{} acte(s) et {} diligence(s) supprimés. Sauvegarde préalable créée.", "{} deadline(s) and {} task(s) deleted. Backup created first.", a.actes.len(), a.diligences.len()),
            "",
            Some(id_affaire.clone()),
        );
        Ok((None, Some(t("Dossier supprimé (une sauvegarde préalable a été créée).").into())))
    })
}

// -----------------------------------------------------------------------------
// COMMANDES : ACTES & ÉCHÉANCES
// -----------------------------------------------------------------------------

#[derive(Deserialize, Default)]
#[serde(default)]
struct SaisieActe {
    nature: String,
    date_depart: String,
    type_delai: String,
    date_echeance_manuelle: Option<String>,
    appliquer_distance: Option<bool>,
    duree_libre: Option<Duree>,
    fondement_libre: Option<String>,
    sanction_libre: Option<String>,
}

#[tauri::command]
fn ajouter_acte_au_dossier(id_affaire: String, acte: SaisieActe, state: State<AppState>) -> Result<Reponse, String> {
    transaction(&state, |d| {
        let nature = acte.nature.trim().to_string();
        if nature.is_empty() {
            return Err(t("La nature de l'acte est obligatoire.").to_string());
        }
        let code = regles::normaliser_code(&acte.type_delai);
        let regle = regles::trouver(&code).ok_or_else(|| crate::tr!("Type de délai non reconnu : {}", "Unrecognised time-limit type: {}", acte.type_delai))?;

        let parametres = d.parametres.clone();
        let (residence, calendrier) = {
            let a = trouver_affaire(d, &id_affaire)?;
            (a.lieu_residence_partie.clone(), calendrier_de(&parametres, a))
        };
        let mut nouvel = ActeDossier {
            id: nouvel_id("act"),
            nature: nature.clone(),
            type_delai: regle.code.to_string(),
            libelle_delai: regle.libelle.to_string(),
            fondement_cpc: regle.fondement.to_string(),
            article_code: regle.article_code.to_string(),
            article_num: regle.article_num.to_string(),
            sanction: regle.sanction.to_string(),
            date_creation: stockage::horodatage_iso(),
            calendrier_applique: calendrier.code().to_string(),
            distance_forcee: acte.appliquer_distance,
            ..Default::default()
        };
        if regle.mode == "LIBRE" {
            let texte = |v: &Option<String>| v.as_ref().map(|x| x.trim().to_string()).filter(|x| !x.is_empty());
            if let Some(f) = texte(&acte.fondement_libre) {
                nouvel.fondement_cpc = f;
            }
            if let Some(sx) = texte(&acte.sanction_libre) {
                nouvel.sanction = sx;
            }
        }

        if code == regles::CODE_ECHEANCE_FIXEE {
            let brut = acte.date_echeance_manuelle.clone().unwrap_or_default();
            let echeance = parse_date(&brut, t("Date de l'échéance fixée"))?;
            let evenement = if acte.date_depart.trim().is_empty() { echeance } else { parse_date(&acte.date_depart, t("Date de l'acte"))? };
            nouvel.date_evenement = evenement.format("%Y-%m-%d").to_string();
            nouvel.date_echeance_theorique = echeance.format("%Y-%m-%d").to_string();
            nouvel.date_echeance_calculee = nouvel.date_echeance_theorique.clone();
            nouvel.saisie_manuelle = true;
            nouvel.details_calcul = crate::tr!("Échéance fixée saisie manuellement : {}.", "Fixed deadline entered manually: {}.", echeance.format("%d/%m/%Y"));
            if cpc_engine::est_chome_642(echeance, calendrier) {
                nouvel.details_calcul.push_str(t(" Attention : cette date tombe un jour non ouvrable."));
            }
        } else {
            let calc = calculer(&acte.date_depart, &code, &residence, calendrier, acte.appliquer_distance, acte.duree_libre)?;
            nouvel.duree = calc.resultat.duree;
            nouvel.article_distance = calc.resultat.article_distance.clone();
            if regle.mode == "LIBRE" {
                nouvel.libelle_delai = crate::tr!("Délai libre de {}", "Custom time limit of {}", calc.resultat.duree.libelle());
            }
            nouvel.date_evenement = calc.resultat.date_declencheur.clone();
            nouvel.date_echeance_theorique = calc.resultat.date_echeance_theorique.clone();
            nouvel.date_echeance_calculee = calc.resultat.date_echeance_legale.clone();
            nouvel.details_calcul = calc.resultat.justification_pas_a_pas.clone();
            nouvel.distance_appliquee_mois = calc.resultat.mois_distance;
            if !regle.note.is_empty() {
                nouvel.details_calcul.push_str(&format!(" NB : {}", regle.note));
            }
        }

        let titre = crate::tr!("Échéance enregistrée : {} ({})", "Deadline saved: {} ({})", nature, nouvel.fondement_cpc);
        let details = crate::tr!("Échéance au {} | {}", "Deadline on {} | {}", date_fr(&nouvel.date_echeance_calculee), nouvel.details_calcul);
        let fondement = nouvel.fondement_cpc.clone();
        let id = nouvel.id.clone();
        let a = trouver_affaire(d, &id_affaire)?;
        a.actes.push(nouvel);
        a.actes.sort_by(|x, y| x.date_echeance_calculee.cmp(&y.date_echeance_calculee));
        a.date_modification = stockage::horodatage_iso();
        journaliser(d, "DELAI_CPC", &titre, &details, &fondement, Some(id_affaire.clone()));
        Ok((Some(id), Some(t("Échéance enregistrée.").into())))
    })
}

#[tauri::command]
fn supprimer_acte(id_affaire: String, id_acte: String, state: State<AppState>) -> Result<Reponse, String> {
    transaction(&state, |d| {
        let a = trouver_affaire(d, &id_affaire)?;
        let pos = a.actes.iter().position(|x| x.id == id_acte).ok_or_else(|| t("Acte introuvable.").to_string())?;
        let acte = a.actes.remove(pos);
        a.date_modification = stockage::horodatage_iso();
        journaliser(
            d,
            "DELAI_CPC",
            &crate::tr!("Suppression de l'échéance : {}", "Deadline deleted: {}", acte.nature),
            &crate::tr!("Échéance supprimée : {} ({})", "Deadline deleted: {} ({})", date_fr(&acte.date_echeance_calculee), acte.fondement_cpc),
            &acte.fondement_cpc,
            Some(id_affaire.clone()),
        );
        Ok((None, Some(t("Échéance supprimée.").into())))
    })
}

#[tauri::command]
fn definir_statut_acte(id_affaire: String, id_acte: String, accompli: bool, state: State<AppState>) -> Result<Reponse, String> {
    transaction(&state, |d| {
        let a = trouver_affaire(d, &id_affaire)?;
        let acte = a.actes.iter_mut().find(|x| x.id == id_acte).ok_or_else(|| t("Acte introuvable.").to_string())?;
        acte.statut = if accompli { "ACCOMPLI".into() } else { "EN_COURS".into() };
        acte.date_accomplissement = if accompli { Some(stockage::horodatage_iso()) } else { None };
        let titre = format!(
            "{} : {}",
            if accompli { t("Diligence accomplie") } else { t("Échéance rouverte") },
            acte.nature
        );
        let details = crate::tr!("Échéance du {}", "Deadline of {}", date_fr(&acte.date_echeance_calculee));
        let fondement = acte.fondement_cpc.clone();
        a.date_modification = stockage::horodatage_iso();
        journaliser(d, "DELAI_CPC", &titre, &details, &fondement, Some(id_affaire.clone()));
        Ok((None, None))
    })
}

// -----------------------------------------------------------------------------
// COMMANDES : DILIGENCES (CHECKLIST)
// -----------------------------------------------------------------------------

#[tauri::command]
fn ajouter_diligence(id_affaire: String, libelle: String, state: State<AppState>) -> Result<Reponse, String> {
    transaction(&state, |d| {
        let libelle = libelle.trim().to_string();
        if libelle.is_empty() {
            return Err(t("Le libellé de la diligence est vide.").to_string());
        }
        let a = trouver_affaire(d, &id_affaire)?;
        let id = nouvel_id("dil");
        a.diligences.push(Diligence { id: id.clone(), libelle: libelle.clone(), fait: false, date_fait: None });
        a.date_modification = stockage::horodatage_iso();
        journaliser(d, "DILIGENCE", &crate::tr!("Diligence ajoutée : {}", "Task added: {}", libelle), "", "", Some(id_affaire.clone()));
        Ok((Some(id), None))
    })
}

#[tauri::command]
fn basculer_diligence(id_affaire: String, id_diligence: String, fait: bool, state: State<AppState>) -> Result<Reponse, String> {
    transaction(&state, |d| {
        let a = trouver_affaire(d, &id_affaire)?;
        let dil = a.diligences.iter_mut().find(|x| x.id == id_diligence).ok_or_else(|| t("Diligence introuvable.").to_string())?;
        dil.fait = fait;
        dil.date_fait = if fait { Some(stockage::horodatage_iso()) } else { None };
        let titre = crate::tr!("Diligence {} : {}", "Task {}: {}", if fait { t("cochée") } else { t("décochée") }, dil.libelle);
        a.date_modification = stockage::horodatage_iso();
        journaliser(d, "DILIGENCE", &titre, "", "", Some(id_affaire.clone()));
        Ok((None, None))
    })
}

#[tauri::command]
fn supprimer_diligence(id_affaire: String, id_diligence: String, state: State<AppState>) -> Result<Reponse, String> {
    transaction(&state, |d| {
        let a = trouver_affaire(d, &id_affaire)?;
        let pos = a.diligences.iter().position(|x| x.id == id_diligence).ok_or_else(|| t("Diligence introuvable.").to_string())?;
        let dil = a.diligences.remove(pos);
        a.date_modification = stockage::horodatage_iso();
        journaliser(d, "DILIGENCE", &crate::tr!("Diligence supprimée : {}", "Task deleted: {}", dil.libelle), "", "", Some(id_affaire.clone()));
        Ok((None, None))
    })
}

// -----------------------------------------------------------------------------
// COMMANDES : CALCULATEURS (MOTEUR RUST, JOURNALISÉ)
// -----------------------------------------------------------------------------

#[tauri::command]
fn calculer_delai(
    date_depart: String,
    type_delai: String,
    residence: String,
    calendrier: Option<String>,
    appliquer_distance: Option<bool>,
    duree_libre: Option<Duree>,
    journaliser_calcul: bool,
    state: State<AppState>,
) -> Result<ReponseCalcul, String> {
    let cal = match calendrier {
        Some(c) if !c.trim().is_empty() => Calendrier::depuis_code(&c),
        _ => Calendrier::depuis_code(&verrou(&state)?.parametres.calendrier_defaut),
    };
    let calc = calculer(&date_depart, &type_delai, &residence, cal, appliquer_distance, duree_libre)?;
    if journaliser_calcul {
        let titre = if calc.regle.mode == "LIBRE" {
            crate::tr!("Calcul express : délai libre de {}", "Quick calculation: custom time limit of {}", calc.resultat.duree.libelle())
        } else {
            crate::tr!("Calcul express : {}", "Quick calculation: {}", calc.regle.libelle)
        };
        let details = crate::tr!(
            "Départ {} -> échéance {} (calendrier : {}) | {}", "Start {} -> deadline {} (calendar: {}) | {}",
            date_fr(&calc.resultat.date_declencheur),
            date_fr(&calc.resultat.date_echeance_legale),
            cal.libelle(),
            calc.resultat.justification_pas_a_pas
        );
        let fondement = format!("{} ; {}", calc.regle.fondement, calc.resultat.fondement_legal);
        let _ = transaction(&state, |d| {
            journaliser(d, "CALCUL", &titre, &details, &fondement, None);
            Ok((None, None))
        });
    }
    Ok(calc)
}

#[derive(Serialize)]
struct EtapeRebours {
    jours_avant: i64,
    date_brute: String,
    date_retenue: String,
    jours_anticipes: i64,
}

/// Délais « à rebours » : une date non ouvrable est anticipée au dernier jour ouvrable précédent
/// (approche prudente : la prorogation de l'art. 642 n'est pas transposée aux délais à rebours).
#[tauri::command]
fn calculer_rebours(date_reference: String, jours_avant: Vec<i64>, calendrier: Option<String>, state: State<AppState>) -> Result<Vec<EtapeRebours>, String> {
    let cal = match calendrier {
        Some(c) if !c.trim().is_empty() => Calendrier::depuis_code(&c),
        _ => Calendrier::depuis_code(&verrou(&state)?.parametres.calendrier_defaut),
    };
    let reference = parse_date(&date_reference, t("Date de référence"))?;
    Ok(jours_avant
        .into_iter()
        .map(|j| {
            let brute = reference - chrono::Duration::days(j);
            let (retenue, n) = cpc_engine::anticiper_jour_ouvrable(brute, cal);
            EtapeRebours {
                jours_avant: j,
                date_brute: brute.format("%Y-%m-%d").to_string(),
                date_retenue: retenue.format("%Y-%m-%d").to_string(),
                jours_anticipes: n,
            }
        })
        .collect())
}

// -----------------------------------------------------------------------------
// COMMANDES : RECALCUL DES ÉCHÉANCES (calendrier, résidence ou règle modifiés)
// -----------------------------------------------------------------------------

#[derive(Serialize)]
struct ChangementEcheance {
    id_affaire: String,
    nom_dossier: String,
    id_acte: String,
    nature: String,
    ancienne: String,
    nouvelle: String,
}

#[derive(Serialize)]
struct RapportRecalcul {
    changements: Vec<ChangementEcheance>,
    ignores: usize,
    donnees: Option<DonneesApplication>,
}

/// Recalcule les échéances en cours (non accomplies, non saisies manuellement) avec le calendrier
/// et la résidence actuels. `appliquer = false` : simple simulation, rien n'est modifié.
fn recalcul(d: &mut DonneesApplication, id_affaire: &Option<String>, appliquer: bool) -> (Vec<ChangementEcheance>, usize) {
    let parametres = d.parametres.clone();
    let mut changements = Vec::new();
    let mut ignores = 0;
    for a in d.affaires.iter_mut() {
        if let Some(id) = id_affaire {
            if &a.id != id {
                continue;
            }
        }
        let cal = calendrier_de(&parametres, a);
        let residence = a.lieu_residence_partie.clone();
        let nom = a.nom_dossier.clone();
        for acte in a.actes.iter_mut() {
            if acte.statut == "ACCOMPLI" || acte.saisie_manuelle {
                continue;
            }
            let Some(regle) = regles::trouver(&acte.type_delai) else {
                ignores += 1;
                continue;
            };
            let libre = if regle.mode == "LIBRE" { Some(acte.duree) } else { None };
            let Ok(calc) = calculer(&acte.date_evenement, regle.code, &residence, cal, acte.distance_forcee, libre) else {
                ignores += 1;
                continue;
            };
            if calc.resultat.date_echeance_legale != acte.date_echeance_calculee {
                changements.push(ChangementEcheance {
                    id_affaire: a.id.clone(),
                    nom_dossier: nom.clone(),
                    id_acte: acte.id.clone(),
                    nature: acte.nature.clone(),
                    ancienne: acte.date_echeance_calculee.clone(),
                    nouvelle: calc.resultat.date_echeance_legale.clone(),
                });
            }
            if appliquer {
                acte.date_echeance_theorique = calc.resultat.date_echeance_theorique.clone();
                acte.date_echeance_calculee = calc.resultat.date_echeance_legale.clone();
                acte.details_calcul = calc.resultat.justification_pas_a_pas.clone();
                if !regle.note.is_empty() {
                    acte.details_calcul.push_str(&format!(" NB : {}", regle.note));
                }
                acte.distance_appliquee_mois = calc.resultat.mois_distance;
                acte.article_distance = calc.resultat.article_distance.clone();
                acte.calendrier_applique = cal.code().to_string();
                acte.duree = calc.resultat.duree;
            }
        }
        if appliquer {
            a.actes.sort_by(|x, y| x.date_echeance_calculee.cmp(&y.date_echeance_calculee));
        }
    }
    (changements, ignores)
}

#[tauri::command]
fn recalculer_echeances(id_affaire: Option<String>, appliquer: bool, state: State<AppState>) -> Result<RapportRecalcul, String> {
    if !appliquer {
        let mut copie = verrou(&state)?.clone();
        let (changements, ignores) = recalcul(&mut copie, &id_affaire, false);
        return Ok(RapportRecalcul { changements, ignores, donnees: None });
    }
    let mut resultat: Option<(Vec<ChangementEcheance>, usize)> = None;
    let rep = transaction(&state, |d| {
        let (changements, ignores) = recalcul(d, &id_affaire, true);
        let details = if changements.is_empty() {
            t("Aucune échéance modifiée.").to_string()
        } else {
            changements
                .iter()
                .map(|c| format!("{} / {} : {} -> {}", c.nom_dossier, c.nature, date_fr(&c.ancienne), date_fr(&c.nouvelle)))
                .collect::<Vec<_>>()
                .join(" | ")
        };
        journaliser(d, "DELAI_CPC", &crate::tr!("Recalcul des échéances ({} modification(s))", "Deadlines recalculated ({} change(s))", changements.len()), &details, "Art. 640 à 644 CPC", id_affaire.clone());
        resultat = Some((changements, ignores));
        Ok((None, None))
    })?;
    let (changements, ignores) = resultat.unwrap_or_default();
    Ok(RapportRecalcul { changements, ignores, donnees: Some(rep.donnees) })
}

// -----------------------------------------------------------------------------
// COMMANDES : PRESCRIPTION, COMPÉTENCE, MODÈLES DE DILIGENCES
// -----------------------------------------------------------------------------

#[tauri::command]
fn analyser_prescription(demande: prescription::Demande) -> Result<prescription::Resultat, String> {
    prescription::analyser(&demande)
}

#[tauri::command]
fn orienter_competence(demande: competence::Demande) -> Result<competence::Resultat, String> {
    competence::orienter(&demande)
}

#[tauri::command]
fn appliquer_modele_diligences(id_affaire: String, code_modele: String, state: State<AppState>) -> Result<Reponse, String> {
    let modele = modeles_diligences::modeles()
        .into_iter()
        .find(|m| m.code == code_modele)
        .ok_or_else(|| t("Modèle de diligences introuvable.").to_string())?;
    transaction(&state, |d| {
        let a = trouver_affaire(d, &id_affaire)?;
        let mut ajoutes = 0;
        for libelle in &modele.elements {
            if a.diligences.iter().any(|x| x.libelle.eq_ignore_ascii_case(libelle)) {
                continue;
            }
            a.diligences.push(Diligence { id: nouvel_id("dil"), libelle: libelle.to_string(), fait: false, date_fait: None });
            ajoutes += 1;
        }
        a.date_modification = stockage::horodatage_iso();
        journaliser(
            d,
            "DILIGENCE",
            &crate::tr!("Modèle de diligences appliqué : {}", "Task template applied: {}", modele.libelle),
            &crate::tr!("{} diligence(s) ajoutée(s)", "{} task(s) added", ajoutes),
            "",
            Some(id_affaire.clone()),
        );
        Ok((None, Some(crate::tr!("{} diligence(s) ajoutée(s).", "{} task(s) added.", ajoutes))))
    })
}

// -----------------------------------------------------------------------------
// COMMANDES : JOURNAL D'AUDIT
// -----------------------------------------------------------------------------

#[tauri::command]
fn enregistrer_historique(
    categorie: String,
    titre: String,
    details: String,
    fondement: String,
    id_affaire: Option<String>,
    state: State<AppState>,
) -> Result<Reponse, String> {
    transaction(&state, |d| {
        journaliser(d, &categorie, &titre, &details, &fondement, id_affaire.clone());
        Ok((None, None))
    })
}

#[tauri::command]
fn exporter_journal(state: State<AppState>) -> Result<String, String> {
    let lock = verrou(&state)?;
    let mut txt = String::new();
    txt.push_str(if langue::en() { "ACTIVITY LOG - DEADLINE BREAKER\r\n" } else { "JOURNAL D'ACTIVITÉ - DEADLINE BREAKER\r\n" });
    if !lock.parametres.nom_cabinet.trim().is_empty() {
        txt.push_str(&format!("Cabinet : {}\r\n", lock.parametres.nom_cabinet.trim()));
    }
    txt.push_str(&format!("Export du {}\r\n", stockage::horodatage_affiche()));
    txt.push_str(&crate::tr!("{} entrée(s)\r\n", "{} entr(y/ies)\r\n", lock.historique.len()));
    txt.push_str("============================================================\r\n\r\n");
    for e in lock.historique.iter() {
        let dossier = e
            .id_affaire
            .as_ref()
            .and_then(|id| lock.affaires.iter().find(|a| &a.id == id))
            .map(|a| format!(" | Dossier : {}", a.nom_dossier))
            .unwrap_or_default();
        txt.push_str(&format!("[{}] {} - {}{}\r\n", e.horodatage, e.categorie, e.titre, dossier));
        if !e.details.is_empty() {
            txt.push_str(&format!("    {}\r\n", e.details));
        }
        if !e.fondement.is_empty() {
            txt.push_str(&format!("    Fondement : {}\r\n", e.fondement));
        }
        txt.push_str("\r\n");
    }
    let nom = format!("Activity_log_DeadlineBreaker_{}.txt", Local::now().format("%Y%m%d_%H%M%S"));
    let mut octets = vec![0xEF, 0xBB, 0xBF]; // BOM : lisible par le Bloc-notes Windows
    octets.extend_from_slice(txt.as_bytes());
    let chemin = stockage::ecrire_fichier(&state.chemins.dossier_exports, &nom, &octets)?;
    Ok(chemin.display().to_string())
}

// -----------------------------------------------------------------------------
// COMMANDES : PARAMÈTRES & CONNECTEURS
// -----------------------------------------------------------------------------

#[tauri::command]
fn enregistrer_parametres(nouveaux_parametres: ParametresApp, state: State<AppState>) -> Result<Reponse, String> {
    let mut p = nouveaux_parametres;
    if p.seuil_rouge_jours < 0 || p.seuil_orange_jours <= p.seuil_rouge_jours {
        return Err(t("Le seuil Orange doit être strictement supérieur au seuil Rouge (lui-même positif).").to_string());
    }
    p.emails_destinataires = p
        .emails_destinataires
        .iter()
        .map(|e| e.trim().to_string())
        .filter(|e| !e.is_empty())
        .collect();
    if let Some(invalide) = p.emails_destinataires.iter().find(|e| !e.contains('@') || e.contains(' ')) {
        return Err(crate::tr!("Adresse e-mail invalide : {}", "Invalid e-mail address: {}", invalide));
    }
    if p.gemini_model.trim().is_empty() {
        p.gemini_model = "gemini-2.5-flash".into();
    }
    p.alertes_jours = alertes::seuils_normalises(&p.alertes_jours);
    if p.alertes_auto && p.alertes_jours.is_empty() {
        return Err(t("Indiquez au moins un seuil d'alerte (ex. : 7, 2) ou désactivez les alertes automatiques.").into());
    }
    if p.claude_model.trim().is_empty() {
        p.claude_model = ia::MODELE_CLAUDE_DEFAUT.into();
    }
    p.demarrage_windows = demarrage::actif();
    p.langue = if p.langue.trim().eq_ignore_ascii_case("fr") { "fr".into() } else { "en".into() };
    langue::definir(&p.langue);
    p.ia_fournisseur = if ia::Fournisseur::depuis(&p.ia_fournisseur) == ia::Fournisseur::Claude { "CLAUDE".into() } else { "GEMINI".into() };
    let cal = Calendrier::depuis_code(&p.calendrier_defaut);
    p.calendrier_defaut = cal.code().to_string();
    p.regime_alsace_moselle = cal == Calendrier::AlsaceMoselle;
    transaction(&state, |d| {
        let calendrier_change = d.parametres.calendrier_defaut != p.calendrier_defaut;
        let mut p = p.clone();
        p.fusionner_secrets(&d.parametres);
        d.parametres = p.clone();
        let mut details = crate::tr!(
            "Seuils {} j / {} j ; calendrier par défaut : {} ; destinataires : {}", "Thresholds {} d / {} d; default calendar: {}; recipients: {}",
            p.seuil_rouge_jours,
            p.seuil_orange_jours,
            cal.libelle(),
            p.emails_destinataires.len()
        );
        if calendrier_change {
            details.push_str(t(" ; calendrier modifié : vérifier le recalcul des échéances en cours."));
        }
        journaliser(d, "SYSTEME", t("Paramètres du cabinet enregistrés"), &details, "", None);
        Ok((None, Some(t("Paramètres enregistrés.").into())))
    })
}

#[tauri::command]
async fn declencher_test_email(state: State<'_, AppState>) -> Result<String, String> {
    let (cle, exp, dest, cabinet) = {
        let lock = state.donnees.lock().map_err(|_| "Verrou indisponible".to_string())?;
        (
            lock.parametres.resend_api_key.clone(),
            lock.parametres.resend_expediteur.clone(),
            lock.parametres.emails_destinataires.clone(),
            lock.parametres.nom_cabinet.clone(),
        )
    };
    let alerte = resend_client::AlerteEmail {
        nom_affaire: if cabinet.trim().is_empty() { "Test Deadline Breaker" } else { cabinet.trim() },
        juridiction: "-",
        numero_rg: "-",
        acte: t("Vérification de la passerelle d'alertes"),
        date_limite: &Local::now().format("%d/%m/%Y").to_string(),
        jours_restants: 0,
        fondement: "Art. 640 et s. CPC",
        sanction: "-",
    };
    let res = resend_client::envoyer_alerte_resend(&cle, &exp, dest, &alerte, true).await;
    if let Ok(mut lock) = state.donnees.lock() {
        let mut copie = lock.clone();
        journaliser(
            &mut copie,
            "ALERTE",
            t("Test d'envoi d'e-mail"),
            &match &res {
                Ok(m) => m.clone(),
                Err(e) => crate::tr!("Échec : {}", "Failed: {}", e),
            },
            "",
            None,
        );
        if stockage::enregistrer(&state.chemins, &copie).is_ok() {
            *lock = copie;
        }
    }
    res
}

#[tauri::command]
async fn envoyer_alerte_acte(id_affaire: String, id_acte: String, state: State<'_, AppState>) -> Result<Reponse, String> {
    let (cle, exp, dest, aff, acte) = {
        let lock = state.donnees.lock().map_err(|_| "Verrou indisponible".to_string())?;
        let aff = lock.affaires.iter().find(|a| a.id == id_affaire).cloned().ok_or_else(|| t("Dossier introuvable.").to_string())?;
        let acte = aff.actes.iter().find(|x| x.id == id_acte).cloned().ok_or_else(|| t("Acte introuvable.").to_string())?;
        (
            lock.parametres.resend_api_key.clone(),
            lock.parametres.resend_expediteur.clone(),
            lock.parametres.emails_destinataires.clone(),
            aff,
            acte,
        )
    };
    let echeance = parse_date(&acte.date_echeance_calculee, t("Échéance"))?;
    let jours = (echeance - aujourd_hui()).num_days();
    let date_txt = date_fr(&acte.date_echeance_calculee);
    let alerte = resend_client::AlerteEmail {
        nom_affaire: &aff.nom_dossier,
        juridiction: &aff.juridiction,
        numero_rg: &aff.numero_rg,
        acte: &acte.nature,
        date_limite: &date_txt,
        jours_restants: jours,
        fondement: &acte.fondement_cpc,
        sanction: &acte.sanction,
    };
    let message = resend_client::envoyer_alerte_resend(&cle, &exp, dest, &alerte, false).await?;
    transaction(&state, |d| {
        let a = trouver_affaire(d, &id_affaire)?;
        if let Some(x) = a.actes.iter_mut().find(|x| x.id == id_acte) {
            x.alerte_envoyee = true;
            x.date_alerte = Some(stockage::horodatage_iso());
        }
        journaliser(
            d,
            "ALERTE",
            &crate::tr!("Alerte e-mail envoyée : {}", "E-mail alert sent: {}", acte.nature),
            &crate::tr!("Échéance du {} | {}", "Deadline of {} | {}", date_txt, message),
            &acte.fondement_cpc,
            Some(id_affaire.clone()),
        );
        Ok((None, Some(message.clone())))
    })
}

// -----------------------------------------------------------------------------
// COMMANDES : LÉGIFRANCE (via OpenLegi) ET LIENS VERS LEGIFRANCE.GOUV.FR
// -----------------------------------------------------------------------------

fn connexion_openlegi(state: &State<'_, AppState>, url: Option<String>, jeton: Option<String>) -> Result<openlegi::Connexion, String> {
    let lock = verrou(state)?;
    let choisir = |saisi: Option<String>, enregistre: &str| match saisi {
        Some(v) if v != MASQUE_SECRET => v,
        _ => enregistre.to_string(),
    };
    let url = choisir(url, &lock.parametres.openlegi_url);
    let jeton = choisir(jeton, &lock.parametres.openlegi_jeton);
    openlegi::Connexion::depuis(&url, &jeton)
}

#[tauri::command]
async fn tester_openlegi(url: Option<String>, jeton: Option<String>, state: State<'_, AppState>) -> Result<String, String> {
    let cx = connexion_openlegi(&state, url, jeton).map_err(|e| if e == "NON_CONFIGURE" { t("Renseignez l'adresse du serveur OpenLegi.").to_string() } else { e })?;
    let a = openlegi::consulter_article(&cx, "Code de procédure civile", "641").await?;
    Ok(crate::tr!(
        "Connexion OpenLegi opérationnelle : article 641 du CPC lu sur Légifrance (état : {}).", "OpenLegi connection working: article 641 CPC read from Légifrance (status: {}).",
        if a.etat.is_empty() { t("non précisé").to_string() } else { a.etat }
    ))
}

/// Article en vigueur via OpenLegi (cache local de 7 jours ; copie locale si le service est indisponible).
/// Erreur « NON_CONFIGURE » : OpenLegi non configuré (l'interface affiche alors la base locale).
#[tauri::command]
async fn consulter_article(code: String, num: String, forcer: bool, state: State<'_, AppState>) -> Result<legifrance::ArticleConsulte, String> {
    let cx = connexion_openlegi(&state, None, None)?;
    let (code_int, _, libelle) = legifrance::code_connu(&code).ok_or_else(|| crate::tr!("Code non pris en charge : {}", "Unsupported code: {}", code))?;
    let cle = legifrance::cle(&code, &num);
    let mut cache = legifrance::lire_cache(&state.chemins.fichier_cache_articles);
    if !forcer {
        if let Some(a) = cache.get(&cle) {
            if legifrance::cache_frais(a, 7) {
                let mut a = a.clone();
                a.source = "CACHE".into();
                return Ok(a);
            }
        }
    }
    match openlegi::consulter_article(&cx, libelle, &num).await {
        Ok(mut a) => {
            a.code = code_int.into();
            a.code_libelle = libelle.into();
            a.source = "OPENLEGI".into();
            a.recupere_le = Local::now().format("%Y-%m-%d %H:%M").to_string();
            cache.insert(cle, a.clone());
            legifrance::ecrire_cache(&state.chemins.fichier_cache_articles, &cache);
            let titre = crate::tr!("Consultation Légifrance : art. {} - {}", "Légifrance lookup: art. {} - {}", a.num, a.code_libelle);
            let details = crate::tr!("État : {} ; en vigueur depuis le {}", "Status: {}; in force since {}", a.etat, a.date_debut.as_deref().map(date_fr).unwrap_or_else(|| "-".into()));
            let _ = transaction(&state, |d| {
                journaliser(d, "LEGIFRANCE", &titre, &details, t("Données Légifrance via OpenLegi"), None);
                Ok((None, None))
            });
            Ok(a)
        }
        Err(msg) => match cache.get(&cle) {
            Some(a) => {
                let mut a = a.clone();
                a.source = "CACHE".into();
                a.avertissement = Some(crate::tr!("OpenLegi indisponible ({}). Copie locale du {}.", "OpenLegi unavailable ({}). Local copy from {}.", msg, a.recupere_le));
                Ok(a)
            }
            None => Err(msg),
        },
    }
}

/// Ouvre une page de legifrance.gouv.fr dans le navigateur par défaut (seul ce site est autorisé).
#[tauri::command]
fn ouvrir_legifrance(url: String) -> Result<(), String> {
    if !legifrance::url_autorisee(&url) {
        return Err(t("Seules les pages de legifrance.gouv.fr peuvent être ouvertes.").into());
    }
    ouvrir_avec_le_systeme(&url).map_err(|e| crate::tr!("Ouverture du navigateur impossible : {}", "Could not open the browser: {}", e))
}

/// Ouvre une adresse ou un fichier avec l'application par défaut du système.
fn ouvrir_avec_le_systeme(cible: &str) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    let r = std::process::Command::new("explorer").arg(cible).spawn();
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(cible).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(cible).spawn();
    r.map(|_| ())
}

const LIB_CATEGORIES: [(&str, &str); 8] = [
    ("DOSSIER", "Dossier"), ("DELAI_CPC", "Échéance"), ("CALCUL", "Calcul"), ("DILIGENCE", "Diligence"),
    ("ALERTE", "Alerte"), ("IA", "Copilote IA"), ("LEGIFRANCE", "Légifrance"), ("SYSTEME", "Système"),
];

fn libelle_categorie(code: &str) -> &str {
    LIB_CATEGORIES.iter().find(|(c, _)| *c == code).map(|&(_, l)| t(l)).unwrap_or(code)
}

/// Mise en page PDF du journal (filtré comme à l'écran, ou limité à un dossier).
fn journal_pdf(d: &DonneesApplication, categorie: &str, recherche: &str, id_affaire: Option<&str>) -> (Vec<u8>, usize) {
    let noms: std::collections::HashMap<&str, &str> = d.affaires.iter().map(|a| (a.id.as_str(), a.nom_dossier.as_str())).collect();
    let q = recherche.trim().to_lowercase();
    let entrees: Vec<&EntreeHistorique> = d
        .historique
        .iter()
        .filter(|e| categorie.is_empty() || e.categorie == categorie)
        .filter(|e| id_affaire.map_or(true, |id| e.id_affaire.as_deref() == Some(id)))
        .filter(|e| {
            let nom = e.id_affaire.as_deref().and_then(|id| noms.get(id)).copied().unwrap_or("");
            q.is_empty() || [e.titre.as_str(), &e.details, &e.fondement, nom].join(" ").to_lowercase().contains(&q)
        })
        .collect();
    let mut doc = pdf::Document::nouveau(if langue::en() { "Deadline Breaker - Activity log" } else { "Deadline Breaker - Journal d'activité" });
    let titre = match id_affaire.and_then(|id| noms.get(id)) {
        Some(nom) => crate::tr!("Journal du dossier {}", "Matter log: {}", nom),
        None => t("Journal d'activité").to_string(),
    };
    doc.paragraphe(&titre, pdf::TITRE, 0.0);
    doc.espace(4.0);
    if !d.parametres.nom_cabinet.trim().is_empty() {
        doc.paragraphe(d.parametres.nom_cabinet.trim(), pdf::SOUS_TITRE, 0.0);
    }
    let mut filtres = vec![crate::tr!("Export du {}", "Exported on {}", stockage::horodatage_affiche()), crate::tr!("{} entrée(s)", "{} entr(y/ies)", entrees.len())];
    if !categorie.is_empty() {
        filtres.push(crate::tr!("catégorie : {}", "category: {}", libelle_categorie(categorie)));
    }
    if !q.is_empty() {
        filtres.push(crate::tr!("recherche : « {} »", "search: \"{}\"", recherche.trim()));
    }
    doc.paragraphe(&filtres.join(" · "), pdf::SOUS_TITRE, 0.0);
    doc.espace(6.0);
    doc.trait_horizontal();
    if entrees.is_empty() {
        doc.paragraphe(t("Aucune entrée."), pdf::TEXTE, 0.0);
    }
    for e in &entrees {
        let nom = e.id_affaire.as_deref().and_then(|id| noms.get(id)).filter(|_| id_affaire.is_none());
        let entete = format!("{} · {}{}", e.horodatage, libelle_categorie(&e.categorie), nom.map(|n| format!(" · {}", n)).unwrap_or_default());
        let hauteur = doc.hauteur(&entete, pdf::DISCRET, 0.0) + doc.hauteur(&e.titre, pdf::GRAS, 0.0) + doc.hauteur(&e.details, pdf::TEXTE, 0.0);
        doc.garder_ensemble(hauteur.min(160.0) + 14.0);
        doc.paragraphe(&entete, pdf::DISCRET, 0.0);
        doc.paragraphe(&e.titre, pdf::GRAS, 0.0);
        if !e.details.trim().is_empty() {
            doc.paragraphe(e.details.trim(), pdf::TEXTE, 0.0);
        }
        if !e.fondement.trim().is_empty() {
            doc.paragraphe(&crate::tr!("Fondement : {}", "Legal basis: {}", e.fondement.trim()), pdf::DISCRET, 0.0);
        }
        doc.trait_horizontal();
    }
    (doc.produire(), entrees.len())
}

#[tauri::command]
fn exporter_journal_pdf(
    categorie: Option<String>,
    recherche: Option<String>,
    id_affaire: Option<String>,
    ouvrir: bool,
    state: State<AppState>,
) -> Result<String, String> {
    let (octets, nom) = {
        let lock = verrou(&state)?;
        let (octets, _) = journal_pdf(&lock, categorie.as_deref().unwrap_or(""), recherche.as_deref().unwrap_or(""), id_affaire.as_deref());
        let nom = match id_affaire.as_deref().and_then(|id| lock.affaires.iter().find(|a| a.id == id)) {
            Some(a) => format!("Journal_{}_{}.pdf", stockage::nom_fichier_sur(&a.nom_dossier), Local::now().format("%Y%m%d_%H%M%S")),
            None => format!("{}_DeadlineBreaker_{}.pdf", if langue::en() { "Activity_log" } else { "Journal_activite" }, Local::now().format("%Y%m%d_%H%M%S")),
        };
        (octets, nom)
    };
    let chemin = stockage::ecrire_fichier(&state.chemins.dossier_exports, &nom, &octets)?;
    if ouvrir {
        let _ = ouvrir_avec_le_systeme(&chemin.display().to_string());
    }
    Ok(chemin.display().to_string())
}

/// Changement de langue immédiat (interface et textes produits par le moteur).
#[tauri::command]
fn definir_langue(code: String, state: State<AppState>) -> Result<Reponse, String> {
    let code = if code.trim().eq_ignore_ascii_case("fr") { "fr" } else { "en" };
    langue::definir(code);
    transaction(&state, |d| {
        d.parametres.langue = code.to_string();
        Ok((None, None))
    })
}

/// Texte d'une pièce adverse déposée (PDF ou .txt), extrait sur ce poste.
#[tauri::command]
fn extraire_texte_document(nom: String, contenu_base64: String) -> Result<documents::DocumentExtrait, String> {
    use base64::Engine;
    let octets = base64::engine::general_purpose::STANDARD
        .decode(contenu_base64.trim())
        .map_err(|_| t("Fichier illisible.").to_string())?;
    documents::extraire(&nom, &octets)
}

#[derive(Serialize)]
struct RapportAlertes {
    envoyees: usize,
    message: String,
    donnees: Option<DonneesApplication>,
}

/// Vérifie les seuils d'alerte et envoie un récapitulatif groupé.
/// `manuel` : bouton « Vérifier maintenant » (erreurs remontées, envoi même si l'automatique est coupé).
async fn verifier_alertes_sur(state: &AppState, manuel: bool) -> Result<RapportAlertes, String> {
    let (p, liste) = {
        let lock = verrou(state)?;
        (lock.parametres.clone(), alertes::selectionner(&lock, aujourd_hui()))
    };
    let rien = |message: &str| Ok(RapportAlertes { envoyees: 0, message: message.to_string(), donnees: None });
    if !manuel && !p.alertes_auto {
        return rien(t("Alertes automatiques désactivées."));
    }
    if !manuel && (p.resend_api_key.trim().is_empty() || p.emails_destinataires.is_empty()) {
        return rien(t("Alertes e-mail non configurées."));
    }
    if liste.is_empty() {
        return rien(t("Aucune nouvelle échéance n'a franchi un seuil d'alerte."));
    }
    let envoi = resend_client::envoyer_html(
        &p.resend_api_key,
        &p.resend_expediteur,
        p.emails_destinataires.clone(),
        &alertes::sujet(&liste),
        &alertes::html(&liste, &p.nom_cabinet),
    )
    .await;
    match envoi {
        Ok(retour) => {
            if let Ok(mut e) = state.derniere_erreur_alerte.lock() {
                e.clear();
            }
            let message = crate::tr!("Récapitulatif envoyé : {} échéance(s).", "Summary sent: {} deadline(s).", liste.len());
            let rep = transaction(state, |d| {
                for e in &liste {
                    let Some(a) = d.affaires.iter_mut().find(|a| a.id == e.id_affaire) else { continue };
                    let Some(x) = a.actes.iter_mut().find(|x| x.id == e.id_acte) else { continue };
                    for s in &e.seuils {
                        if !x.alertes_auto_envoyees.contains(s) {
                            x.alertes_auto_envoyees.push(*s);
                        }
                    }
                    x.alerte_envoyee = true;
                    x.date_alerte = Some(stockage::horodatage_iso());
                }
                for e in &liste {
                    let seuils = e.seuils.iter().map(|s| format!("J-{}", s)).collect::<Vec<_>>().join(", ");
                    journaliser(
                        d,
                        "ALERTE",
                        &crate::tr!("Alerte automatique ({}) : {}", "Automatic alert ({}): {}", seuils, e.nature),
                        &crate::tr!("Échéance du {} | {}", "Deadline of {} | {}", e.date_echeance.format("%d/%m/%Y"), retour),
                        &e.fondement,
                        Some(e.id_affaire.clone()),
                    );
                }
                Ok((None, Some(message.clone())))
            })?;
            Ok(RapportAlertes { envoyees: liste.len(), message, donnees: Some(rep.donnees) })
        }
        Err(err) => {
            let nouvelle = state.derniere_erreur_alerte.lock().map(|mut e| if *e != err { *e = err.clone(); true } else { false }).unwrap_or(false);
            if nouvelle && !manuel {
                let _ = transaction(state, |d| {
                    journaliser(d, "ALERTE", t("Échec de l'envoi automatique des alertes"), &err, "", None);
                    Ok((None, None))
                });
            }
            Err(err)
        }
    }
}

#[tauri::command]
async fn verifier_alertes(state: State<'_, AppState>) -> Result<RapportAlertes, String> {
    verifier_alertes_sur(&state, true).await
}

#[tauri::command]
fn definir_demarrage_windows(actif: bool, state: State<AppState>) -> Result<Reponse, String> {
    demarrage::definir(actif)?;
    transaction(&state, |d| {
        d.parametres.demarrage_windows = actif;
        journaliser(
            d,
            "SYSTEME",
            &if actif {
                crate::tr!("Lancement automatique avec {} activé", "Launch at {} startup turned on", demarrage::nom_systeme())
            } else {
                crate::tr!("Lancement automatique avec {} désactivé", "Launch at {} startup turned off", demarrage::nom_systeme())
            },
            if actif { t("Démarrage fenêtre réduite ; échéances vérifiées toutes les heures.") } else { "" },
            "",
            None,
        );
        Ok((None, Some(if actif { crate::tr!("Deadline Breaker démarrera avec {}.", "Deadline Breaker will start with {}.", demarrage::nom_systeme()) } else { t("Lancement automatique désactivé.").into() })))
    })
}

/// Lien Légifrance d'un article : identifiant connu, cache local, ou page du code en repli.
#[tauri::command]
fn lien_legifrance(code: String, num: String, state: State<AppState>) -> Result<String, String> {
    let cache = legifrance::lire_cache(&state.chemins.fichier_cache_articles);
    if let Some(a) = cache.get(&legifrance::cle(&code, &num)) {
        if legifrance::url_autorisee(&a.url) {
            return Ok(a.url.clone());
        }
    }
    legifrance::url_code(&code).ok_or_else(|| crate::tr!("Code non pris en charge : {}", "Unsupported code: {}", code))
}

// -----------------------------------------------------------------------------
// COMMANDES : COPILOTE IA (GEMINI)
// -----------------------------------------------------------------------------

struct ContexteIa {
    fournisseur: ia::Fournisseur,
    /// Clé et modèle Claude, utilisés en secours si Gemini est indisponible.
    secours_claude: Option<(String, String)>,
    cle: String,
    modele: String,
    anonymiser: bool,
    affaire: Option<Affaire>,
    calendrier: String,
    etape: String,
}

fn contexte_ia(state: &State<'_, AppState>, id_affaire: &Option<String>) -> Result<ContexteIa, String> {
    let lock = verrou(state)?;
    let affaire = id_affaire.as_ref().and_then(|id| lock.affaires.iter().find(|a| &a.id == id).cloned());
    let calendrier = affaire.as_ref().map(|a| calendrier_de(&lock.parametres, a).libelle().to_string()).unwrap_or_default();
    let etape = affaire.as_ref().map(|a| ETAPES[(a.etape_procedure.clamp(1, 5) - 1) as usize].to_string()).unwrap_or_default();
    let p = &lock.parametres;
    let fournisseur = ia::Fournisseur::depuis(&p.ia_fournisseur);
    let (cle, modele) = match fournisseur {
        ia::Fournisseur::Claude => (p.claude_api_key.clone(), p.claude_model.clone()),
        ia::Fournisseur::Gemini => (p.gemini_api_key.clone(), p.gemini_model.clone()),
    };
    let secours_claude = (fournisseur == ia::Fournisseur::Gemini && !p.claude_api_key.trim().is_empty())
        .then(|| (p.claude_api_key.clone(), p.claude_model.clone()));
    Ok(ContexteIa {
        fournisseur,
        secours_claude,
        cle,
        modele,
        anonymiser: lock.parametres.gemini_anonymize,
        affaire,
        calendrier,
        etape,
    })
}

fn journaliser_ia(state: &State<'_, AppState>, titre: &str, ctx: &ContexteIa, id_affaire: &Option<String>) {
    let details = crate::tr!(
        "Modèle : {} {} ; anonymisation : {} ; contenu des échanges non journalisé (secret professionnel).", "Model: {} {}; anonymisation: {}; content of the exchange not logged (professional secrecy).",
        ctx.fournisseur.nom(),
        ctx.modele,
        if ctx.anonymiser { t("activée") } else { t("désactivée") }
    );
    let _ = transaction(state, |d| {
        journaliser(d, "IA", titre, &details, "", id_affaire.clone());
        Ok((None, None))
    });
}

#[tauri::command]
async fn tester_gemini(cle: Option<String>, modele: String, state: State<'_, AppState>) -> Result<String, String> {
    let cle = match cle {
        Some(c) if !c.trim().is_empty() && c != MASQUE_SECRET => c,
        _ => verrou(&state)?.parametres.gemini_api_key.clone(),
    };
    let message = [ia::MessageChat { role: "user".into(), texte: "Réponds uniquement : OK".into() }];
    ia::generer(&cle, &modele, None, &message, false, 0.0).await?;
    Ok(crate::tr!("Connexion Gemini opérationnelle (modèle {}).", "Gemini connection working (model {}).", if modele.trim().is_empty() { "gemini-2.5-flash" } else { modele.trim() }))
}

#[tauri::command]
async fn tester_claude(cle: Option<String>, modele: String, state: State<'_, AppState>) -> Result<String, String> {
    let cle = match cle {
        Some(c) if !c.trim().is_empty() && c != MASQUE_SECRET => c,
        _ => verrou(&state)?.parametres.claude_api_key.clone(),
    };
    let message = [ia::MessageChat { role: "user".into(), texte: "Réponds uniquement : OK".into() }];
    ia::generer_claude(&cle, &modele, None, &message, false).await?;
    Ok(crate::tr!("Connexion Claude opérationnelle (modèle {}).", "Claude connection working (model {}).", if modele.trim().is_empty() { ia::MODELE_CLAUDE_DEFAUT } else { modele.trim() }))
}

#[derive(Serialize)]
struct ReponseCopilote {
    reponse: String,
    anonymise: bool,
    modele: String,
    /// Vrai si un modèle de secours a répondu (Gemini surchargé).
    secours: bool,
}

#[tauri::command]
async fn copilote_message(
    historique: Vec<ia::MessageChat>,
    message: String,
    id_affaire: Option<String>,
    contexte_outil: Option<String>,
    state: State<'_, AppState>,
) -> Result<ReponseCopilote, String> {
    if message.trim().is_empty() {
        return Err(t("Message vide.").into());
    }
    let ctx = contexte_ia(&state, &id_affaire)?;
    let anon = ia::Anonymiseur::pour(ctx.affaire.as_ref(), ctx.anonymiser);
    let dossier = ctx.affaire.as_ref().map(|a| anon.anonymiser(&ia::contexte_dossier(a, &ctx.calendrier, &ctx.etape)));
    let outil = contexte_outil.as_deref().map(|o| anon.anonymiser(o));
    let systeme = ia::invite_systeme(&cpc_engine::fr(aujourd_hui()), dossier.as_deref(), outil.as_deref());
    let debut = historique.len().saturating_sub(20);
    let mut contenus: Vec<ia::MessageChat> = historique[debut..]
        .iter()
        .map(|m| ia::MessageChat { role: m.role.clone(), texte: anon.anonymiser(&m.texte) })
        .collect();
    contenus.push(ia::MessageChat { role: "user".into(), texte: anon.anonymiser(&message) });
    let secours = ctx.secours_claude.as_ref().map(|(c, m)| (c.as_str(), m.as_str()));
    let rep = ia::generer_avec(&ctx.fournisseur, &ctx.cle, &ctx.modele, Some(&systeme), &contenus, false, 0.2, secours).await?;
    journaliser_ia(&state, t("Consultation du copilote IA"), &ctx, &id_affaire);
    Ok(ReponseCopilote { reponse: anon.retablir(&rep.texte), anonymise: anon.actif(), modele: rep.modele, secours: rep.secours })
}

#[tauri::command]
async fn analyser_acte_adverse(texte: String, id_affaire: Option<String>, state: State<'_, AppState>) -> Result<ia::AnalyseActe, String> {
    if texte.trim().is_empty() {
        return Err(t("Collez le texte de l'acte adverse.").into());
    }
    if texte.chars().count() > 200_000 {
        return Err(t("Texte trop long (200 000 caractères au maximum).").into());
    }
    let ctx = contexte_ia(&state, &id_affaire)?;
    let anon = ia::Anonymiseur::pour(ctx.affaire.as_ref(), ctx.anonymiser);
    let consigne = format!("{}{}", ia::consigne_analyse(), anon.anonymiser(&texte));
    let message = [ia::MessageChat { role: "user".into(), texte: consigne }];
    let secours = ctx.secours_claude.as_ref().map(|(c, m)| (c.as_str(), m.as_str()));
    let rep = ia::generer_avec(&ctx.fournisseur, &ctx.cle, &ctx.modele, None, &message, true, 0.1, secours).await?;
    let mut analyse = ia::lire_analyse(&rep.texte)?;
    analyse.modele = rep.modele;
    journaliser_ia(&state, t("Analyse d'un acte adverse par le copilote IA"), &ctx, &id_affaire);
    Ok(ia::retablir_analyse(analyse, &anon))
}

// -----------------------------------------------------------------------------
// COMMANDES : SAUVEGARDE, IMPORT, EXPORTS
// -----------------------------------------------------------------------------

#[tauri::command]
fn exporter_sauvegarde(state: State<AppState>) -> Result<String, String> {
    let lock = verrou(&state)?;
    let chemin = stockage::sauvegarde_horodatee(&state.chemins, &lock, t("manuelle"))?;
    Ok(chemin.display().to_string())
}

#[tauri::command]
fn importer_sauvegarde(contenu: String, conserver_parametres: bool, state: State<AppState>) -> Result<Reponse, String> {
    let propre = contenu.trim_start_matches('\u{feff}').trim();
    let mut importees: DonneesApplication =
        serde_json::from_str(propre).map_err(|e| crate::tr!("Fichier de sauvegarde invalide : {}", "Invalid backup file: {}", e))?;
    stockage::migrer(&mut importees);
    {
        let lock = verrou(&state)?;
        stockage::sauvegarde_horodatee(&state.chemins, &lock, "avant_import")?;
    }
    let nb = importees.affaires.len();
    transaction(&state, |d| {
        let parametres_actuels = d.parametres.clone();
        let historique_actuel = std::mem::take(&mut d.historique);
        *d = importees;
        if conserver_parametres {
            d.parametres = parametres_actuels;
        } else {
            let p = &mut d.parametres;
            if p.gemini_api_key.is_empty() {
                p.gemini_api_key = parametres_actuels.gemini_api_key.clone();
            }
            if p.claude_api_key.is_empty() {
                p.claude_api_key = parametres_actuels.claude_api_key.clone();
            }
            if p.openlegi_url.is_empty() {
                p.openlegi_url = parametres_actuels.openlegi_url.clone();
            }
            if p.openlegi_jeton.is_empty() {
                p.openlegi_jeton = parametres_actuels.openlegi_jeton.clone();
            }
            if p.resend_api_key.is_empty() {
                p.resend_api_key = parametres_actuels.resend_api_key.clone();
            }
        }
        // Le journal d'audit n'est jamais perdu : l'historique courant est conservé à la suite.
        d.historique.extend(historique_actuel);
        journaliser(
            d,
            "SYSTEME",
            t("Import d'une sauvegarde cabinet"),
            &crate::tr!("{} dossier(s) importé(s). Sauvegarde préalable de la base remplacée créée.", "{} matter(s) imported. The replaced database was backed up first.", nb),
            "",
            None,
        );
        Ok((None, Some(crate::tr!("{} dossier(s) importé(s).", "{} matter(s) imported.", nb))))
    })
}

/// Reprise des dossiers saisis dans l'ancienne interface (stockés dans le navigateur, hors data.json).
#[tauri::command]
fn importer_donnees_navigateur(affaires: Vec<serde_json::Value>, parametres: Option<serde_json::Value>, state: State<AppState>) -> Result<Reponse, String> {
    transaction(&state, |d| {
        let mut ajoutees = 0;
        for brut in affaires {
            let mut a: Affaire = match serde_json::from_value(brut) {
                Ok(a) => a,
                Err(_) => continue,
            };
            if ["aff-001", "aff-002", "aff-003", "demo-001", "demo-002"].contains(&a.id.as_str()) || a.nom_dossier.trim().is_empty() {
                continue;
            }
            // L'ancienne interface créait un acte fictif « act-init » à date figée : il est écarté.
            a.actes.retain(|x| x.id != "act-init");
            if a.id.trim().is_empty() || d.affaires.iter().any(|x| x.id == a.id) {
                a.id = nouvel_id("aff");
            }
            for acte in a.actes.iter_mut() {
                if acte.statut != "ACCOMPLI" {
                    acte.statut = "EN_COURS".into();
                }
                if acte.id.trim().is_empty() {
                    acte.id = nouvel_id("act");
                }
                acte.details_calcul = format!("[Repris de l'ancienne interface - à revérifier] {}", acte.details_calcul);
            }
            for dil in a.diligences.iter_mut() {
                if dil.id.trim().is_empty() {
                    dil.id = nouvel_id("dil");
                }
            }
            d.affaires.push(a);
            ajoutees += 1;
        }
        let mut cles = Vec::new();
        if let Some(serde_json::Value::Object(p)) = parametres {
            let texte = |k: &str| p.get(k).and_then(|v| v.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
            if d.parametres.gemini_api_key.is_empty() {
                if let Some(v) = texte("gemini_api_key") {
                    d.parametres.gemini_api_key = v;
                    cles.push("Gemini");
                }
            }
            if d.parametres.resend_api_key.is_empty() {
                if let Some(v) = texte("resend_api_key") {
                    d.parametres.resend_api_key = v;
                    cles.push("Resend");
                }
            }
        }
        let details = format!(
            "{} dossier(s) repris ; identifiants repris : {}",
            ajoutees,
            if cles.is_empty() { t("aucun").to_string() } else { cles.join(", ") }
        );
        journaliser(d, "SYSTEME", t("Reprise des données de l'ancienne interface"), &details, "", None);
        Ok((None, Some(details)))
    })
}

#[tauri::command]
fn exporter_fichier(nom: String, contenu: String, bom: Option<bool>, state: State<AppState>) -> Result<String, String> {
    let mut octets = Vec::new();
    if bom.unwrap_or(false) {
        octets.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    }
    octets.extend_from_slice(contenu.as_bytes());
    let chemin = stockage::ecrire_fichier(&state.chemins.dossier_exports, &nom, &octets)?;
    Ok(chemin.display().to_string())
}

#[tauri::command]
fn reinitialiser_base(state: State<AppState>) -> Result<Reponse, String> {
    let chemin = {
        let lock = verrou(&state)?;
        stockage::sauvegarde_horodatee(&state.chemins, &lock, "avant_reinitialisation")?
    };
    transaction(&state, |d| {
        *d = DonneesApplication::default();
        journaliser(
            d,
            "SYSTEME",
            t("Réinitialisation de la base cabinet"),
            &crate::tr!("Base précédente sauvegardée : {}", "Previous database backed up: {}", chemin.display()),
            "",
            None,
        );
        Ok((None, Some(crate::tr!("Base réinitialisée. Sauvegarde : {}", "Database reset. Backup: {}", chemin.display()))))
    })
}

fn main() {
    let chemins = Chemins::initialiser();
    let (mut donnees, mut avertissements) = stockage::charger(&chemins);
    // Clés API : coffre du système, Windows ou macOS (transfert automatique des clés encore présentes dans data.json).
    let transferees = coffre::charger(&mut donnees.parametres, &mut avertissements);
    if transferees > 0 {
        journaliser(
            &mut donnees,
            "SYSTEME",
            &crate::tr!("Clés API rangées dans le {}", "API keys stored in {}", coffre::nom_coffre()),
            &crate::tr!("{} clé(s) retirée(s) du fichier de données.", "{} key(s) removed from the data file.", transferees),
            "",
            None,
        );
    }
    demarrage::actualiser();
    donnees.parametres.demarrage_windows = demarrage::actif();
    langue::definir(&donnees.parametres.langue);
    // Persiste immédiatement une base neuve ou migrée (sans jamais écraser un fichier illisible,
    // déjà mis en quarantaine par `charger`).
    if let Err(e) = stockage::enregistrer(&chemins, &donnees) {
        eprintln!("Deadline Breaker : écriture initiale impossible : {}", e);
    }

    let app_state = AppState {
        chemins,
        donnees: Mutex::new(donnees),
        avertissements: Mutex::new(avertissements),
        derniere_erreur_alerte: Mutex::new(String::new()),
    };

    tauri::Builder::default()
        .manage(app_state)
        .setup(|app| {
            // Lancement automatique (Windows ou macOS) : fenêtre réduite (barre des tâches / Dock).
            if std::env::args().any(|a| a == demarrage::ARGUMENT_REDUIT) {
                if let Some(fenetre) = app.get_webview_window("main") {
                    let _ = fenetre.minimize();
                }
            }
            // Alertes automatiques : première vérification peu après le lancement, puis toutes les heures.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(45));
                loop {
                    let etat = handle.state::<AppState>();
                    if let Ok(r) = tauri::async_runtime::block_on(verifier_alertes_sur(&etat, false)) {
                        if r.envoyees > 0 {
                            let _ = handle.emit("donnees-modifiees", ());
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_secs(3600));
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            charger_donnees,
            obtenir_infos_stockage,
            lister_regles_delais,
            lister_referentiels,
            recalculer_echeances,
            analyser_prescription,
            orienter_competence,
            appliquer_modele_diligences,
            creer_affaire,
            modifier_affaire,
            supprimer_affaire,
            ajouter_acte_au_dossier,
            supprimer_acte,
            definir_statut_acte,
            ajouter_diligence,
            basculer_diligence,
            supprimer_diligence,
            calculer_delai,
            calculer_rebours,
            enregistrer_historique,
            exporter_journal,
            exporter_journal_pdf,
            definir_langue,
            extraire_texte_document,
            verifier_alertes,
            definir_demarrage_windows,
            enregistrer_parametres,
            declencher_test_email,
            envoyer_alerte_acte,
            tester_openlegi,
            consulter_article,
            ouvrir_legifrance,
            lien_legifrance,
            tester_gemini,
            tester_claude,
            copilote_message,
            analyser_acte_adverse,
            exporter_sauvegarde,
            importer_sauvegarde,
            importer_donnees_navigateur,
            exporter_fichier,
            reinitialiser_base
        ])
        .run(tauri::generate_context!())
        .expect("Erreur au lancement de Deadline Breaker");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acte(id: &str, date: &str, code: &str, echeance: &str) -> ActeDossier {
        ActeDossier {
            id: id.into(),
            nature: format!("Acte {}", id),
            date_evenement: date.into(),
            type_delai: code.into(),
            date_echeance_calculee: echeance.into(),
            ..Default::default()
        }
    }

    #[test]
    fn recalcul_suit_le_calendrier_et_la_residence() {
        let mut d = DonneesApplication::default();
        let mut a = Affaire { id: "aff-1".into(), nom_dossier: "Dossier".into(), ..Default::default() };
        // 11/12/2025 + 15 j = 26/12/2025 (vendredi) : férié uniquement en Alsace-Moselle
        a.actes.push(acte("a1", "2025-12-11", "DELAI_CONSTITUTION_AVOCAT_TJ", "2025-12-26"));
        let mut fixe = acte("a2", "2025-12-11", regles::CODE_ECHEANCE_FIXEE, "2026-01-15");
        fixe.saisie_manuelle = true;
        a.actes.push(fixe);
        let mut fait = acte("a3", "2025-12-11", "DELAI_APPEL_CONTENTIEUX", "2026-01-12");
        fait.statut = "ACCOMPLI".into();
        a.actes.push(fait);
        d.affaires.push(a);

        let (c, _) = recalcul(&mut d, &None, false);
        assert!(c.is_empty(), "rien ne change en métropole");

        d.affaires[0].calendrier_juridiction = "ALSACE_MOSELLE".into();
        let (c, _) = recalcul(&mut d, &None, false);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].nouvelle, "2025-12-29");
        assert_eq!(d.affaires[0].actes[0].date_echeance_calculee, "2025-12-26", "simulation sans effet");

        // Partie à l'étranger : appel +2 mois, mais acte accompli et échéance fixée ignorés
        d.affaires[0].lieu_residence_partie = "ETRANGER".into();
        let (c, _) = recalcul(&mut d, &Some("aff-1".into()), true);
        assert_eq!(c.len(), 1);
        assert_eq!(d.affaires[0].actes.iter().find(|x| x.id == "a1").unwrap().date_echeance_calculee, "2026-02-26");
        assert_eq!(d.affaires[0].actes.iter().find(|x| x.id == "a2").unwrap().date_echeance_calculee, "2026-01-15");
        assert_eq!(d.affaires[0].actes.iter().find(|x| x.id == "a3").unwrap().date_echeance_calculee, "2026-01-12");

        if std::env::var("LEX_EXPORT_UI").is_ok() {
            if let Ok(l) = std::env::var("LEX_LANGUE") {
                langue::definir(&l);
                let (c2, _) = recalcul(&mut d, &Some("aff-1".into()), true);
                let _ = c2;
            }
            let rapport = RapportRecalcul { changements: c, ignores: 0, donnees: Some(d.clone()) };
            println!("UI_RECALCUL={}", serde_json::to_string(&rapport).unwrap());
        }
    }

    #[test]
    fn textes_produits_en_anglais() {
        langue::definir("en");
        let r = calculer("2026-11-19", "DELAI_APPEL_CONTENTIEUX", "FRANCE_METROPOLE", Calendrier::Metropole, None, None).unwrap();
        assert_eq!(r.regle.libelle, "Appeal - contentious matters (1 month)");
        assert_eq!(r.regle.sanction, "Appeal filed out of time is inadmissible");
        assert!(r.resultat.etapes[0].starts_with("Start of the time limit: Thursday 19/11/2026 (Art. 640 CPC)"), "{:?}", r.resultat.etapes);
        assert!(r.resultat.etapes.iter().any(|e| e.contains("Saturday or Sunday") || e.contains("Monday 21/12/2026")), "{:?}", r.resultat.etapes);
        let c = competence::orienter(&competence::Demande { matiere: "CIVIL_GENERAL".into(), montant: Some(18400.0), regle_territoriale: "DEFENDEUR".into(), ville: "Grenoble".into() }).unwrap();
        assert_eq!(c.juridiction, "Judicial Court (tribunal judiciaire) of Grenoble");
        assert!(c.representation.starts_with("Mandatory"));
        for r in regles::catalogue() {
            for texte in [r.libelle, r.sanction, r.categorie] {
                assert!(!texte.contains("Délai") && !texte.contains("Irrecevabilité"), "non traduit : {}", texte);
            }
        }
        assert!(ia::invite_systeme("19/11/2026", None, None).contains("Always answer in English"));
        assert!(ia::consigne_analyse().contains("en anglais"));
        langue::definir("fr");
        assert!(!ia::consigne_analyse().contains("en anglais"));
    }

    #[test]
    fn journal_pdf_filtre_et_lisible() {
        let mut d = DonneesApplication::default();
        d.parametres.nom_cabinet = "Cabinet Martin & Associés".into();
        d.affaires.push(Affaire { id: "aff-1".into(), nom_dossier: "SCI Les Tilleuls c/ M. Martin".into(), ..Default::default() });
        for i in 0..60 {
            journaliser(&mut d, if i % 2 == 0 { "CALCUL" } else { "ALERTE" }, &format!("Échéance n° {} (art. 641 CPC)", i), "Délai d'appel : « un mois » ; prorogé au lundi.", "Art. 538 CPC", if i % 3 == 0 { Some("aff-1".into()) } else { None });
        }
        let (pdf, n) = journal_pdf(&d, "", "", None);
        assert_eq!(n, 60);
        assert!(pdf.starts_with(b"%PDF-1.4") && !String::from_utf8_lossy(&pdf).contains("/Count 1 "), "60 entrées : plusieurs pages");
        let (_, n_cat) = journal_pdf(&d, "ALERTE", "", None);
        assert_eq!(n_cat, 30);
        let (_, n_dossier) = journal_pdf(&d, "", "", Some("aff-1"));
        assert_eq!(n_dossier, 20);
        let (_, n_q) = journal_pdf(&d, "", "tilleuls", None);
        assert_eq!(n_q, 20, "la recherche porte aussi sur le nom du dossier");
        if let Ok(chemin) = std::env::var("LEX_EXPORT_PDF") {
            std::fs::write(chemin, &pdf).unwrap();
        }
    }

    #[test]
    fn calcul_delai_libre_et_distance_forcee() {
        let cal = Calendrier::Metropole;
        let libre = Duree { annees: 0, mois: 2, jours: 10 };
        let r = calculer("2026-12-31", regles::CODE_DELAI_LIBRE, "FRANCE_METROPOLE", cal, None, Some(libre)).unwrap();
        assert_eq!(r.resultat.date_echeance_legale, "2027-03-10");
        assert!(calculer("2026-12-31", regles::CODE_DELAI_LIBRE, "FRANCE_METROPOLE", cal, None, None).is_err());
        let r = calculer("2026-01-31", regles::CODE_DELAI_LIBRE, "ETRANGER", cal, Some(true), Some(Duree { mois: 1, ..Default::default() })).unwrap();
        assert_eq!(r.resultat.date_echeance_legale, "2026-04-30");
        assert!(calculer("2026-01-31", regles::CODE_ECHEANCE_FIXEE, "ETRANGER", cal, None, None).is_err());
    }

    /// Échantillons de réponses réelles pour le banc de test de l'interface.
    #[test]
    fn exporter_echantillons_ui() {
        if std::env::var("LEX_EXPORT_UI").is_err() {
            return;
        }
        if let Ok(l) = std::env::var("LEX_LANGUE") {
            langue::definir(&l);
        }
        println!("UI_REFERENTIELS={}", serde_json::to_string(&lister_referentiels()).unwrap());
        let ev = |t: &str, a: Option<&str>, o: Option<&str>, dep: Option<&str>, deb: Option<&str>, fin: Option<&str>| {
            serde_json::json!({"type": t, "date_assign": a, "date_ord": o, "date_depot": dep, "date_debut": deb, "date_fin": fin, "non_avenue": false})
        };
        let cas = vec![
            ("5_DROIT_COMMUN", "2018-05-15", vec![ev("REFERE_EXPERTISE", Some("2020-03-10"), Some("2020-06-25"), Some("2024-01-15"), None, None)]),
            ("5_DROIT_COMMUN", "2020-01-01", vec![ev("MEDIATION", None, None, None, Some("2024-12-01"), Some("2025-02-01"))]),
            ("5_DROIT_COMMUN", "2020-01-01", vec![ev("DEMANDE_JUSTICE", Some("2023-06-01"), None, None, None, None)]),
            ("5_DROIT_COMMUN", "2020-01-01", vec![ev("DEMANDE_JUSTICE", Some("2023-06-01"), None, None, None, Some("2025-03-01"))]),
        ];
        let mut sorties = Vec::new();
        for (regime, t0, evs) in cas {
            let dm = serde_json::json!({"regime": regime, "qualification": "PRESCRIPTION", "date_depart": t0, "date_naissance_droit": null, "date_cible": "2026-11-18", "evenements": evs});
            let demande: prescription::Demande = serde_json::from_value(dm.clone()).unwrap();
            sorties.push(serde_json::json!({"demande": dm, "resultat": prescription::analyser(&demande).unwrap()}));
        }
        println!("UI_PRESCRIPTION={}", serde_json::to_string(&sorties).unwrap());
        let mut comp = Vec::new();
        for (m, montant) in [("CIVIL_GENERAL", Some(4000.0)), ("BAUX_HABITATION", Some(4000.0)), ("CIVIL_GENERAL", None)] {
            let dm = competence::Demande { matiere: m.into(), montant, regle_territoriale: "DEFENDEUR".into(), ville: String::new() };
            comp.push(serde_json::json!({"matiere": m, "montant": montant, "resultat": competence::orienter(&dm).unwrap()}));
        }
        println!("UI_COMPETENCE={}", serde_json::to_string(&comp).unwrap());
    }
}
