//! Tests d'intégration HTTP : un faux serveur local vérifie la forme exacte des requêtes
//! envoyées à Gemini, à Claude et à OpenLegi / Légifrance (URL, en-têtes d'authentification, corps).

use crate::{ia, openlegi, resend_client};
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

struct Requete {
    ligne: String,
    entetes: Vec<(String, String)>,
    corps: String,
}

impl Requete {
    fn entete(&self, nom: &str) -> String {
        self.entetes.iter().find(|(k, _)| k == nom).map(|(_, v)| v.clone()).unwrap_or_default()
    }
    fn json(&self) -> Value {
        serde_json::from_str(&self.corps).unwrap()
    }
}

/// Serveur HTTP minimal : répond successivement avec les (statut, corps) fournis.
fn serveur(reponses: Vec<(u16, String)>) -> (String, Arc<Mutex<Vec<Requete>>>) {
    serveur_complet(reponses.into_iter().map(|(s, c)| (s, c, "application/json".to_string(), String::new())).collect())
}

fn serveur_complet(reponses: Vec<(u16, String, String, String)>) -> (String, Arc<Mutex<Vec<Requete>>>) {
    std::env::set_var("NO_PROXY", "127.0.0.1,localhost");
    std::env::set_var("no_proxy", "127.0.0.1,localhost");
    let ecoute = TcpListener::bind("127.0.0.1:0").unwrap();
    let adresse = format!("http://{}", ecoute.local_addr().unwrap());
    let journal = Arc::new(Mutex::new(Vec::new()));
    let j = journal.clone();
    thread::spawn(move || {
        for (statut, corps, type_contenu, entetes_sup) in reponses {
            let Ok((mut flux, _)) = ecoute.accept() else { return };
            let mut lecteur = BufReader::new(flux.try_clone().unwrap());
            let mut ligne = String::new();
            lecteur.read_line(&mut ligne).unwrap();
            let mut entetes = Vec::new();
            let mut longueur = 0usize;
            loop {
                let mut h = String::new();
                lecteur.read_line(&mut h).unwrap();
                let h = h.trim_end().to_string();
                if h.is_empty() {
                    break;
                }
                if let Some((k, v)) = h.split_once(':') {
                    let k = k.trim().to_lowercase();
                    if k == "content-length" {
                        longueur = v.trim().parse().unwrap_or(0);
                    }
                    entetes.push((k, v.trim().to_string()));
                }
            }
            let mut tampon = vec![0u8; longueur];
            lecteur.read_exact(&mut tampon).unwrap();
            j.lock().unwrap().push(Requete { ligne: ligne.trim_end().to_string(), entetes, corps: String::from_utf8_lossy(&tampon).into_owned() });
            let reponse = format!(
                "HTTP/1.1 {} X\r\nContent-Type: {}\r\n{}Content-Length: {}\r\nConnection: close\r\n\r\n{}",
                statut,
                type_contenu,
                entetes_sup,
                corps.len(),
                corps
            );
            flux.write_all(reponse.as_bytes()).unwrap();
            flux.flush().unwrap();
        }
    });
    (adresse, journal)
}

fn bloquer<F: std::future::Future>(f: F) -> F::Output {
    tauri::async_runtime::block_on(f)
}

#[test]
fn gemini_requete_et_reponse() {
    let (base, journal) = serveur(vec![
        (200, r#"{"candidates":[{"content":{"parts":[{"text":"Réponse "},{"text":"juridique"}]},"finishReason":"STOP"}]}"#.into()),
        (400, r#"{"error":{"code":400,"message":"API key not valid"}}"#.into()),
    ]);
    let msgs = [
        ia::MessageChat { role: "user".into(), texte: "Question 1".into() },
        ia::MessageChat { role: "model".into(), texte: "Réponse 1".into() },
        ia::MessageChat { role: "user".into(), texte: "Question 2".into() },
    ];
    let r = bloquer(ia::generer_sur(&base, "CLE-TEST", "gemini-2.5-flash", Some("CONSIGNE"), &msgs, true, 0.2)).unwrap();
    assert_eq!(r, "Réponse juridique");
    let err = bloquer(ia::generer_sur(&base, "MAUVAISE", "gemini-2.5-flash", None, &msgs[..1], false, 0.2)).unwrap_err();
    assert!(err.contains("400") && err.contains("API key not valid"), "{}", err);

    let j = journal.lock().unwrap();
    assert_eq!(j[0].ligne, "POST /v1beta/models/gemini-2.5-flash:generateContent HTTP/1.1");
    assert_eq!(j[0].entete("x-goog-api-key"), "CLE-TEST");
    assert!(!j[0].ligne.contains("key="), "la clé ne doit pas figurer dans l'URL");
    let corps = j[0].json();
    assert_eq!(corps["systemInstruction"]["parts"][0]["text"], "CONSIGNE");
    assert_eq!(corps["contents"].as_array().unwrap().len(), 3);
    assert_eq!(corps["contents"][1]["role"], "model");
    assert_eq!(corps["contents"][2]["parts"][0]["text"], "Question 2");
    assert_eq!(corps["generationConfig"]["responseMimeType"], "application/json");
    assert!(j[1].json().get("systemInstruction").is_none());
}

#[test]
fn gemini_refuse_modele_invalide_et_cle_absente() {
    let msgs = [ia::MessageChat { role: "user".into(), texte: "x".into() }];
    assert!(bloquer(ia::generer_sur("http://127.0.0.1:9", "CLE", "../evil", None, &msgs, false, 0.0)).is_err());
    assert!(bloquer(ia::generer_sur("http://127.0.0.1:9", "", "gemini-2.5-flash", None, &msgs, false, 0.0)).unwrap_err().contains("non configurée"));
}

#[test]
fn claude_requete_et_reponse() {
    let (base, journal) = serveur(vec![
        (200, r#"{"id":"msg_1","type":"message","role":"assistant","stop_reason":"end_turn","content":[{"type":"thinking","thinking":"","signature":"s"},{"type":"text","text":"Réponse "},{"type":"text","text":"juridique"}]}"#.into()),
        (401, r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#.into()),
        (200, r#"{"stop_reason":"end_turn","content":[{"type":"text","text":"{}"}]}"#.into()),
    ]);
    let msgs = [
        ia::MessageChat { role: "model".into(), texte: "Accueil".into() },
        ia::MessageChat { role: "user".into(), texte: "Question 1".into() },
        ia::MessageChat { role: "model".into(), texte: "Réponse 1".into() },
        ia::MessageChat { role: "user".into(), texte: "Question 2".into() },
        ia::MessageChat { role: "user".into(), texte: "Précision".into() },
    ];
    let r = bloquer(ia::generer_claude_sur(&base, "sk-ant-TEST", "claude-opus-5-5", Some("CONSIGNE"), &msgs, false)).unwrap();
    assert_eq!(r, "Réponse juridique");
    let err = bloquer(ia::generer_claude_sur(&base, "MAUVAISE", "claude-opus-5-5", None, &msgs[1..2], false)).unwrap_err();
    assert!(err.contains("401") && err.contains("invalid x-api-key") && err.contains("Vérifiez la clé"), "{}", err);
    bloquer(ia::generer_claude_sur(&base, "sk-ant-TEST", "claude-haiku-5-5", None, &msgs[1..2], true)).unwrap();

    let j = journal.lock().unwrap();
    assert_eq!(j[0].ligne, "POST /v1/messages HTTP/1.1");
    assert_eq!(j[0].entete("x-api-key"), "sk-ant-TEST");
    assert_eq!(j[0].entete("anthropic-version"), "2026-10-21");
    assert_eq!(j[0].entete("anthropic-beta"), "server-side-fallback-2026-07-01");
    let corps = j[0].json();
    assert_eq!(corps["model"], "claude-opus-5-5");
    assert_eq!(corps["system"], "CONSIGNE");
    assert_eq!(corps["fallbacks"], "default");
    assert_eq!(corps["output_config"]["effort"], "medium");
    assert!(corps.get("temperature").is_none() && corps.get("thinking").is_none());
    let m = corps["messages"].as_array().unwrap();
    assert_eq!(m.len(), 3, "accueil retiré, tours consécutifs fusionnés");
    assert_eq!(m[0]["role"], "user");
    assert_eq!(m[1]["role"], "assistant");
    assert_eq!(m[2]["content"], "Question 2\n\nPrécision");
    // Haiku : pas de repli serveur ; réponse JSON demandée par la consigne système
    assert!(j[2].entete("anthropic-beta").is_empty());
    assert!(j[2].json().get("fallbacks").is_none());
    assert!(j[2].json()["system"].as_str().unwrap().contains("JSON"));
}

#[test]
fn resend_recapitulatif_html() {
    let (base, journal) = serveur(vec![(200, r#"{"id":"e-1"}"#.into()), (422, r#"{"message":"domaine non vérifié"}"#.into())]);
    let dest = vec!["avocat@cabinet.fr".to_string()];
    let ok = bloquer(resend_client::envoyer_html_sur(&base, "re_CLE", "LexProcedura <alertes@cabinet.fr>", dest.clone(), "[LexProcedura] 2 échéances", "<p>récap</p>")).unwrap();
    assert!(ok.contains("avocat@cabinet.fr"));
    let err = bloquer(resend_client::envoyer_html_sur(&base, "re_CLE", "x@y.fr", dest, "s", "h")).unwrap_err();
    assert!(err.contains("422") && err.contains("domaine non vérifié"));
    let j = journal.lock().unwrap();
    assert_eq!(j[0].ligne, "POST /emails HTTP/1.1");
    assert_eq!(j[0].entete("authorization"), "Bearer re_CLE");
    let corps = j[0].json();
    assert_eq!(corps["subject"], "[LexProcedura] 2 échéances");
    assert_eq!(corps["to"][0], "avocat@cabinet.fr");
    assert_eq!(corps["html"], "<p>récap</p>");
    assert!(bloquer(resend_client::envoyer_html("", "", vec!["a@b.fr".into()], "s", "h")).unwrap_err().contains("non configurée"));
}

#[test]
fn gemini_surcharge_nouvelles_tentatives_puis_modele_de_secours() {
    let surcharge = r#"{"error":{"code":503,"message":"This model is currently experiencing high demand."}}"#;
    let (base, journal) = serveur(vec![
        (503, surcharge.into()),
        (503, surcharge.into()),
        (503, surcharge.into()),
        (200, r#"{"candidates":[{"content":{"parts":[{"text":"Réponse du secours"}]},"finishReason":"STOP"}]}"#.into()),
    ]);
    let msgs = [ia::MessageChat { role: "user".into(), texte: "Question".into() }];
    let (texte, modele) = bloquer(ia::generer_gemini_resilient_sur(&base, "CLE", "gemini-3.8-flash", None, &msgs, false, 0.2)).unwrap();
    assert_eq!((texte.as_str(), modele.as_str()), ("Réponse du secours", ia::MODELE_GEMINI_SECOURS));
    let j = journal.lock().unwrap();
    let chemins: Vec<&str> = j.iter().map(|r| r.ligne.as_str()).collect();
    assert_eq!(chemins, vec![
        "POST /v1beta/models/gemini-3.8-flash:generateContent HTTP/1.1",
        "POST /v1beta/models/gemini-3.8-flash:generateContent HTTP/1.1",
        "POST /v1beta/models/gemini-3.8-flash:generateContent HTTP/1.1",
        "POST /v1beta/models/gemini-2.5-flash:generateContent HTTP/1.1",
    ]);
}

#[test]
fn gemini_modele_introuvable_bascule_directe_et_erreur_definitive() {
    let (base, journal) = serveur(vec![
        (404, r#"{"error":{"code":404,"message":"models/gemini-9 is not found"}}"#.into()),
        (200, r#"{"candidates":[{"content":{"parts":[{"text":"OK"}]}}]}"#.into()),
        (400, r#"{"error":{"code":400,"message":"API key not valid"}}"#.into()),
    ]);
    let msgs = [ia::MessageChat { role: "user".into(), texte: "Q".into() }];
    let (_, modele) = bloquer(ia::generer_gemini_resilient_sur(&base, "CLE", "gemini-9", None, &msgs, false, 0.2)).unwrap();
    assert_eq!(modele, "gemini-2.5-flash", "modèle introuvable : pas de nouvelle tentative inutile");
    // Clé invalide : aucune nouvelle tentative.
    let err = bloquer(ia::generer_gemini_resilient_sur(&base, "MAUVAISE", "gemini-2.5-flash", None, &msgs, false, 0.2)).unwrap_err();
    assert!(err.contains("API key not valid"));
    assert_eq!(journal.lock().unwrap().len(), 3);
}

#[test]
fn claude_refuse_modele_invalide_et_cle_absente() {
    let msgs = [ia::MessageChat { role: "user".into(), texte: "x".into() }];
    assert!(bloquer(ia::generer_claude_sur("http://127.0.0.1:9", "CLE", "../evil", None, &msgs, false)).is_err());
    assert!(bloquer(ia::generer_claude_sur("http://127.0.0.1:9", "", "claude-opus-5-5", None, &msgs, false)).unwrap_err().contains("non configurée"));
}

#[test]
fn openlegi_session_mcp_et_lecture_article() {
    let texte_outil = "=== ARTICLE CODE 1 ===\nArticle 641\n• Identifiant article: LEGIARTI000006411002\n• Numéro article: 641\n• État juridique: VIGUEUR\n• Date début vigueur: 01/01/1976\n• Lien article Légifrance: https://www.legifrance.gouv.fr/codes/article_lc/LEGIARTI000006411002\nTexte de l'article:\nLorsqu'un délai est exprimé en jours...\n";
    let resultat = serde_json::json!({"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":texte_outil}],"isError":false}});
    let (base, journal) = serveur_complet(vec![
        (200, r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-03-26","capabilities":{"tools":{}},"serverInfo":{"name":"OpenLegi"}}}"#.into(), "application/json".into(), "Mcp-Session-Id: SESSION-42\r\n".into()),
        (202, String::new(), "application/json".into(), String::new()),
        (200, format!("event: message\ndata: {}\n\n", resultat), "text/event-stream".into(), String::new()),
    ]);
    let cx = openlegi::Connexion::depuis(&format!("{}/mcp", base), "JETON-OL").unwrap();
    let a = tauri::async_runtime::block_on(openlegi::consulter_article(&cx, "Code de procédure civile", "641")).unwrap();
    assert_eq!(a.id, "LEGIARTI000006411002");
    assert_eq!(a.etat, "VIGUEUR");
    assert!(a.texte.starts_with("Lorsqu'un délai"));

    let j = journal.lock().unwrap();
    assert_eq!(j.len(), 3);
    assert_eq!(j[0].ligne, "POST /mcp HTTP/1.1");
    assert_eq!(j[0].entete("authorization"), "Bearer JETON-OL");
    assert!(j[0].entete("accept").contains("text/event-stream"));
    assert_eq!(j[0].json()["method"], "initialize");
    assert_eq!(j[1].json()["method"], "notifications/initialized");
    assert_eq!(j[1].entete("mcp-session-id"), "SESSION-42");
    let appel = j[2].json();
    assert_eq!(appel["method"], "tools/call");
    assert_eq!(appel["params"]["name"], "rechercher_code");
    assert_eq!(appel["params"]["arguments"]["code_name"], "Code de procédure civile");
    assert_eq!(appel["params"]["arguments"]["search"], "641");
    assert_eq!(appel["params"]["arguments"]["champ"], "NUM_ARTICLE");
    assert_eq!(j[2].entete("mcp-session-id"), "SESSION-42");
}

#[test]
fn openlegi_acces_refuse_et_jeton_dans_l_adresse() {
    let (base, journal) = serveur(vec![(401, r#"{"error":"unauthorized"}"#.into())]);
    let cx = openlegi::Connexion::depuis(&format!("{}/mcp?token=ABC", base), "").unwrap();
    let e = tauri::async_runtime::block_on(openlegi::consulter_article(&cx, "Code civil", "2224")).unwrap_err();
    assert!(e.contains("refusé l'accès"), "{}", e);
    let j = journal.lock().unwrap();
    assert_eq!(j[0].ligne, "POST /mcp?token=ABC HTTP/1.1");
    assert_eq!(j[0].entete("authorization"), "", "pas d'en-tête si le jeton est dans l'adresse");
}
