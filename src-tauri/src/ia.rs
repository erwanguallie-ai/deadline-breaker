//! Copilote IA (API Google Gemini) appelé depuis le moteur Rust : la clé API ne transite
//! jamais par l'interface, et l'anonymisation est appliquée avant tout envoi.

use crate::langue::t;

use crate::modeles::Affaire;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::OnceLock;

// -----------------------------------------------------------------------------
// ANONYMISATION (secret professionnel)
// -----------------------------------------------------------------------------

/// Table de correspondance valeur réelle -> marqueur, propre à un dossier.
#[derive(Debug, Clone, Default)]
pub struct Anonymiseur {
    entrees: Vec<(String, String)>,
    actif: bool,
}

fn motifs_generiques() -> &'static Vec<(Regex, &'static str)> {
    static M: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    M.get_or_init(|| {
        vec![
            (Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").unwrap(), "[E-MAIL]"),
            (Regex::new(r"\b[A-Z]{2}\d{2}(?:\s?[A-Z0-9]{4}){3,7}(?:\s?[A-Z0-9]{1,3})?\b").unwrap(), "[IBAN]"),
            (Regex::new(r"\b[12]\s?\d{2}\s?(?:0[1-9]|1[0-2])\s?(?:\d{2}|2A|2B)\s?\d{3}\s?\d{3}(?:\s?\d{2})?\b").unwrap(), "[N° SÉCURITÉ SOCIALE]"),
            (Regex::new(r"(?:\+33\s?|\b0)[1-9](?:[\s.-]?\d{2}){4}\b").unwrap(), "[TÉLÉPHONE]"),
        ]
    })
}

impl Anonymiseur {
    pub fn pour(affaire: Option<&Affaire>, actif: bool) -> Anonymiseur {
        let mut entrees: Vec<(String, String)> = Vec::new();
        if let (Some(a), true) = (affaire, actif) {
            let mut ajouter = |valeur: &str, marqueur: String| {
                let v = valeur.trim();
                if v.chars().count() >= 3 && !entrees.iter().any(|(x, _)| x.eq_ignore_ascii_case(v)) {
                    entrees.push((v.to_string(), marqueur));
                }
            };
            ajouter(&a.client, "[CLIENT]".into());
            ajouter(&a.partie_adverse, "[PARTIE ADVERSE]".into());
            ajouter(&a.nom_dossier, "[DOSSIER]".into());
            ajouter(&a.numero_rg, "[N° RG]".into());
            ajouter(&a.reference_cabinet, "[RÉF. CABINET]".into());
            for (i, t) in a.termes_confidentiels.iter().enumerate() {
                ajouter(t, format!("[TERME CONFIDENTIEL {}]", i + 1));
            }
        }
        // Les valeurs les plus longues d'abord (« SARL Dupont & Fils » avant « Dupont »).
        entrees.sort_by(|a, b| b.0.chars().count().cmp(&a.0.chars().count()));
        Anonymiseur { entrees, actif }
    }

    pub fn actif(&self) -> bool {
        self.actif
    }

    pub fn anonymiser(&self, texte: &str) -> String {
        if !self.actif {
            return texte.to_string();
        }
        let mut t = texte.to_string();
        for (valeur, marqueur) in &self.entrees {
            if let Ok(re) = Regex::new(&format!("(?i){}", regex::escape(valeur))) {
                t = re.replace_all(&t, marqueur.as_str()).into_owned();
            }
        }
        for (re, marqueur) in motifs_generiques().iter() {
            t = re.replace_all(&t, *marqueur).into_owned();
        }
        t
    }

    /// Rétablit localement les noms du dossier dans la réponse (Google n'a vu que les marqueurs).
    pub fn retablir(&self, texte: &str) -> String {
        let mut t = texte.to_string();
        for (valeur, marqueur) in &self.entrees {
            t = t.replace(marqueur.as_str(), valeur);
        }
        t
    }
}

// -----------------------------------------------------------------------------
// CLIENT GEMINI
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MessageChat {
    /// "user" ou "model".
    pub role: String,
    pub texte: String,
}

pub const BASE_GEMINI: &str = "https://generativelanguage.googleapis.com";

pub async fn generer(
    cle: &str,
    modele: &str,
    systeme: Option<&str>,
    contenus: &[MessageChat],
    reponse_json: bool,
    temperature: f32,
) -> Result<String, String> {
    generer_sur(BASE_GEMINI, cle, modele, systeme, contenus, reponse_json, temperature).await
}

/// Modèle Gemini de secours, réputé stable, utilisé quand le modèle choisi est surchargé.
pub const MODELE_GEMINI_SECOURS: &str = "gemini-2.5-flash";

/// Attentes entre deux tentatives sur un même modèle.
#[cfg(not(test))]
const ATTENTES_MS: [u64; 2] = [1500, 3500];
#[cfg(test)]
const ATTENTES_MS: [u64; 2] = [5, 5];

/// Nature d'un échec d'appel : une surcharge mérite une nouvelle tentative, un modèle introuvable
/// un passage au modèle de secours, le reste (clé invalide, requête refusée) est définitif.
enum Echec {
    Transitoire(String),
    ModeleIndisponible(String),
    Definitif(String),
}

impl Echec {
    fn message(self) -> String {
        match self {
            Echec::Transitoire(m) | Echec::ModeleIndisponible(m) | Echec::Definitif(m) => m,
        }
    }
}

/// Un seul appel à Gemini (sans nouvelle tentative).
pub async fn generer_sur(
    base: &str,
    cle: &str,
    modele: &str,
    systeme: Option<&str>,
    contenus: &[MessageChat],
    reponse_json: bool,
    temperature: f32,
) -> Result<String, String> {
    appel_gemini(base, cle, modele, systeme, contenus, reponse_json, temperature).await.map_err(Echec::message)
}

async fn appel_gemini(
    base: &str,
    cle: &str,
    modele: &str,
    systeme: Option<&str>,
    contenus: &[MessageChat],
    reponse_json: bool,
    temperature: f32,
) -> Result<String, Echec> {
    if cle.trim().is_empty() {
        return Err(Echec::Definitif(t("Clé API Gemini non configurée (Paramètres).").to_string()));
    }
    let modele = if modele.trim().is_empty() { MODELE_GEMINI_SECOURS } else { modele.trim() };
    if !modele.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_') {
        return Err(Echec::Definitif(crate::tr!("Identifiant de modèle invalide : {}", "Invalid model identifier: {}", modele)));
    }
    let mut corps = json!({
        "contents": contenus.iter().map(|m| json!({
            "role": if m.role == "model" { "model" } else { "user" },
            "parts": [{ "text": m.texte }]
        })).collect::<Vec<_>>(),
        "generationConfig": { "temperature": temperature }
    });
    if let Some(s) = systeme {
        corps["systemInstruction"] = json!({ "parts": [{ "text": s }] });
    }
    if reponse_json {
        corps["generationConfig"]["responseMimeType"] = json!("application/json");
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| Echec::Definitif(crate::tr!("Client HTTP indisponible : {}", "HTTP client unavailable: {}", e)))?;
    let url = format!("{}/v1beta/models/{}:generateContent", base, modele);
    let rep = client
        .post(url)
        .header("x-goog-api-key", cle.trim())
        .json(&corps)
        .send()
        .await
        .map_err(|e| Echec::Transitoire(crate::tr!("Erreur réseau Gemini : {}", "Gemini network error: {}", e)))?;
    let statut = rep.status();
    let v: Value = rep.json().await.unwrap_or(Value::Null);
    if !statut.is_success() {
        let msg = format!("Gemini ({}) : {}", statut.as_u16(), v["error"]["message"].as_str().unwrap_or(t("réponse invalide")));
        return Err(match statut.as_u16() {
            429 | 500 | 502 | 503 | 504 => Echec::Transitoire(msg),
            404 => Echec::ModeleIndisponible(msg),
            _ => Echec::Definitif(msg),
        });
    }
    extraire_texte(&v).map_err(Echec::Definitif)
}

/// Appel résilient : 3 tentatives espacées sur le modèle choisi, puis 2 sur le modèle de secours.
/// Retourne le texte et le modèle qui a effectivement répondu.
pub async fn generer_gemini_resilient_sur(
    base: &str,
    cle: &str,
    modele: &str,
    systeme: Option<&str>,
    contenus: &[MessageChat],
    reponse_json: bool,
    temperature: f32,
) -> Result<(String, String), String> {
    let principal = if modele.trim().is_empty() { MODELE_GEMINI_SECOURS } else { modele.trim() };
    let mut essais: Vec<&str> = vec![principal; 3];
    if principal != MODELE_GEMINI_SECOURS {
        essais.extend([MODELE_GEMINI_SECOURS; 2]);
    }
    let mut derniere = String::new();
    let mut i = 0;
    let mut precedent: Option<&str> = None;
    while i < essais.len() {
        let m = essais[i];
        if precedent == Some(m) {
            let attente = ATTENTES_MS[(i % 3).saturating_sub(1).min(1)];
            tokio::time::sleep(std::time::Duration::from_millis(attente)).await;
        }
        precedent = Some(m);
        match appel_gemini(base, cle, m, systeme, contenus, reponse_json, temperature).await {
            Ok(texte) => return Ok((texte, m.to_string())),
            Err(Echec::Definitif(e)) => return Err(e),
            Err(Echec::Transitoire(e)) => derniere = e,
            Err(Echec::ModeleIndisponible(e)) => {
                derniere = e;
                // Modèle introuvable : inutile d'insister, passage direct au modèle de secours.
                while i + 1 < essais.len() && essais[i + 1] == m {
                    i += 1;
                }
            }
        }
        i += 1;
    }
    Err(crate::tr!(
        "Gemini est momentanément indisponible ({}). Modèles essayés : {} puis {}. Réessayez dans un instant, ou enregistrez une clé Claude dans les Paramètres pour servir de secours.",
        "Gemini is temporarily unavailable ({}). Models tried: {} then {}. Try again shortly, or save a Claude key in Settings to act as a backup.",
        derniere, principal, MODELE_GEMINI_SECOURS
    ))
}

pub async fn generer_gemini_resilient(
    cle: &str,
    modele: &str,
    systeme: Option<&str>,
    contenus: &[MessageChat],
    reponse_json: bool,
    temperature: f32,
) -> Result<(String, String), String> {
    generer_gemini_resilient_sur(BASE_GEMINI, cle, modele, systeme, contenus, reponse_json, temperature).await
}

// -----------------------------------------------------------------------------
// CLIENT CLAUDE (API Messages d'Anthropic, appel HTTP direct)
// -----------------------------------------------------------------------------

pub const BASE_CLAUDE: &str = "https://api.anthropic.com";
pub const MODELE_CLAUDE_DEFAUT: &str = "claude-opus-5-5";
const VERSION_API_CLAUDE: &str = "2026-10-21";
const BETA_REPLI_CLAUDE: &str = "server-side-fallback-2026-07-01";

/// Modèles qui acceptent le repli automatique côté serveur (`fallbacks: "default"`) :
/// si un filtre de sécurité décline la demande, Anthropic la rejoue sur un autre modèle.
fn accepte_repli(modele: &str) -> bool {
    ["claude-opus-5-5", "claude-opus-5", "claude-sonnet-5-5", "claude-fable-5-2", "claude-fable-5-1"].contains(&modele)
}

pub async fn generer_claude(cle: &str, modele: &str, systeme: Option<&str>, contenus: &[MessageChat], reponse_json: bool) -> Result<String, String> {
    generer_claude_sur(BASE_CLAUDE, cle, modele, systeme, contenus, reponse_json).await
}

pub async fn generer_claude_sur(
    base: &str,
    cle: &str,
    modele: &str,
    systeme: Option<&str>,
    contenus: &[MessageChat],
    reponse_json: bool,
) -> Result<String, String> {
    if cle.trim().is_empty() {
        return Err(t("Clé API Claude non configurée (Paramètres).").to_string());
    }
    let modele = if modele.trim().is_empty() { MODELE_CLAUDE_DEFAUT } else { modele.trim() };
    if !modele.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_') {
        return Err(crate::tr!("Identifiant de modèle invalide : {}", "Invalid model identifier: {}", modele));
    }
    // L'API exige des tours alternés commençant par l'utilisateur : fusion des tours consécutifs.
    let mut messages: Vec<(String, String)> = Vec::new();
    for m in contenus {
        let role = if m.role == "model" || m.role == "assistant" { "assistant" } else { "user" };
        match messages.last_mut() {
            Some((r, t)) if r == role => {
                t.push_str("\n\n");
                t.push_str(&m.texte);
            }
            _ => messages.push((role.to_string(), m.texte.clone())),
        }
    }
    while messages.first().map(|(r, _)| r == "assistant").unwrap_or(false) {
        messages.remove(0);
    }
    if messages.is_empty() {
        return Err(t("Message vide.").to_string());
    }
    let mut instructions = systeme.unwrap_or("").to_string();
    if reponse_json {
        if !instructions.is_empty() {
            instructions.push_str("\n\n");
        }
        instructions.push_str("Réponds uniquement par un objet JSON valide, sans texte ni balise autour.");
    }
    let mut corps = json!({
        "model": modele,
        "max_tokens": 16000,
        "output_config": { "effort": "medium" },
        "messages": messages.iter().map(|(r, t)| json!({ "role": r, "content": t })).collect::<Vec<_>>(),
    });
    if !instructions.is_empty() {
        corps["system"] = json!(instructions);
    }
    let repli = accepte_repli(modele);
    if repli {
        corps["fallbacks"] = json!("default");
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| crate::tr!("Client HTTP indisponible : {}", "HTTP client unavailable: {}", e))?;
    let mut requete = client
        .post(format!("{}/v1/messages", base))
        .header("x-api-key", cle.trim())
        .header("anthropic-version", VERSION_API_CLAUDE);
    if repli {
        requete = requete.header("anthropic-beta", BETA_REPLI_CLAUDE);
    }
    let rep = requete.json(&corps).send().await.map_err(|e| crate::tr!("Erreur réseau Claude : {}", "Claude network error: {}", e))?;
    let statut = rep.status();
    let v: Value = rep.json().await.unwrap_or(Value::Null);
    if !statut.is_success() {
        let msg = v["error"]["message"].as_str().unwrap_or(t("réponse invalide"));
        let conseil = match statut.as_u16() {
            401 => t(" Vérifiez la clé API."),
            404 => t(" Vérifiez l'identifiant du modèle."),
            429 => t(" Limite de débit ou plafond de dépenses atteint ; réessayez plus tard."),
            500 | 529 => t(" Service momentanément surchargé ; réessayez dans un instant."),
            _ => "",
        };
        return Err(format!("Claude ({}) : {}.{}", statut.as_u16(), msg.trim_end_matches('.'), conseil));
    }
    extraire_texte_claude(&v)
}

/// Texte de la réponse : seuls les blocs « text » sont retenus (les blocs de réflexion sont ignorés).
pub fn extraire_texte_claude(v: &Value) -> Result<String, String> {
    if v["stop_reason"].as_str() == Some("refusal") {
        let categorie = v["stop_details"]["category"].as_str().map(|c| crate::tr!(" (catégorie : {})", " (category: {})", c)).unwrap_or_default();
        return Err(crate::tr!("Claude a décliné cette demande{}. Reformulez la question.", "Claude declined this request{}. Rephrase the question.", categorie));
    }
    let texte: String = v["content"]
        .as_array()
        .map(|blocs| blocs.iter().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join(""))
        .unwrap_or_default();
    if texte.trim().is_empty() {
        let fin = v["stop_reason"].as_str().unwrap_or(t("inconnue"));
        return Err(crate::tr!("Réponse vide de Claude (motif : {}).", "Empty response from Claude (reason: {}).", fin));
    }
    Ok(texte)
}

/// Fournisseur choisi dans les Paramètres.
#[derive(Debug, Clone, PartialEq)]
pub enum Fournisseur {
    Gemini,
    Claude,
}

impl Fournisseur {
    pub fn depuis(code: &str) -> Fournisseur {
        if code.trim().eq_ignore_ascii_case("CLAUDE") { Fournisseur::Claude } else { Fournisseur::Gemini }
    }
    pub fn nom(&self) -> &'static str {
        match self {
            Fournisseur::Gemini => "Gemini",
            Fournisseur::Claude => "Claude",
        }
    }
}

/// Réponse de l'IA et modèle qui l'a produite (secours : modèle de remplacement utilisé).
pub struct ReponseIa {
    pub texte: String,
    pub modele: String,
    pub secours: bool,
}

/// Appel du fournisseur configuré (la température n'est transmise qu'à Gemini).
/// Gemini indisponible : nouvelles tentatives, modèle de secours, puis Claude si une clé est enregistrée.
#[allow(clippy::too_many_arguments)]
pub async fn generer_avec(
    fournisseur: &Fournisseur,
    cle: &str,
    modele: &str,
    systeme: Option<&str>,
    contenus: &[MessageChat],
    reponse_json: bool,
    temperature: f32,
    secours_claude: Option<(&str, &str)>,
) -> Result<ReponseIa, String> {
    match fournisseur {
        Fournisseur::Claude => {
            let texte = generer_claude(cle, modele, systeme, contenus, reponse_json).await?;
            Ok(ReponseIa { texte, modele: format!("Claude · {}", if modele.trim().is_empty() { MODELE_CLAUDE_DEFAUT } else { modele.trim() }), secours: false })
        }
        Fournisseur::Gemini => match generer_gemini_resilient(cle, modele, systeme, contenus, reponse_json, temperature).await {
            Ok((texte, utilise)) => {
                let secours = utilise != modele.trim() && !modele.trim().is_empty();
                Ok(ReponseIa { texte, modele: format!("Gemini · {}", utilise), secours })
            }
            Err(e) => match secours_claude {
                Some((cle_claude, modele_claude)) if !cle_claude.trim().is_empty() => {
                    let texte = generer_claude(cle_claude, modele_claude, systeme, contenus, reponse_json)
                        .await
                        .map_err(|e2| format!("{}\n{}", e, e2))?;
                    let m = if modele_claude.trim().is_empty() { MODELE_CLAUDE_DEFAUT } else { modele_claude.trim() };
                    Ok(ReponseIa { texte, modele: format!("Claude · {}", m), secours: true })
                }
                _ => Err(e),
            },
        },
    }
}

pub fn extraire_texte(v: &Value) -> Result<String, String> {
    if let Some(raison) = v["promptFeedback"]["blockReason"].as_str() {
        return Err(crate::tr!("Demande bloquée par Gemini ({}).", "Request blocked by Gemini ({}).", raison));
    }
    let candidat = &v["candidates"][0];
    let texte: String = candidat["content"]["parts"]
        .as_array()
        .map(|parts| parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join(""))
        .unwrap_or_default();
    if texte.trim().is_empty() {
        let fin = candidat["finishReason"].as_str().unwrap_or(t("inconnue"));
        return Err(crate::tr!("Réponse vide de Gemini (motif : {}).", "Empty response from Gemini (reason: {}).", fin));
    }
    Ok(texte)
}

// -----------------------------------------------------------------------------
// INVITES (PROMPTS)
// -----------------------------------------------------------------------------

pub fn invite_systeme(date_du_jour: &str, contexte_dossier: Option<&str>, contexte_outil: Option<&str>) -> String {
    let mut p = if crate::langue::en() {
        format!(
            "You are a legal assistant specialised in French civil procedure, built into the Deadline Breaker software used by a lawyer.\n\
             Rules:\n\
             0. Always answer in English. French law applies: cite French provisions in their original form (e.g. art. 641 CPC, art. 2224 C. civ.) and keep French legal terms in italics or parentheses where useful.\n\
             1. Cite the applicable articles precisely (CPC, Civil Code, COJ, special codes) and flag any uncertainty or possible recent change in the texts.\n\
             2. Never invent case law, references or facts: if you are not sure, say so explicitly.\n\
             3. Professional, structured, directly usable tone; draft documents are templates for the lawyer to check.\n\
             4. Markers in square brackets ([CLIENT], [OPPOSING PARTY], [N° RG]...) replace confidential information: reuse them as they are.\n\
             5. Today's date: {}.",
            date_du_jour
        )
    } else {
        format!(
            "Tu es un assistant juridique expert en procédure civile française, intégré au logiciel Deadline Breaker utilisé par un avocat.\n\
             Règles :\n\
             1. Cite précisément les articles applicables (CPC, Code civil, COJ, codes spéciaux) et signale toute incertitude ou évolution récente possible des textes.\n\
             2. N'invente jamais de jurisprudence, de référence ou de fait : si tu n'es pas certain, dis-le explicitement.\n\
             3. Ton professionnel, structuré, directement exploitable ; les projets d'actes sont des trames à vérifier par l'avocat.\n\
             4. Les marqueurs entre crochets ([CLIENT], [PARTIE ADVERSE], [N° RG]...) remplacent des informations confidentielles : réutilise-les tels quels.\n\
             5. Date du jour : {}.",
            date_du_jour
        )
    };
    if let Some(d) = contexte_dossier {
        p.push_str(if crate::langue::en() { "\n\n--- SELECTED MATTER ---\n" } else { "\n\n--- DOSSIER SÉLECTIONNÉ ---\n" });
        p.push_str(d);
        p.push_str("\n---");
    }
    if let Some(o) = contexte_outil {
        if !o.trim().is_empty() {
            p.push_str(if crate::langue::en() { "\n\n--- CONTEXT OF THE OPEN TOOL ---\n" } else { "\n\n--- CONTEXTE DE L'OUTIL OUVERT ---\n" });
            p.push_str(o.trim());
            p.push_str("\n---");
        }
    }
    p
}

pub fn contexte_dossier(a: &Affaire, calendrier: &str, etape: &str) -> String {
    let actes = if a.actes.is_empty() {
        "- aucun acte enregistré".to_string()
    } else {
        a.actes
            .iter()
            .map(|x| {
                format!(
                    "- {} (acte du {}) : échéance {}, {}, sanction : {}, statut : {}",
                    x.nature,
                    x.date_evenement,
                    x.date_echeance_calculee,
                    x.fondement_cpc,
                    x.sanction,
                    if x.statut == "ACCOMPLI" { t("accompli") } else { t("en cours") }
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let dils = if a.diligences.is_empty() {
        "- aucune".to_string()
    } else {
        a.diligences.iter().map(|d| format!("- [{}] {}", if d.fait { "x" } else { " " }, d.libelle)).collect::<Vec<_>>().join("\n")
    };
    format!(
        "Dossier : {}\nClient : {}\nPartie adverse : {}\nJuridiction : {}\nN° RG : {}\nEnjeu : {} €\nRésidence de la partie : {}\nSiège / calendrier : {}\nÉtape : {}\nÉchéances :\n{}\nDiligences :\n{}\nNotes : {}",
        a.nom_dossier,
        if a.client.is_empty() { "-" } else { &a.client },
        if a.partie_adverse.is_empty() { "-" } else { &a.partie_adverse },
        if a.juridiction.is_empty() { "-" } else { &a.juridiction },
        if a.numero_rg.is_empty() { "-" } else { &a.numero_rg },
        a.enjeu_financier,
        a.lieu_residence_partie,
        calendrier,
        etape,
        actes,
        dils,
        if a.notes.is_empty() { "-" } else { &a.notes }
    )
}

pub const CONSIGNE_ANALYSE: &str = "Analyse l'acte de procédure ci-dessous (assignation ou conclusions adverses) et réponds UNIQUEMENT en JSON valide, sans texte autour, selon ce schéma :\n\
{\"resume\":\"...\",\"dates\":[{\"libelle\":\"...\",\"date\":\"AAAA-MM-JJ\"}],\"pretentions\":[{\"libelle\":\"...\",\"montant\":\"...\"}],\"fondements\":[\"...\"],\"vigilance\":[\"...\"]}\n\
- \"resume\" : nature de l'acte, juridiction, parties (marqueurs conservés) et objet, en trois phrases au plus.\n\
- \"dates\" : uniquement les dates expressément mentionnées (signification, audience, délais...).\n\
- \"fondements\" : articles et textes invoqués, tels qu'ils figurent dans l'acte.\n\
- \"vigilance\" : irrégularités ou moyens possibles (exceptions in limine litis, nullités, fins de non-recevoir, prescription), formulés prudemment, avec l'article concerné. N'invente aucun fait.\n\
- Conserve tels quels les marqueurs entre crochets.\n\nACTE :\n";

/// Consigne d'analyse dans la langue courante (les clés JSON ne changent pas).
pub fn consigne_analyse() -> String {
    if crate::langue::en() {
        CONSIGNE_ANALYSE.replacen(
            "Conserve tels quels les marqueurs entre crochets.",
            "Conserve tels quels les marqueurs entre crochets.\n- Rédige toutes les valeurs (resume, libelle, vigilance) en anglais ; garde les références légales françaises telles quelles (ex. art. 56 CPC).",
            1,
        )
    } else {
        CONSIGNE_ANALYSE.to_string()
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct DateDetectee {
    pub libelle: String,
    pub date: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Pretention {
    pub libelle: String,
    pub montant: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct AnalyseActe {
    pub resume: String,
    pub dates: Vec<DateDetectee>,
    pub pretentions: Vec<Pretention>,
    pub fondements: Vec<String>,
    pub vigilance: Vec<String>,
    /// Modèle ayant produit l'analyse (renseigné par le moteur).
    pub modele: String,
}

/// Lecture tolérante de la réponse JSON (bloc ```json éventuel, montants numériques).
pub fn lire_analyse(brut: &str) -> Result<AnalyseActe, String> {
    let t = brut.trim();
    let t = t.strip_prefix("```json").or_else(|| t.strip_prefix("```")).unwrap_or(t);
    let t = t.strip_suffix("```").unwrap_or(t).trim();
    let mut v: Value = serde_json::from_str(t).map_err(|e| crate::tr!("Réponse de l'IA illisible (JSON attendu) : {}", "Unreadable AI response (JSON expected): {}", e))?;
    if let Some(arr) = v["pretentions"].as_array_mut() {
        for p in arr.iter_mut() {
            if p["montant"].is_number() {
                p["montant"] = json!(p["montant"].to_string());
            }
        }
    }
    serde_json::from_value(v).map_err(|e| crate::tr!("Structure de réponse inattendue : {}", "Unexpected response structure: {}", e))
}

pub fn retablir_analyse(a: AnalyseActe, anon: &Anonymiseur) -> AnalyseActe {
    AnalyseActe {
        resume: anon.retablir(&a.resume),
        modele: a.modele,
        dates: a.dates.into_iter().map(|d| DateDetectee { libelle: anon.retablir(&d.libelle), date: d.date }).collect(),
        pretentions: a.pretentions.into_iter().map(|p| Pretention { libelle: anon.retablir(&p.libelle), montant: p.montant }).collect(),
        fondements: a.fondements.into_iter().map(|f| anon.retablir(&f)).collect(),
        vigilance: a.vigilance.into_iter().map(|f| anon.retablir(&f)).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn affaire() -> Affaire {
        Affaire {
            nom_dossier: "SAS Alpha c/ Dupont".into(),
            client: "SAS Alpha".into(),
            partie_adverse: "Jean Dupont".into(),
            numero_rg: "RG 26/01234".into(),
            termes_confidentiels: vec!["Résidence Les Pins".into()],
            ..Default::default()
        }
    }

    #[test]
    fn anonymisation_et_retablissement() {
        let a = affaire();
        let anon = Anonymiseur::pour(Some(&a), true);
        let t = "Le dossier SAS Alpha c/ Dupont (RG 26/01234) : jean dupont, résidence les pins, joignable au 06 12 34 56 78 ou jean.dupont@exemple.fr, IBAN FR76 3000 6000 0112 3456 7890 189.";
        let r = anon.anonymiser(t);
        assert!(!r.to_lowercase().contains("dupont"), "{}", r);
        assert!(!r.contains("Alpha"));
        assert!(!r.contains("26/01234"));
        assert!(!r.to_lowercase().contains("les pins"));
        assert!(r.contains("[DOSSIER]") && r.contains("[N° RG]") && r.contains("[PARTIE ADVERSE]"));
        assert!(r.contains("[TÉLÉPHONE]") && r.contains("[E-MAIL]") && r.contains("[IBAN]") && r.contains("[TERME CONFIDENTIEL 1]"), "{}", r);
        assert_eq!(anon.retablir("Écrire à [CLIENT] contre [PARTIE ADVERSE]"), "Écrire à SAS Alpha contre Jean Dupont");
    }

    #[test]
    fn anonymisation_desactivee() {
        let a = affaire();
        let anon = Anonymiseur::pour(Some(&a), false);
        assert_eq!(anon.anonymiser("SAS Alpha"), "SAS Alpha");
        let sans_dossier = Anonymiseur::pour(None, true);
        assert_eq!(sans_dossier.anonymiser("Appeler le 01 23 45 67 89"), "Appeler le [TÉLÉPHONE]");
    }

    #[test]
    fn dates_et_montants_non_masques() {
        let anon = Anonymiseur::pour(Some(&affaire()), true);
        let t = "Audience du 15/01/2027, demande de 120 000 € au visa de l'article 1231-1 du Code civil, assignation du 14 octobre 2026.";
        assert_eq!(anon.anonymiser(t), t);
    }

    #[test]
    fn lecture_reponse_gemini() {
        let v = json!({"candidates":[{"content":{"parts":[{"text":"Bonjour "},{"text":"Maître"}]},"finishReason":"STOP"}]});
        assert_eq!(extraire_texte(&v).unwrap(), "Bonjour Maître");
        let bloque = json!({"promptFeedback":{"blockReason":"SAFETY"}});
        assert!(extraire_texte(&bloque).unwrap_err().contains("SAFETY"));
        let vide = json!({"candidates":[{"finishReason":"MAX_TOKENS"}]});
        assert!(extraire_texte(&vide).unwrap_err().contains("MAX_TOKENS"));
    }

    #[test]
    fn lecture_reponse_claude() {
        let v = json!({ "stop_reason": "end_turn", "content": [
            { "type": "thinking", "thinking": "", "signature": "x" },
            { "type": "text", "text": "Délai de " }, { "type": "text", "text": "un mois." } ] });
        assert_eq!(extraire_texte_claude(&v).unwrap(), "Délai de un mois.");
        let refus = json!({ "stop_reason": "refusal", "stop_details": { "type": "refusal", "category": "cyber" }, "content": [] });
        assert!(extraire_texte_claude(&refus).unwrap_err().contains("cyber"));
        assert!(extraire_texte_claude(&json!({ "stop_reason": "max_tokens", "content": [] })).unwrap_err().contains("max_tokens"));
        assert_eq!(Fournisseur::depuis("claude"), Fournisseur::Claude);
        assert_eq!(Fournisseur::depuis(""), Fournisseur::Gemini);
    }

    #[test]
    fn lecture_analyse_tolerante() {
        let brut = "```json\n{\"dates\":[{\"libelle\":\"Audience\",\"date\":\"2027-01-15\"}],\"pretentions\":[{\"libelle\":\"Principal\",\"montant\":120000}],\"fondements\":[\"art. 1231-1 C. civ.\"]}\n```";
        let a = lire_analyse(brut).unwrap();
        assert_eq!(a.dates[0].date, "2027-01-15");
        assert_eq!(a.pretentions[0].montant, "120000");
        assert!(a.vigilance.is_empty());
        assert!(lire_analyse("pas du json").is_err());
    }
}
