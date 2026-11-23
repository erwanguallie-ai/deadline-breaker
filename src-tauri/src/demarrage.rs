//! Lancement automatique à l'ouverture de session, sans droits d'administrateur :
//! - Windows : clé « Run » de l'utilisateur ;
//! - macOS : agent de lancement ~/Library/LaunchAgents/fr.avocat.deadlinebreaker.plist.
//! L'application démarre alors fenêtre réduite (« --reduit ») pour vérifier les échéances
//! et envoyer les alertes.
//! (Le nom des champs « demarrage_windows » est conservé pour la compatibilité des données.)

pub const ARGUMENT_REDUIT: &str = "--reduit";

/// Identifiant de l'agent de lancement macOS (nom du fichier .plist).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub const LABEL_MAC: &str = "fr.avocat.deadlinebreaker";

/// Nom du système, pour les messages (« avec Windows », « avec macOS »).
pub fn nom_systeme() -> &'static str {
    if cfg!(target_os = "macos") { "macOS" } else { "Windows" }
}

/// Ligne de commande enregistrée (Windows) : chemin entre guillemets suivi de l'argument.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn commande(exe: &str) -> String {
    format!("\"{}\" {}", exe, ARGUMENT_REDUIT)
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn echapper_xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Contenu de l'agent de lancement macOS : ouvre l'application réduite à l'ouverture de session.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn plist_mac(exe: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>{}</string>
	<key>ProgramArguments</key>
	<array>
		<string>{}</string>
		<string>{}</string>
	</array>
	<key>RunAtLoad</key>
	<true/>
	<key>ProcessType</key>
	<string>Interactive</string>
</dict>
</plist>
"#,
        LABEL_MAC,
        echapper_xml(exe),
        ARGUMENT_REDUIT
    )
}

/// Refuse un exécutable qui ne serait pas l'application installée :
/// version de développement (target/debug, target/release), application lancée
/// directement depuis l'image disque (.dmg) ou depuis un emplacement provisoire de macOS
/// (« App Translocation », quand l'application n'a pas été glissée dans Applications).
#[cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]
pub fn verifier_exe(exe: &str) -> Result<(), String> {
    let bas = exe.replace('/', "\\").to_lowercase();
    if bas.contains("\\target\\debug\\") || bas.contains("\\target\\release\\") {
        return Err(if cfg!(target_os = "macos") {
            crate::langue::t("Option à activer depuis l'application installée (dossier Applications), et non depuis la version de développement.")
        } else {
            crate::langue::t("Option à activer depuis l'application installée (menu Démarrer), et non depuis la version de développement lancée par « cargo run ».")
        }
        .into());
    }
    if bas.contains("\\apptranslocation\\") || bas.starts_with("\\volumes\\deadline breaker") {
        return Err(crate::langue::t("Glissez d'abord Deadline Breaker dans le dossier Applications, puis relancez-le depuis ce dossier avant d'activer cette option.").into());
    }
    Ok(())
}

#[cfg(windows)]
mod systeme {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};
    use winreg::RegKey;

    const CLE: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    const VALEUR: &str = "DeadlineBreaker";
    /// Valeur des versions précédentes, retirée lors de toute modification.
    const ANCIENNE: &str = "LexProcedura";

    pub fn actif() -> bool {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(CLE, KEY_READ)
            .map(|k| k.get_value::<String, _>(VALEUR).is_ok() || k.get_value::<String, _>(ANCIENNE).is_ok())
            .unwrap_or(false)
    }

    pub fn definir(actif: bool) -> Result<(), String> {
        let (cle, _) = RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey_with_flags(CLE, KEY_SET_VALUE)
            .map_err(|e| crate::tr!("Registre inaccessible : {}", "Registry unavailable: {}", e))?;
        let _ = cle.delete_value(ANCIENNE);
        if actif {
            let exe = std::env::current_exe().map_err(|e| crate::tr!("Chemin de l'application introuvable : {}", "Application path not found: {}", e))?;
            let exe = exe.display().to_string();
            super::verifier_exe(&exe)?;
            cle.set_value(VALEUR, &super::commande(&exe)).map_err(|e| crate::tr!("Écriture dans le registre impossible : {}", "Could not write to the registry: {}", e))
        } else {
            match cle.delete_value(VALEUR) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(crate::tr!("Suppression dans le registre impossible : {}", "Could not delete from the registry: {}", e)),
            }
        }
    }

    pub fn actualiser() {}
}

#[cfg(target_os = "macos")]
mod systeme {
    use std::path::PathBuf;

    fn fichier() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join("Library").join("LaunchAgents").join(format!("{}.plist", super::LABEL_MAC)))
    }

    fn exe_installe() -> Result<String, String> {
        let exe = std::env::current_exe().map_err(|e| crate::tr!("Chemin de l'application introuvable : {}", "Application path not found: {}", e))?;
        let exe = exe.display().to_string();
        super::verifier_exe(&exe)?;
        Ok(exe)
    }

    pub fn actif() -> bool {
        fichier().map(|f| f.exists()).unwrap_or(false)
    }

    pub fn definir(actif: bool) -> Result<(), String> {
        let f = fichier().ok_or_else(|| crate::langue::t("Dossier personnel introuvable.").to_string())?;
        if actif {
            let exe = exe_installe()?;
            if let Some(dossier) = f.parent() {
                std::fs::create_dir_all(dossier).map_err(|e| crate::tr!("Création du dossier LaunchAgents impossible : {}", "Could not create the LaunchAgents folder: {}", e))?;
            }
            std::fs::write(&f, super::plist_mac(&exe)).map_err(|e| crate::tr!("Écriture de l'agent de lancement impossible : {}", "Could not write the launch agent: {}", e))
        } else {
            match std::fs::remove_file(&f) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(crate::tr!("Suppression de l'agent de lancement impossible : {}", "Could not delete the launch agent: {}", e)),
            }
        }
    }

    /// Application déplacée ou mise à jour : l'agent pointe à nouveau vers l'exécutable courant.
    pub fn actualiser() {
        let (Some(f), Ok(exe)) = (fichier(), exe_installe()) else { return };
        if let Ok(actuel) = std::fs::read_to_string(&f) {
            let attendu = super::plist_mac(&exe);
            if actuel != attendu {
                let _ = std::fs::write(&f, attendu);
            }
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod systeme {
    pub fn actif() -> bool {
        false
    }
    pub fn definir(_: bool) -> Result<(), String> {
        Err(crate::langue::t("Le lancement automatique n'est disponible que sous Windows et macOS.").into())
    }
    pub fn actualiser() {}
}

pub fn actif() -> bool {
    systeme::actif()
}

pub fn definir(actif: bool) -> Result<(), String> {
    systeme::definir(actif)
}

/// Au démarrage : met à jour le chemin enregistré si l'application a été déplacée (macOS).
pub fn actualiser() {
    systeme::actualiser()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commande_et_exe_de_developpement() {
        assert_eq!(commande(r"C:\Users\a\AppData\Local\LexProcedura\lexprocedura.exe"), r#""C:\Users\a\AppData\Local\LexProcedura\lexprocedura.exe" --reduit"#);
        assert!(verifier_exe(r"C:\Users\a\LexProcedura\src-tauri\target\debug\lexprocedura.exe").is_err());
        assert!(verifier_exe(r"C:\Users\a\AppData\Local\LexProcedura\lexprocedura.exe").is_ok());
    }

    #[test]
    fn agent_de_lancement_mac() {
        let exe = "/Applications/Deadline Breaker.app/Contents/MacOS/lexprocedura";
        assert!(verifier_exe(exe).is_ok());
        assert!(verifier_exe("/Users/a/LexProcedura/src-tauri/target/release/lexprocedura").is_err());
        assert!(verifier_exe("/Volumes/Deadline Breaker/Deadline Breaker.app/Contents/MacOS/lexprocedura").is_err());
        assert!(verifier_exe("/private/var/folders/x/AppTranslocation/ABC/d/Deadline Breaker.app/Contents/MacOS/lexprocedura").is_err());
        let p = plist_mac(exe);
        assert!(p.contains("<string>fr.avocat.deadlinebreaker</string>"));
        assert!(p.contains("<string>/Applications/Deadline Breaker.app/Contents/MacOS/lexprocedura</string>\n\t\t<string>--reduit</string>"));
        assert!(p.contains("<key>RunAtLoad</key>\n\t<true/>"));
        assert!(plist_mac("/Applications/A&B <x>.app").contains("/Applications/A&amp;B &lt;x&gt;.app"));
    }
}
