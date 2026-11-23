//! Client OpenLegi : serveur MCP (Model Context Protocol) donnant accès aux données Légifrance.
//! Transport « Streamable HTTP » : requêtes JSON-RPC 2.0 en POST, réponse JSON ou flux SSE.
//! Séquence : initialize -> notifications/initialized -> tools/call « rechercher_code ».

use crate::langue::t;

use crate::legifrance::ArticleConsulte;
use serde_json::{json, Value};
use std::time::Duration;

const VERSION_PROTOCOLE: &str = "2025-03-26";

pub struct Connexion {
    pub url: String,
    pub jeton: String,
}

/// Affichage de débogage sans jamais révéler le jeton.
impl std::fmt::Debug for Connexion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hote = self.url.split('?').next().unwrap_or_default();
        write!(f, "Connexion {{ url: {}, jeton: {} }}", hote, if self.jeton.is_empty() { t("aucun") } else { "masqué" })
    }
}

impl Connexion {
    pub fn depuis(url: &str, jeton: &str) -> Result<Connexion, String> {
        let url = url.trim();
        if url.is_empty() {
            return Err("NON_CONFIGURE".into());
        }
        if !(url.starts_with("https://") || url.starts_with("http://127.0.0.1") || url.starts_with("http://localhost")) {
            return Err(t("L'adresse du serveur OpenLegi doit commencer par https://").into());
        }
        Ok(Connexion { url: url.to_string(), jeton: jeton.trim().to_string() })
    }
}

/// Lit une réponse JSON-RPC, qu'elle arrive en JSON simple ou en flux SSE (« data: {...} »).
pub fn lire_reponse(type_contenu: &str, corps: &str, id: u64) -> Result<Value, String> {
    let candidats: Vec<Value> = if type_contenu.contains("text/event-stream") {
        corps
            .lines()
            .filter_map(|l| l.strip_prefix("data:"))
            .filter_map(|d| serde_json::from_str::<Value>(d.trim()).ok())
            .collect()
    } else {
        match serde_json::from_str::<Value>(corps.trim()) {
            Ok(Value::Array(a)) => a,
            Ok(v) => vec![v],
            Err(e) => return Err(crate::tr!("Réponse OpenLegi illisible : {}", "Unreadable OpenLegi response: {}", e)),
        }
    };
    let v = candidats
        .into_iter()
        .find(|v| v["id"].as_u64() == Some(id))
        .ok_or_else(|| t("Réponse OpenLegi sans résultat pour la requête.").to_string())?;
    if let Some(err) = v.get("error") {
        return Err(format!("OpenLegi : {}", err["message"].as_str().unwrap_or("erreur inconnue")));
    }
    Ok(v["result"].clone())
}

async fn poster(
    client: &reqwest::Client,
    cx: &Connexion,
    session: &Option<String>,
    corps: Value,
) -> Result<(String, String, Option<String>), String> {
    let mut req = client
        .post(&cx.url)
        .header("Accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", VERSION_PROTOCOLE)
        .json(&corps);
    if !cx.jeton.is_empty() {
        req = req.bearer_auth(&cx.jeton);
    }
    if let Some(s) = session {
        req = req.header("Mcp-Session-Id", s);
    }
    let rep = req.send().await.map_err(|e| crate::tr!("Erreur réseau OpenLegi : {}", "OpenLegi network error: {}", e))?;
    let statut = rep.status().as_u16();
    let session_rep = rep.headers().get("mcp-session-id").and_then(|v| v.to_str().ok()).map(|s| s.to_string());
    let type_contenu = rep.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let texte = rep.text().await.unwrap_or_default();
    if statut == 401 || statut == 403 {
        return Err(crate::tr!("OpenLegi a refusé l'accès ({}) : vérifiez l'adresse du serveur et le jeton.", "OpenLegi refused access ({}): check the server address and token.", statut));
    }
    if !(200..300).contains(&statut) {
        let extrait: String = texte.chars().take(250).collect();
        return Err(format!("OpenLegi ({}) : {}", statut, extrait));
    }
    Ok((type_contenu, texte, session_rep))
}

/// Appelle un outil du serveur MCP et renvoie le texte de son résultat.
pub async fn appeler_outil(cx: &Connexion, outil: &str, arguments: Value) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| crate::tr!("Client HTTP indisponible : {}", "HTTP client unavailable: {}", e))?;

    let init = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": VERSION_PROTOCOLE,
            "capabilities": {},
            "clientInfo": { "name": "Deadline Breaker", "version": env!("CARGO_PKG_VERSION") }
        }
    });
    let (tc, corps, session) = poster(&client, cx, &None, init).await?;
    lire_reponse(&tc, &corps, 1)?;

    poster(&client, cx, &session, json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).await?;

    let appel = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": outil, "arguments": arguments } });
    let (tc, corps, _) = poster(&client, cx, &session, appel).await?;
    let resultat = lire_reponse(&tc, &corps, 2)?;
    let texte = resultat["content"]
        .as_array()
        .map(|c| c.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default();
    if resultat["isError"].as_bool().unwrap_or(false) {
        return Err(format!("OpenLegi : {}", texte.chars().take(250).collect::<String>()));
    }
    if texte.trim().is_empty() {
        return Err(t("OpenLegi n'a renvoyé aucun texte.").into());
    }
    Ok(texte)
}

/// Valeur d'une ligne « • Libellé: valeur » du résultat texte d'OpenLegi.
fn champ(bloc: &str, libelle: &str) -> String {
    bloc.lines()
        .map(|l| l.trim().trim_start_matches('•').trim())
        .find_map(|l| l.strip_prefix(libelle).map(|v| v.trim_start_matches(':').trim().to_string()))
        .unwrap_or_default()
}

fn date_fr_vers_iso(d: &str) -> Option<String> {
    let p: Vec<&str> = d.trim().split('/').collect();
    if p.len() == 3 && p[2].len() == 4 {
        Some(format!("{}-{}-{}", p[2], p[1], p[0]))
    } else {
        None
    }
}

/// Analyse la réponse texte de « rechercher_code » et retient l'article dont le numéro correspond exactement.
pub fn lire_article(texte: &str, num: &str) -> Result<ArticleConsulte, String> {
    if texte.contains("[SEARCH_NO_RESULTS]") || texte.contains(t("Aucun résultat")) {
        return Err(crate::tr!("Article {} introuvable sur Légifrance.", "Article {} not found on Légifrance.", num));
    }
    let voulu = crate::legifrance::normaliser_num(num);
    for bloc in texte.split("=== ARTICLE CODE").skip(1) {
        let numero = champ(bloc, "Numéro article");
        if crate::legifrance::normaliser_num(&numero) != voulu {
            continue;
        }
        let debut = match bloc.find("Texte de l'article:") {
            Some(i) => i + "Texte de l'article:".len(),
            None => continue,
        };
        let reste = &bloc[debut..];
        let fin = ["\nNotes:", "\nARTICLES CITÉS", "\n=====", "\nPage suivante"]
            .iter()
            .filter_map(|m| reste.find(m))
            .min()
            .unwrap_or(reste.len());
        let corps = reste[..fin].trim().to_string();
        if corps.is_empty() {
            continue;
        }
        let id = champ(bloc, "Identifiant article");
        let lien = champ(bloc, "Lien article Légifrance");
        return Ok(ArticleConsulte {
            num: numero,
            texte: corps,
            etat: champ(bloc, "État juridique"),
            date_debut: date_fr_vers_iso(&champ(bloc, "Date début vigueur")),
            url: if lien.starts_with("https://www.legifrance.gouv.fr/") {
                lien
            } else if id.starts_with("LEGIARTI") {
                format!("https://www.legifrance.gouv.fr/codes/article_lc/{}", id)
            } else {
                String::new()
            },
            id,
            ..Default::default()
        });
    }
    Err(crate::tr!("Article {} introuvable dans la réponse d'OpenLegi.", "Article {} not found in the OpenLegi response.", num))
}

/// Consulte un article de code via l'outil « rechercher_code » d'OpenLegi.
pub async fn consulter_article(cx: &Connexion, nom_code: &str, num: &str) -> Result<ArticleConsulte, String> {
    let n = crate::legifrance::normaliser_num(num);
    let arguments = json!({ "search": n, "code_name": nom_code, "champ": "NUM_ARTICLE", "page_size": 5 });
    let texte = appeler_outil(cx, "rechercher_code", arguments).await?;
    lire_article(&texte, &n)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Extrait réel d'une réponse d'OpenLegi (format observé le 18/11/2026).
    const REPONSE: &str = "RÉSULTATS CODE - Page 1/1 (2 article(s) au total)\nAffichage: articles 1-2\n==================================================\n\n\n============================================================\n\n=== ARTICLE CODE 1 ===\nArticle 641-1 - (VIGUEUR) - Chapitre Ier\n\nINFORMATIONS:\n• Identifiant article: LEGIARTI000000000099\n• Numéro article: 641-1\n• État juridique: VIGUEUR\n\nCONTENU:\nTexte de l'article:\nTexte d'un autre article.\n\n=== ARTICLE CODE 2 ===\nArticle 641 - (VIGUEUR) - Chapitre Ier : La computation des délais.\n\nINFORMATIONS:\n• Identifiant article: LEGIARTI000006411002\n• CID article: LEGIARTI000006411002\n• Numéro article: 641\n• État juridique: VIGUEUR\n• Date début vigueur: 01/01/1976\n• Date fin vigueur: 01/01/2999\n• Lien article Légifrance: https://www.legifrance.gouv.fr/codes/article_lc/LEGIARTI000006411002\n\nCONTENU:\nTexte de l'article:\nLorsqu'un délai est exprimé en jours, celui de l'acte ne compte pas. Lorsqu'un délai est exprimé en mois et en jours, les mois sont d'abord décomptés, puis les jours.\n\nARTICLES CITÉS:\n  • Décret n°2016-154 du 15 février 2016 - art. 1 - (cible)\n";

    #[test]
    fn lecture_article_exact() {
        let a = lire_article(REPONSE, "641").unwrap();
        assert_eq!(a.num, "641");
        assert_eq!(a.id, "LEGIARTI000006411002");
        assert_eq!(a.etat, "VIGUEUR");
        assert_eq!(a.date_debut.as_deref(), Some("1976-01-01"));
        assert_eq!(a.url, "https://www.legifrance.gouv.fr/codes/article_lc/LEGIARTI000006411002");
        assert!(a.texte.starts_with("Lorsqu'un délai") && a.texte.ends_with("puis les jours."));
        let b = lire_article(REPONSE, "641-1").unwrap();
        assert_eq!(b.texte, "Texte d'un autre article.");
        assert!(b.url.ends_with("LEGIARTI000000000099"));
        assert!(lire_article(REPONSE, "642").is_err());
        assert!(lire_article("[SEARCH_NO_RESULTS] Aucun résultat pour '9999'", "9999").is_err());
    }

    #[test]
    fn lecture_json_et_sse() {
        let json = r#"{"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"ok"}]}}"#;
        assert_eq!(lire_reponse("application/json", json, 2).unwrap()["content"][0]["text"], "ok");
        let sse = "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"flux\"}]}}\n\n";
        assert_eq!(lire_reponse("text/event-stream", sse, 2).unwrap()["content"][0]["text"], "flux");
        let erreur = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"Method not found"}}"#;
        assert!(lire_reponse("application/json", erreur, 1).unwrap_err().contains("Method not found"));
    }

    #[test]
    fn adresse_validee() {
        assert!(Connexion::depuis("", "x").unwrap_err().contains("NON_CONFIGURE"));
        assert!(Connexion::depuis("ftp://exemple", "").is_err());
        assert!(Connexion::depuis("https://exemple.fr/mcp", "").is_ok());
    }
}
