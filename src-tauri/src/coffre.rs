//! Coffre-fort des clés API : Gestionnaire d'identification Windows (identifiants génériques
//! « LexProcedura ») ou Trousseau macOS (mots de passe d'application « LexProcedura »).
//! Les clés ne sont alors plus écrites dans data.json.
//! Sur les autres systèmes (développement sous Linux), le coffre est indisponible et data.json
//! conserve les clés, comme avant. Les tests utilisent un coffre en mémoire propre à chaque fil d'exécution.

use crate::modeles::ParametresApp;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

#[cfg_attr(any(test, not(any(windows, target_os = "macos"))), allow(dead_code))]
pub const SERVICE: &str = "LexProcedura";
pub const NOMS: [&str; 5] = ["gemini_api_key", "claude_api_key", "openlegi_url", "openlegi_jeton", "resend_api_key"];

fn champ<'a>(p: &'a mut ParametresApp, nom: &str) -> &'a mut String {
    match nom {
        "gemini_api_key" => &mut p.gemini_api_key,
        "claude_api_key" => &mut p.claude_api_key,
        "openlegi_url" => &mut p.openlegi_url,
        "openlegi_jeton" => &mut p.openlegi_jeton,
        _ => &mut p.resend_api_key,
    }
}

pub fn disponible() -> bool {
    backend::disponible()
}

/// Nom du coffre du système, pour les messages et l'interface.
pub fn nom_coffre() -> &'static str {
    match (cfg!(target_os = "macos"), crate::langue::en()) {
        (true, true) => "macOS Keychain",
        (true, false) => "Trousseau macOS",
        (false, true) => "Windows Credential Manager",
        (false, false) => "Gestionnaire d'identification Windows",
    }
}

/// Une lecture du coffre a échoué au démarrage : on n'y écrit plus (une clé non lue
/// serait sinon effacée au premier enregistrement) et data.json conserve les clés.
static LECTURE_EN_ECHEC: AtomicBool = AtomicBool::new(false);

/// Dernières valeurs écrites dans le coffre (évite de réécrire à chaque enregistrement).
static CACHE: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

fn memoriser(nom: &str, valeur: &str) {
    if let Ok(mut c) = CACHE.lock() {
        c.get_or_insert_with(HashMap::new).insert(nom.to_string(), valeur.to_string());
    }
}

fn deja_ecrit(nom: &str, valeur: &str) -> bool {
    CACHE.lock().ok().and_then(|c| c.as_ref().and_then(|m| m.get(nom).cloned())).as_deref() == Some(valeur)
}

/// Au démarrage : complète les paramètres avec les clés du coffre. Une clé encore présente
/// dans data.json (versions précédentes) est transférée dans le coffre.
/// Retourne le nombre de clés transférées (data.json doit alors être réécrit sans elles).
pub fn charger(p: &mut ParametresApp, avertissements: &mut Vec<String>) -> usize {
    if !disponible() {
        return 0;
    }
    let mut transferees = 0;
    for nom in NOMS {
        let valeur = champ(p, nom).trim().to_string();
        if !valeur.is_empty() {
            match backend::ecrire(nom, &valeur) {
                Ok(()) => {
                    memoriser(nom, &valeur);
                    transferees += 1;
                }
                Err(e) => avertissements.push(crate::tr!("Clé « {} » non transférée dans le {} : {}", "Key \"{}\" not moved to the {}: {}", nom, nom_coffre(), e)),
            }
        } else {
            match backend::lire(nom) {
                Ok(Some(v)) => {
                    memoriser(nom, &v);
                    *champ(p, nom) = v;
                }
                Ok(None) => memoriser(nom, ""),
                Err(e) => {
                    LECTURE_EN_ECHEC.store(true, Ordering::SeqCst);
                    avertissements.push(crate::tr!("Lecture de la clé « {} » dans le {} impossible : {}", "Could not read key \"{}\" from the {}: {}", nom, nom_coffre(), e));
                }
            }
        }
    }
    transferees
}

/// Écrit dans le coffre les clés modifiées (une clé vidée est supprimée du coffre).
pub fn persister(p: &ParametresApp) -> Result<(), String> {
    if !disponible() {
        return Err("coffre indisponible".into());
    }
    if LECTURE_EN_ECHEC.load(Ordering::SeqCst) {
        return Err("coffre en échec de lecture depuis le démarrage".into());
    }
    let mut copie = p.clone();
    for nom in NOMS {
        let valeur = champ(&mut copie, nom).trim().to_string();
        if deja_ecrit(nom, &valeur) {
            continue;
        }
        if valeur.is_empty() {
            backend::supprimer(nom)?;
        } else {
            backend::ecrire(nom, &valeur)?;
        }
        memoriser(nom, &valeur);
    }
    Ok(())
}

// -----------------------------------------------------------------------------
// Implémentations
// -----------------------------------------------------------------------------

#[cfg(all(any(windows, target_os = "macos"), not(test)))]
mod backend {
    use super::SERVICE;

    fn entree(nom: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(SERVICE, nom).map_err(|e| e.to_string())
    }
    pub fn disponible() -> bool {
        true
    }
    pub fn lire(nom: &str) -> Result<Option<String>, String> {
        match entree(nom)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn ecrire(nom: &str, valeur: &str) -> Result<(), String> {
        entree(nom)?.set_password(valeur).map_err(|e| e.to_string())
    }
    pub fn supprimer(nom: &str) -> Result<(), String> {
        match entree(nom)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[cfg(all(not(any(windows, target_os = "macos")), not(test)))]
mod backend {
    pub fn disponible() -> bool {
        false
    }
    pub fn lire(_: &str) -> Result<Option<String>, String> {
        Ok(None)
    }
    pub fn ecrire(_: &str, _: &str) -> Result<(), String> {
        Err("coffre indisponible sur ce système".into())
    }
    pub fn supprimer(_: &str) -> Result<(), String> {
        Ok(())
    }
}

/// Coffre de test : en mémoire, propre à chaque fil (les tests s'exécutent en parallèle),
/// inactif tant qu'un test ne l'a pas activé.
#[cfg(test)]
pub mod backend {
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static ACTIF: RefCell<bool> = const { RefCell::new(false) };
        static STOCK: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
    }
    pub fn activer() {
        ACTIF.with(|a| *a.borrow_mut() = true);
    }
    pub fn contenu(nom: &str) -> Option<String> {
        STOCK.with(|s| s.borrow().get(nom).cloned())
    }
    pub fn disponible() -> bool {
        ACTIF.with(|a| *a.borrow())
    }
    pub fn lire(nom: &str) -> Result<Option<String>, String> {
        Ok(contenu(nom))
    }
    pub fn ecrire(nom: &str, valeur: &str) -> Result<(), String> {
        STOCK.with(|s| s.borrow_mut().insert(nom.to_string(), valeur.to_string()));
        Ok(())
    }
    pub fn supprimer(nom: &str) -> Result<(), String> {
        STOCK.with(|s| s.borrow_mut().remove(nom));
        Ok(())
    }
}
