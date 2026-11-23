//! Langue de l'interface et des textes produits par le moteur (anglais par défaut, français
//! en option). Le droit applicable reste le droit français : seules les explications sont
//! traduites, les références (« art. 538 CPC ») ne changent pas.

use std::collections::HashMap;
use std::sync::OnceLock;

#[cfg(not(test))]
mod etat {
    use std::sync::atomic::{AtomicBool, Ordering};
    static EN: AtomicBool = AtomicBool::new(true);
    pub fn en() -> bool {
        EN.load(Ordering::Relaxed)
    }
    pub fn definir_en(v: bool) {
        EN.store(v, Ordering::Relaxed)
    }
}

/// Tests : langue propre à chaque fil (français par défaut, comme les assertions existantes).
#[cfg(test)]
mod etat {
    use std::cell::Cell;
    thread_local! { static EN: Cell<bool> = const { Cell::new(false) }; }
    pub fn en() -> bool {
        EN.with(|c| c.get())
    }
    pub fn definir_en(v: bool) {
        EN.with(|c| c.set(v))
    }
}

pub fn en() -> bool {
    etat::en()
}

/// "en" ou "fr" (toute autre valeur : anglais).
pub fn definir(code: &str) {
    etat::definir_en(!code.trim().eq_ignore_ascii_case("fr"));
}

#[cfg(test)]
pub fn code() -> &'static str {
    if en() { "en" } else { "fr" }
}

fn table() -> &'static HashMap<&'static str, &'static str> {
    static T: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    T.get_or_init(|| crate::dictionnaire::DICT.iter().copied().collect())
}

/// Texte fixe dans la langue courante (le texte français sert de clé).
pub fn t(fr: &'static str) -> &'static str {
    if !en() {
        return fr;
    }
    table().get(fr).copied().unwrap_or(fr)
}

/// `tr!("texte {}", "text {}", x)` : format!() dans la langue courante.
#[macro_export]
macro_rules! tr {
    ($fr:literal, $en:literal $(, $arg:expr)* $(,)?) => {
        if $crate::langue::en() { format!($en $(, $arg)*) } else { format!($fr $(, $arg)*) }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bascule_et_dictionnaire() {
        assert_eq!(t("Tribunal judiciaire"), "Tribunal judiciaire");
        definir("en");
        assert_eq!(t("Tribunal judiciaire"), "Judicial Court (tribunal judiciaire)");
        assert_eq!(crate::tr!("Délai de {} jour(s)", "Time limit of {} day(s)", 15), "Time limit of 15 day(s)");
        assert_eq!(t("texte absent du dictionnaire"), "texte absent du dictionnaire");
        definir("fr");
        assert_eq!(code(), "fr");
        // Chaque entrée du dictionnaire est unique et réellement traduite.
        let mut cles: Vec<&str> = crate::dictionnaire::DICT.iter().map(|(f, _)| *f).collect();
        let n = cles.len();
        cles.sort();
        cles.dedup();
        assert_eq!(n, cles.len(), "clé en double dans le dictionnaire");
    }
}
