// =============================================================================
// Deadline Breaker - Interface (anglais / français ; droit français)
// Toute donnée métier est lue et écrite par le backend Rust (data.json local).
// =============================================================================
(() => {
'use strict';

// -----------------------------------------------------------------------------
// 0. LANGUE DE L'INTERFACE (anglais par défaut, français en option ; le droit reste français)
//    Les textes de l'interface sont écrits en français ; en anglais, ils sont traduits à
//    l'affichage d'après le dictionnaire ui/i18n-en.js (texte exact, ou gabarit avec {}).
//    Les éléments marqués translate="no" (textes de loi, saisies, réponses de l'IA) sont exclus.
// -----------------------------------------------------------------------------
const I18N = (() => {
    const dico = window.I18N_EN || { exact: {}, gabarits: [] };
    const exact = new Map(Object.entries(dico.exact || {}));
    const echapper = (s) => s.replace(/[.*+?^$()|[\]\\]/g, '\\$&');
    // {} : valeur quelconque ; {n} : nombre. Un gabarit doit comporter assez de texte fixe
    // (ou une valeur numérique) pour ne jamais capturer une phrase sans rapport.
    const gabarits = (dico.gabarits || [])
        .filter(([fr]) => /\{n\}/.test(fr) || fr.replace(/\{n?\}/g, '').trim().length >= 5)
        .slice()
        .sort((a, b) => b[0].replace(/\{n?\}/g, '').length - a[0].replace(/\{n?\}/g, '').length)
        .map(([fr, en]) => ({
            re: new RegExp('^' + fr.split(/(\{n?\})/).map(p => p === '{}' ? '([\\s\\S]+?)' : p === '{n}' ? '(\\d+)' : echapper(p)).join('') + '$'),
            en,
        }));
    const origines = new WeakMap();   // nœud texte -> texte français d'origine
    const posees = new WeakMap();     // nœud texte -> traduction posée (évite les boucles)
    const attributs = new WeakMap();  // élément -> { attribut: texte d'origine }
    const ATTRS = ['placeholder', 'title', 'aria-label'];
    let code = 'en';
    let observateur = null;
    const manquants = new Set();
    const FRANCAIS = /[éèêàùâîôûçœÉÈÀ]|\b(le|la|les|des|du|une|un|et|pour|avec|sans|aucun|aucune|dans|sur|au|aux|est|sont|vous|votre|vos)\b/i;

    function traduire(texte) {
        if (code !== 'en' || !texte) return texte;
        const m = texte.match(/^(\s*)([\s\S]*?)(\s*)$/);
        const coeur = m[2];
        if (!coeur) return texte;
        let tr = exact.get(coeur);
        if (tr === undefined) {
            for (const g of gabarits) {
                const r = coeur.match(g.re);
                if (r) {
                    let i = 1;
                    tr = g.en.replace(/\{n?\}/g, () => { const v = r[i++]; return exact.get(v) ?? v; });
                    break;
                }
            }
        }
        if (tr === undefined) {
            if (FRANCAIS.test(coeur) && coeur.length < 400) manquants.add(coeur);
            return texte;
        }
        return m[1] + tr + m[3];
    }

    function exclu(noeud) {
        const el = noeud.nodeType === 1 ? noeud : noeud.parentElement;
        if (!el) return true;
        // Les attributs (placeholder, title) d'un champ de saisie restent traduisibles.
        return !!el.closest('[translate="no"], script, style') || (noeud.nodeType === 3 && !!el.closest('textarea'));
    }

    function noeudTexte(n) {
        if (exclu(n)) return;
        const valeur = n.nodeValue;
        if (posees.get(n) === valeur) return;
        const origine = valeur;
        const tr = traduire(origine);
        origines.set(n, origine);
        if (tr !== valeur) {
            posees.set(n, tr);
            n.nodeValue = tr;
        }
    }

    function element(el) {
        if (exclu(el)) return;
        for (const a of ATTRS) {
            const v = el.getAttribute && el.getAttribute(a);
            if (!v) continue;
            const memo = attributs.get(el) || {};
            if (memo['_' + a] === v) continue;
            memo[a] = v;
            const tr = traduire(v);
            memo['_' + a] = tr;
            attributs.set(el, memo);
            if (tr !== v) el.setAttribute(a, tr);
        }
    }

    function parcourir(racine) {
        if (racine.nodeType === 3) { noeudTexte(racine); return; }
        if (racine.nodeType !== 1) return;
        element(racine);
        const tw = document.createTreeWalker(racine, NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT);
        let n;
        while ((n = tw.nextNode())) { if (n.nodeType === 3) noeudTexte(n); else element(n); }
    }

    function restaurer(racine) {
        const tw = document.createTreeWalker(racine, NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT);
        let n;
        while ((n = tw.nextNode())) {
            if (n.nodeType === 3) {
                const o = origines.get(n);
                if (o !== undefined && posees.get(n) === n.nodeValue) n.nodeValue = o;
                posees.delete(n);
            } else {
                const memo = attributs.get(n);
                if (memo) ATTRS.forEach(a => { if (memo[a] !== undefined && memo['_' + a] === n.getAttribute(a)) n.setAttribute(a, memo[a]); });
                attributs.delete(n);
            }
        }
    }

    function demarrerObservation() {
        if (observateur) return;
        observateur = new MutationObserver((mutations) => {
            for (const m of mutations) {
                if (m.type === 'childList') m.addedNodes.forEach(parcourir);
                else if (m.type === 'characterData') noeudTexte(m.target);
                else if (m.type === 'attributes') element(m.target);
            }
        });
        observateur.observe(document.body, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ATTRS });
    }

    return {
        code: () => code,
        t: (fr) => traduire(fr),
        manquants: () => [...manquants],
        appliquer(nouveau) {
            code = nouveau === 'fr' ? 'fr' : 'en';
            document.documentElement.lang = code;
            if (code === 'en') {
                parcourir(document.body);
                demarrerObservation();
            } else {
                if (observateur) { observateur.disconnect(); observateur = null; }
                restaurer(document.body);
            }
        },
    };
})();
window.__i18nManquants = () => I18N.manquants();

/** Texte de l'interface dans la langue courante (le français sert de clé). */
const t = (fr) => I18N.t(fr);
const langue = () => I18N.code();
const locale = () => (langue() === 'en' ? 'en-GB' : 'fr-FR');


// -----------------------------------------------------------------------------
// 0. PONT IPC TAURI
// -----------------------------------------------------------------------------
const TAURI = window.__TAURI__;

async function appelRust(commande, args = {}) {
    if (!TAURI || !TAURI.core || typeof TAURI.core.invoke !== 'function') {
        throw new Error("Moteur Deadline Breaker introuvable : l'interface doit être lancée depuis l'application (cargo tauri dev / exécutable).");
    }
    try {
        return await TAURI.core.invoke(commande, args);
    } catch (e) {
        throw new Error(typeof e === 'string' ? e : (e && e.message) || String(e));
    }
}

const etat = {
    donnees: { parametres: {}, affaires: [], historique: [] },
    regles: [],
    ref: { calendriers: [], regimes_prescription: [], matieres_competence: [], modeles_diligences: [] },
    infos: null,
    selectedId: null,
    filtre: 'ALL',
    vue: 'view-dashboard',
    dockLarge: false,
    chat: [],
    prescription: [],
    dernierExpress: null,
    dernierRetro: [],
    dernierePrescription: null,
    articleModal: null,
    articleOuvert: null,
    lienArticle: null,
    onglet: 'synthese',
    notesDossier: null,
    derniereCompetence: null,
    cleCompetence: null,
    expressJournalise: false,
    retroJournalise: false,
    agenda: null,
    paletteIndex: 0,
};

const P = () => etat.donnees.parametres || {};

// Mac : raccourcis ⌘ et libellés propres à macOS (Trousseau, ouverture de session).
// Appliqué avant la traduction, qui part ainsi des libellés adaptés.
const SUR_MAC = /Macintosh|Mac OS X/.test(navigator.userAgent || '');
function adapterAuSysteme() {
    document.documentElement.dataset.systeme = SUR_MAC ? 'macos' : 'windows';
    if (!SUR_MAC) return;
    document.querySelectorAll('kbd').forEach(k => { k.textContent = k.textContent.replace(/^Ctrl\s*/, '⌘ '); });
    document.querySelectorAll('[title*="Ctrl+"]').forEach(el => { el.title = el.title.replace(/Ctrl\+/g, '⌘'); });
    const libelle = document.querySelector('label:has(#param-demarrage-windows) span');
    if (libelle) libelle.textContent = "Lancer Deadline Breaker à l'ouverture de session macOS (fenêtre réduite), pour que les alertes partent même si l'application n'est pas ouverte";
}

function appliquer(reponse) {
    if (reponse && reponse.donnees) etat.donnees = reponse.donnees;
    if (etat.selectedId && !etat.donnees.affaires.some(a => a.id === etat.selectedId)) etat.selectedId = null;
    renderAll();
    return reponse;
}

// -----------------------------------------------------------------------------
// 1. OUTILS : ÉCHAPPEMENT, DATES LOCALES, NOTIFICATIONS, CONFIRMATION
// -----------------------------------------------------------------------------
function esc(v) {
    return String(v ?? '').replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/'/g, '&#39;');
}
const $ = (id) => document.getElementById(id);

function isoLocal(d) {
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}
function aujourdHui() { return isoLocal(new Date()); }
function parseIso(iso) {
    const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(String(iso || ''));
    return m ? new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3])) : null;
}
function joursJusqua(iso) {
    const d = parseIso(iso);
    if (!d) return null;
    const t = new Date();
    return Math.round((Date.UTC(d.getFullYear(), d.getMonth(), d.getDate()) - Date.UTC(t.getFullYear(), t.getMonth(), t.getDate())) / 86400000);
}
function joursEntre(isoA, isoB) {
    const a = parseIso(isoA), b = parseIso(isoB);
    if (!a || !b) return null;
    return Math.round((Date.UTC(b.getFullYear(), b.getMonth(), b.getDate()) - Date.UTC(a.getFullYear(), a.getMonth(), a.getDate())) / 86400000);
}
function fmt(iso) {
    const d = parseIso(iso);
    return d ? d.toLocaleDateString(locale()) : (iso || '-');
}
/** « vendredi 18 décembre 2026 » */
function dateEnLettres(iso) {
    if (!iso) return '-';
    return parseIso(iso).toLocaleDateString(locale(), { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' });
}
function fmtLong(iso) {
    const d = parseIso(iso);
    return d ? d.toLocaleDateString(locale(), { weekday: 'long', day: '2-digit', month: '2-digit', year: 'numeric' }) : (iso || '-');
}
function ajouterJours(iso, n) {
    const d = parseIso(iso); d.setDate(d.getDate() + n); return isoLocal(d);
}
function dernierJourDuMois(a, m) { return new Date(a, m + 1, 0).getDate(); }
/** Ajout de mois au même quantième, à défaut le dernier jour du mois. */
function ajouterMois(iso, n) {
    const d = parseIso(iso);
    const total = d.getFullYear() * 12 + d.getMonth() + n;
    const a = Math.floor(total / 12), m = total % 12;
    return isoLocal(new Date(a, m, Math.min(d.getDate(), dernierJourDuMois(a, m))));
}
function euros(n) { return (Number(n) || 0).toLocaleString(locale(), { style: 'currency', currency: 'EUR', minimumFractionDigits: 0, maximumFractionDigits: 2 }); }

function toast(message, type = 'info', duree = 4200) {
    const zone = $('toast-zone');
    const couleurs = { info: 'bg-slate-900 text-white', ok: 'bg-emerald-700 text-white', erreur: 'bg-rose-700 text-white', alerte: 'bg-amber-600 text-white' };
    const el = document.createElement('div');
    el.className = `pointer-events-auto max-w-xl px-3.5 py-2 rounded-lg shadow-lg text-xs font-semibold select-text ${couleurs[type] || couleurs.info}`;
    el.textContent = message;
    while (zone.children.length >= 3) zone.firstElementChild.remove();
    zone.appendChild(el);
    setTimeout(() => el.remove(), type === 'erreur' ? Math.max(duree, 7000) : duree);
}
function erreur(e) { console.error(e); toast(e && e.message ? e.message : String(e), 'erreur'); }

function uiConfirm(titre, message, libelleOk = 'Confirmer') {
    return new Promise((resolve) => {
        const modal = $('modal-confirm');
        $('modal-confirm-title').textContent = titre;
        $('modal-confirm-message').textContent = message;
        $('btn-confirm-ok').textContent = libelleOk;
        modal.classList.remove('hidden'); modal.classList.add('flex');
        const fin = (v) => { modal.classList.add('hidden'); modal.classList.remove('flex'); $('btn-confirm-ok').onclick = null; $('btn-confirm-cancel').onclick = null; resolve(v); };
        $('btn-confirm-ok').onclick = () => fin(true);
        $('btn-confirm-cancel').onclick = () => fin(false);
    });
}

function ouvrirModal(id) { $(id).classList.remove('hidden'); $(id).classList.add('flex'); }
function fermerModal(id) { $(id).classList.add('hidden'); $(id).classList.remove('flex'); }
function icones() { if (window.lucide) window.lucide.createIcons(); }

async function avecBouton(bouton, action) {
    if (bouton) bouton.disabled = true;
    try { return await action(); } catch (e) { erreur(e); } finally { if (bouton) bouton.disabled = false; }
}

async function exporterFichier(nom, contenu, bom = false) {
    const chemin = await appelRust('exporter_fichier', { nom, contenu, bom });
    toast(`Fichier enregistré : ${chemin}`, 'ok', 6500);
    return chemin;
}

// -----------------------------------------------------------------------------
// 2. BASE LOCALE DES TEXTES (vérifiée sur Légifrance le 18/11/2026) - repli hors-ligne
//    Lecture en direct via OpenLegi lorsqu'il est configuré ; liens vers legifrance.gouv.fr dans tous les cas.
// -----------------------------------------------------------------------------
const TEXTES_LOCAUX = {
    CPC_42: { code: 'Code de procédure civile', id: 'LEGIARTI000006410140', texte: "La juridiction territorialement compétente est, sauf disposition contraire, celle du lieu où demeure le défendeur.\n\nS'il y a plusieurs défendeurs, le demandeur saisit, à son choix, la juridiction du lieu où demeure l'un d'eux.\n\nSi le défendeur n'a ni domicile ni résidence connus, le demandeur peut saisir la juridiction du lieu où il demeure ou celle de son choix s'il demeure à l'étranger." },
    CPC_43: { code: 'Code de procédure civile', id: 'LEGIARTI000006410141', texte: "Le lieu où demeure le défendeur s'entend :\n\n- s'il s'agit d'une personne physique, du lieu où celle-ci a son domicile ou, à défaut, sa résidence ;\n\n- s'il s'agit d'une personne morale, du lieu où celle-ci est établie." },
    CPC_44: { code: 'Code de procédure civile', id: 'LEGIARTI000006410142', texte: "En matière réelle immobilière, la juridiction du lieu où est situé l'immeuble est seule compétente." },
    CPC_45: { code: 'Code de procédure civile', id: 'LEGIARTI000006410143', texte: "En matière de succession, sont portées devant la juridiction dans le ressort de laquelle est ouverte la succession jusqu'au partage inclusivement :\n\n- les demandes entre héritiers ;\n\n- les demandes formées par les créanciers du défunt ;\n\n- les demandes relatives à l'exécution des dispositions à cause de mort." },
    CPC_46: { code: 'Code de procédure civile', id: 'LEGIARTI000006410145', texte: "Le demandeur peut saisir à son choix, outre la juridiction du lieu où demeure le défendeur :\n\n- en matière contractuelle, la juridiction du lieu de la livraison effective de la chose ou du lieu de l'exécution de la prestation de service ;\n\n- en matière délictuelle, la juridiction du lieu du fait dommageable ou celle dans le ressort de laquelle le dommage a été subi ;\n\n- en matière mixte, la juridiction du lieu où est situé l'immeuble ;\n\n- en matière d'aliments ou de contribution aux charges du mariage, la juridiction du lieu où demeure le créancier." },
    CPC_853: { code: 'Code de procédure civile', id: 'LEGIARTI000044200291', texte: "Les parties sont, sauf disposition contraire, tenues de constituer avocat devant le tribunal de commerce.\n\nLa constitution de l'avocat emporte élection de domicile.\n\nLes parties sont dispensées de l'obligation de constituer avocat dans les cas prévus par la loi ou le règlement, lorsque la demande porte sur un montant inférieur ou égal à 10 000 euros ou qu'elle a pour origine l'exécution d'une obligation dont le montant n'excède pas 10 000 euros, dans le cadre des procédures instituées par le livre VI du code de commerce ou pour les litiges relatifs à la tenue du registre du commerce et des sociétés. Le montant de la demande est apprécié conformément aux dispositions des articles 35 à 37.\n\nDans ces cas, elles ont la faculté de se faire assister ou représenter par toute personne de leur choix. Le représentant, s'il n'est avocat, doit justifier d'un pouvoir spécial. [...]" },
    'CTRAV_R1453-1': { code: 'Code du travail', id: 'LEGIARTI000032580149', texte: "Les parties se défendent elles-mêmes.\n\nElles ont la faculté de se faire assister ou représenter." },
    CPC_74: { code: 'Code de procédure civile', id: 'LEGIARTI000006410179', texte: "Les exceptions doivent, à peine d'irrecevabilité, être soulevées simultanément et avant toute défense au fond ou fin de non-recevoir. Il en est ainsi alors même que les règles invoquées au soutien de l'exception seraient d'ordre public.\n\nLa demande de communication de pièces ne constitue pas une cause d'irrecevabilité des exceptions.\n\nLes dispositions de l'alinéa premier ne font pas non plus obstacle à l'application des articles 103, 111, 112 et 118." },
    CPC_75: { code: 'Code de procédure civile', id: 'LEGIARTI000034756977', texte: "S'il est prétendu que la juridiction saisie en première instance ou en appel est incompétente, la partie qui soulève cette exception doit, à peine d'irrecevabilité, la motiver et faire connaître dans tous les cas devant quelle juridiction elle demande que l'affaire soit portée." },
    CPC_114: { code: 'Code de procédure civile', id: 'LEGIARTI000006410221', texte: "Aucun acte de procédure ne peut être déclaré nul pour vice de forme si la nullité n'en est pas expressément prévue par la loi, sauf en cas d'inobservation d'une formalité substantielle ou d'ordre public.\n\nLa nullité ne peut être prononcée qu'à charge pour l'adversaire qui l'invoque de prouver le grief que lui cause l'irrégularité, même lorsqu'il s'agit d'une formalité substantielle ou d'ordre public." },
    CPC_117: { code: 'Code de procédure civile', id: 'LEGIARTI000006410224', texte: "Constituent des irrégularités de fond affectant la validité de l'acte :\n\nLe défaut de capacité d'ester en justice ;\n\nLe défaut de pouvoir d'une partie ou d'une personne figurant au procès comme représentant soit d'une personne morale, soit d'une personne atteinte d'une incapacité d'exercice ;\n\nLe défaut de capacité ou de pouvoir d'une personne assurant la représentation d'une partie en justice." },
    CPC_122: { code: 'Code de procédure civile', id: 'LEGIARTI000006410229', texte: "Constitue une fin de non-recevoir tout moyen qui tend à faire déclarer l'adversaire irrecevable en sa demande, sans examen au fond, pour défaut de droit d'agir, tel le défaut de qualité, le défaut d'intérêt, la prescription, le délai préfix, la chose jugée." },
    CPC_538: { code: 'Code de procédure civile', id: 'LEGIARTI000006410858', texte: "Le délai de recours par une voie ordinaire est d'un mois en matière contentieuse ; il est de quinze jours en matière gracieuse." },
    CPC_574: { code: 'Code de procédure civile', id: 'LEGIARTI000006410908', texte: "L'opposition doit contenir les moyens du défaillant." },
    CPC_612: { code: 'Code de procédure civile', id: 'LEGIARTI000006410964', texte: "Le délai de pourvoi en cassation est de deux mois, sauf disposition contraire." },
    CPC_640: { code: 'Code de procédure civile', id: 'LEGIARTI000006411001', texte: "Lorsqu'un acte ou une formalité doit être accompli avant l'expiration d'un délai, celui-ci a pour origine la date de l'acte, de l'événement, de la décision ou de la notification qui le fait courir." },
    CPC_641: { code: 'Code de procédure civile', id: 'LEGIARTI000006411002', texte: "Lorsqu'un délai est exprimé en jours, celui de l'acte, de l'événement, de la décision ou de la notification qui le fait courir ne compte pas.\n\nLorsqu'un délai est exprimé en mois ou en années, ce délai expire le jour du dernier mois ou de la dernière année qui porte le même quantième que le jour de l'acte, de l'événement, de la décision ou de la notification qui fait courir le délai. A défaut d'un quantième identique, le délai expire le dernier jour du mois.\n\nLorsqu'un délai est exprimé en mois et en jours, les mois sont d'abord décomptés, puis les jours." },
    CPC_642: { code: 'Code de procédure civile', id: 'LEGIARTI000006411003', texte: "Tout délai expire le dernier jour à vingt-quatre heures.\n\nLe délai qui expirerait normalement un samedi, un dimanche ou un jour férié ou chômé est prorogé jusqu'au premier jour ouvrable suivant." },
    CPC_643: { code: 'Code de procédure civile', id: 'LEGIARTI000034747123', texte: "Lorsque la demande est portée devant une juridiction qui a son siège en France métropolitaine, les délais de comparution, d'appel, d'opposition, de tierce opposition dans l'hypothèse prévue à l'article 586 alinéa 3, de recours en révision et de pourvoi en cassation sont augmentés de :\n\n1. Un mois pour les personnes qui demeurent en Guadeloupe, en Guyane, à la Martinique, à La Réunion, à Mayotte, à Saint-Barthélemy, à Saint-Martin, à Saint-Pierre-et-Miquelon, en Polynésie française, dans les îles Wallis et Futuna, en Nouvelle-Calédonie et dans les Terres australes et antarctiques françaises ;\n\n2. Deux mois pour celles qui demeurent à l'étranger." },
    CPC_761: { code: 'Code de procédure civile', id: 'LEGIARTI000051869371', texte: "Les parties sont dispensées de constituer avocat dans les cas prévus par la loi ou le règlement et dans les cas suivants :\n\n1° Dans les matières relevant de la compétence du juge des contentieux de la protection ;\n\n2° Dans les matières énumérées par les articles R. 211-3-13 à R. 211-3-21, R. 211-3-23 du code de l'organisation judiciaire et dans les matières énumérées au tableau IV-II annexé au code de l'organisation judiciaire ;\n\n3° A l'exclusion des matières relevant de la compétence exclusive du tribunal judiciaire, lorsque la demande porte sur un montant inférieur ou égal à 10 000 euros ou a pour objet une demande indéterminée ayant pour origine l'exécution d'une obligation dont le montant n'excède pas 10 000 euros. Le montant de la demande est apprécié conformément aux dispositions des articles 35 à 37. [...]" },
    CPC_763: { code: 'Code de procédure civile', id: 'LEGIARTI000042597306', texte: "Lorsque la représentation par avocat est obligatoire, le défendeur est tenu de constituer avocat dans le délai de quinze jours, à compter de l'assignation.\n\nToutefois, si l'assignation lui est délivrée dans un délai inférieur ou égal à quinze jours avant la date de l'audience, il peut constituer avocat jusqu'à l'audience." },
    CPC_908: { code: 'Code de procédure civile', id: 'LEGIARTI000048869023', texte: "A peine de caducité de la déclaration d'appel, relevée d'office, l'appelant dispose d'un délai de trois mois à compter de la déclaration d'appel pour remettre ses conclusions au greffe." },
    CPC_909: { code: 'Code de procédure civile', id: 'LEGIARTI000048868939', texte: "L'intimé dispose, à peine d'irrecevabilité relevée d'office, d'un délai de trois mois à compter de la notification qui lui est faite des conclusions de l'appelant prévues à l'article 908 pour remettre ses conclusions au greffe et former, le cas échéant, appel incident ou appel provoqué." },
    CCIV_2224: { code: 'Code civil', id: 'LEGIARTI000019017112', texte: "Les actions personnelles ou mobilières se prescrivent par cinq ans à compter du jour où le titulaire d'un droit a connu ou aurait dû connaître les faits lui permettant de l'exercer." },
    CCIV_2226: { code: 'Code civil', id: 'LEGIARTI000019017259', texte: "L'action en responsabilité née à raison d'un événement ayant entraîné un dommage corporel, engagée par la victime directe ou indirecte des préjudices qui en résultent, se prescrit par dix ans à compter de la date de la consolidation du dommage initial ou aggravé.\n\nToutefois, en cas de préjudice causé par des tortures ou des actes de barbarie, ou par des violences ou des agressions sexuelles commises contre un mineur, l'action en responsabilité civile est prescrite par vingt ans." },
    CCIV_2231: { code: 'Code civil', id: 'LEGIARTI000019017290', texte: "L'interruption efface le délai de prescription acquis. Elle fait courir un nouveau délai de même durée que l'ancien." },
    CCIV_2238: { code: 'Code civil', id: 'LEGIARTI000032042671', texte: "La prescription est suspendue à compter du jour où, après la survenance d'un litige, les parties conviennent de recourir à la médiation ou à la conciliation ou, à défaut d'accord écrit, à compter du jour de la première réunion de médiation ou de conciliation. La prescription est également suspendue à compter de la conclusion d'une convention de procédure participative ou à compter de l'accord du débiteur constaté par l'huissier de justice pour participer à la procédure prévue à l'article L. 125-1 du code des procédures civiles d'exécution.\n\nLe délai de prescription recommence à courir, pour une durée qui ne peut être inférieure à six mois, à compter de la date à laquelle soit l'une des parties ou les deux, soit le médiateur ou le conciliateur déclarent que la médiation ou la conciliation est terminée. En cas de convention de procédure participative, le délai de prescription recommence à courir à compter du terme de la convention, pour une durée qui ne peut être inférieure à six mois. En cas d'échec de la procédure prévue au même article, le délai de prescription recommence à courir à compter de la date du refus du débiteur, constaté par l'huissier, pour une durée qui ne peut être inférieure à six mois." },
    CCIV_2239: { code: 'Code civil', id: 'LEGIARTI000019017360', texte: "La prescription est également suspendue lorsque le juge fait droit à une demande de mesure d'instruction présentée avant tout procès.\n\nLe délai de prescription recommence à courir, pour une durée qui ne peut être inférieure à six mois, à compter du jour où la mesure a été exécutée." },
    CCIV_2241: { code: 'Code civil', id: 'LEGIARTI000019017098', texte: "La demande en justice, même en référé, interrompt le délai de prescription ainsi que le délai de forclusion.\n\nIl en est de même lorsqu'elle est portée devant une juridiction incompétente ou lorsque l'acte de saisine de la juridiction est annulé par l'effet d'un vice de procédure." },
    CCIV_2242: { code: 'Code civil', id: 'LEGIARTI000019017096', texte: "L'interruption résultant de la demande en justice produit ses effets jusqu'à l'extinction de l'instance." },
    CCIV_2243: { code: 'Code civil', id: 'LEGIARTI000019017094', texte: "L'interruption est non avenue si le demandeur se désiste de sa demande ou laisse périmer l'instance, ou si sa demande est définitivement rejetée." },
    CPC_490: { code: 'Code de procédure civile', id: 'LEGIARTI000006410760', texte: "L'ordonnance de référé peut être frappée d'appel à moins qu'elle n'émane du premier président de la cour d'appel ou qu'elle n'ait été rendue en dernier ressort en raison du montant ou de l'objet de la demande.\n\nL'ordonnance rendue en dernier ressort par défaut est susceptible d'opposition.\n\nLe délai d'appel ou d'opposition est de quinze jours." },
    CPC_644: { code: 'Code de procédure civile', id: 'LEGIARTI000034747120', texte: "Lorsque la demande est portée devant une juridiction qui a son siège en Guadeloupe, en Guyane, à la Martinique, à La Réunion, à Mayotte, à Saint-Barthélemy, à Saint-Martin, à Saint-Pierre-et-Miquelon et dans les îles Wallis et Futuna, les délais de comparution, d'appel, d'opposition de tierce opposition dans l'hypothèse prévue à l'article 586 alinéa 3, et de recours en révision sont augmentés d'un mois pour les personnes qui ne demeurent pas dans la collectivité territoriale dans le ressort de laquelle la juridiction a son siège et de deux mois pour les personnes qui demeurent à l'étranger." },
    'CPC_906-2': { code: 'Code de procédure civile', id: 'LEGIARTI000048852560', texte: "A peine de caducité de la déclaration d'appel, relevée d'office par ordonnance du président de la chambre saisie ou du magistrat désigné par le premier président, l'appelant dispose d'un délai de deux mois à compter de la réception de l'avis de fixation de l'affaire à bref délai pour remettre ses conclusions au greffe.\n\nL'intimé dispose, à peine d'irrecevabilité relevée d'office par ordonnance du président de la chambre saisie ou du magistrat désigné par le premier président, d'un délai de deux mois à compter de la notification des conclusions de l'appelant pour remettre ses conclusions au greffe et former, le cas échéant, appel incident ou appel provoqué.\n\n[...] Le président de la chambre saisie ou le magistrat désigné par le premier président peut, à la demande d'une partie ou d'office, allonger ou réduire les délais prévus aux alinéas précédents. [...]" },
    CCIV_1648: { code: 'Code civil', id: 'LEGIARTI000020466328', texte: "L'action résultant des vices rédhibitoires doit être intentée par l'acquéreur dans un délai de deux ans à compter de la découverte du vice.\n\nDans le cas prévu par l'article 1642-1, l'action doit être introduite, à peine de forclusion, dans l'année qui suit la date à laquelle le vendeur peut être déchargé des vices ou des défauts de conformité apparents." },
    CCIV_2220: { code: 'Code civil', id: 'LEGIARTI000019017126', texte: "Les délais de forclusion ne sont pas, sauf dispositions contraires prévues par la loi, régis par le présent titre." },
    CCIV_2227: { code: 'Code civil', id: 'LEGIARTI000019017108', texte: "Le droit de propriété est imprescriptible. Sous cette réserve, les actions réelles immobilières se prescrivent par trente ans à compter du jour où le titulaire d'un droit a connu ou aurait dû connaître les faits lui permettant de l'exercer." },
    CCIV_2232: { code: 'Code civil', id: 'LEGIARTI000033033506', texte: "Le report du point de départ, la suspension ou l'interruption de la prescription ne peut avoir pour effet de porter le délai de la prescription extinctive au-delà de vingt ans à compter du jour de la naissance du droit.\n\nLe premier alinéa n'est pas applicable dans les cas mentionnés aux articles 2226, 2226-1, 2227, 2233 et 2236, au premier alinéa de l'article 2241 et à l'article 2244. Il ne s'applique pas non plus aux actions relatives à l'état des personnes." },
    'CTRAV_R1461-1': { code: 'Code du travail', id: 'LEGIARTI000032580273', texte: "Le délai d'appel est d'un mois.\n\nA défaut d'être représentées par la personne mentionnée au 2° de l'article R. 1453-2, les parties sont tenues de constituer avocat. [...]" },
    'CTRAV_L3245-1': { code: 'Code du travail', id: 'LEGIARTI000027566295', texte: "L'action en paiement ou en répétition du salaire se prescrit par trois ans à compter du jour où celui qui l'exerce a connu ou aurait dû connaître les faits lui permettant de l'exercer. La demande peut porter sur les sommes dues au titre des trois dernières années à compter de ce jour ou, lorsque le contrat de travail est rompu, sur les sommes dues au titre des trois années précédant la rupture du contrat." },
    'CTRAV_L3422-2': { code: 'Code du travail', id: 'LEGIARTI000035902463', texte: "En Guadeloupe, en Guyane, en Martinique, à Mayotte, à La Réunion, à Saint-Barthélemy et à Saint-Martin, les journées de commémoration de l'abolition de l'esclavage ci-après désignées sont des jours fériés :\n\n1° Le 27 avril à Mayotte ;\n2° Le 22 mai en Martinique ;\n3° Le 27 mai en Guadeloupe et à Saint-Martin ;\n4° Le 10 juin en Guyane ;\n5° Le 9 octobre à Saint-Barthélemy ;\n6° Le 20 décembre à La Réunion." },
    'CPCE_R121-20': { code: "Code des procédures civiles d'exécution", id: 'LEGIARTI000048869099', texte: "Le délai d'appel est de quinze jours à compter de la notification de la décision.\n\nL'appel est formé, instruit et jugé selon les règles applicables à la procédure à bref délai ou à la procédure à jour fixe." },
    'CCONSO_L218-2': { code: 'Code de la consommation', id: 'LEGIARTI000032226897', texte: "L'action des professionnels, pour les biens ou les services qu'ils fournissent aux consommateurs, se prescrit par deux ans." },
    'COJ_L213-6': { code: "Code de l'organisation judiciaire", id: 'LEGIARTI000054142606', texte: "Le juge de l'exécution connaît, de manière exclusive, des difficultés relatives aux titres exécutoires et des contestations qui s'élèvent à l'occasion de l'exécution forcée, même si elles portent sur le fond du droit, à moins qu'elles n'échappent à la compétence des juridictions de l'ordre judiciaire.\n\nDans les mêmes conditions, il autorise les mesures conservatoires et connaît des contestations relatives à leur mise en oeuvre.\n\nLe juge de l'exécution connaît, sous la même réserve, de la procédure de saisie immobilière, des contestations qui s'élèvent à l'occasion de celle-ci et des demandes nées de cette procédure ou s'y rapportant directement, même si elles portent sur le fond du droit ainsi que de la procédure de distribution qui en découle. [...]" },
    'COJ_L213-4-4': { code: "Code de l'organisation judiciaire", id: 'LEGIARTI000038274638', texte: "Le juge des contentieux de la protection connaît des actions dont un contrat de louage d'immeubles à usage d'habitation ou un contrat portant sur l'occupation d'un logement est l'objet, la cause ou l'occasion ainsi que des actions relatives à l'application de la loi n° 48-1360 du 1er septembre 1948 [...]." },
    'COJ_L213-4-5': { code: "Code de l'organisation judiciaire", id: 'LEGIARTI000038274640', texte: "Le juge des contentieux de la protection connaît des actions relatives à l'application du chapitre II du titre Ier du livre III du code de la consommation." },
    'COJ_R211-3-24': { code: "Code de l'organisation judiciaire", id: 'LEGIARTI000039013368', texte: "Lorsque le tribunal judiciaire est appelé à connaître, en matière civile, d'une action personnelle ou mobilière portant sur une demande dont le montant est inférieur ou égal à la somme de 5 000 euros, le tribunal judiciaire statue en dernier ressort." },
};

function boutonArticle(code, num, libelle, classes = 'text-sky-700') {
    return `<button type="button" class="btn-legal-article ${classes} font-bold underline" data-code="${esc(code)}" data-article="${esc(num)}">${esc(libelle || ('Art. ' + num))}</button>`;
}

const NOMS_CODES = { CPC: 'Code de procédure civile', CCIV: 'Code civil', COJ: "Code de l'organisation judiciaire", CTRAV: 'Code du travail', CPCE: "Code des procédures civiles d'exécution", CCONSO: 'Code de la consommation' };
const openlegiConfigure = () => !!(P().openlegi_url || '').trim();

/** Code interne d'après sa mention dans un texte (« CPC », « C. civ. », « Code du travail »...). */
function codeDepuis(t) {
    const x = t.replace(/&#39;|’/g, "'").toLowerCase().replace(/\s+/g, ' ').trim().replace(/^(du|de la) /, '');
    if (x === 'cpce' || x.includes("procédures civiles d'exécution")) return 'CPCE';
    if (x === 'cpc' || x.includes('procédure civile')) return 'CPC';
    if (x.startsWith('c. civ') || x.startsWith('c.civ') || x === 'code civil') return 'CCIV';
    if (x === 'coj' || x.includes('organisation judiciaire')) return 'COJ';
    if (x.startsWith('c. trav') || x.startsWith('c.trav') || x.includes('code du travail')) return 'CTRAV';
    if (x.startsWith('c. conso') || x.includes('consommation')) return 'CCONSO';
    return null;
}
const MOTIF_ARTICLE = /\b(art(?:icles?|s?\.)?)\s+((?:[LRD]\.?\s?)?\d+(?:-\d+)*)((?:\s+(?:al\.|alinéa)\s*\d+|\s+\d+°)?)(\s+(?:du\s+|de\s+la\s+)?(?:CPCE|CPC|Code de procédure civile|C\.\s?civ\.|Code civil|COJ|Code de l(?:'|&#39;|’)organisation judiciaire|C\.\s?trav\.|Code du travail|Code des procédures civiles d(?:'|&#39;|’)exécution|C\.\s?conso\.|Code de la consommation))?/gi;

/** Transforme les mentions d'articles d'un texte DÉJÀ échappé en liens cliquables. */
function lierArticles(html, codeParDefaut = null) {
    return String(html).replace(MOTIF_ARTICLE, (tout, _art, num, _al, codeTexte) => {
        const code = codeTexte ? codeDepuis(codeTexte) : codeParDefaut;
        if (!code) return tout;
        const n = num.replace(/[\s.]/g, '').toUpperCase();
        return `<button type="button" class="btn-legal-article text-sky-700 font-semibold underline decoration-dotted" data-code="${code}" data-article="${esc(n)}" title="Lire le texte et ouvrir sur Légifrance">${tout}</button>`;
    });
}

function afficherArticle({ titre, code, texte, badge, badgeClasse, vigueur, id, url, note }) {
    $('modal-legi-title').textContent = titre;
    $('modal-legi-code').textContent = code;
    $('modal-legi-text').textContent = texte;
    $('modal-legi-date-vigueur').innerHTML = vigueur;
    $('modal-legi-id').textContent = id || '';
    $('modal-legi-url').textContent = [url, note].filter(Boolean).join('\n');
    const b = $('modal-legi-source-badge');
    b.textContent = badge;
    b.className = `px-2 py-0.5 rounded text-[10px] font-bold uppercase ${badgeClasse}`;
    icones();
}

function afficherArticleLocal(code, num, note) {
    const local = TEXTES_LOCAUX[`${code}_${num}`];
    if (local) {
        afficherArticle({
            titre: `Article ${num}`, code: local.code, texte: local.texte, id: local.id,
            url: `https://www.legifrance.gouv.fr/codes/article_lc/${local.id}`, note,
            badge: 'Base locale vérifiée', badgeClasse: 'bg-sky-100 text-sky-800',
            vigueur: `<i data-lucide="check-circle" class="w-3.5 h-3.5 text-emerald-600"></i> Base locale - texte vérifié sur Légifrance le 18/11/2026`,
        });
        etat.articleModal = { article: `article ${num}`, code: local.code, texte: local.texte };
        etat.lienArticle = `https://www.legifrance.gouv.fr/codes/article_lc/${local.id}`;
    } else {
        etat.lienArticle = null;
        afficherArticle({
            titre: `Article ${num}`, code: NOMS_CODES[code] || code, note,
            texte: openlegiConfigure() ? "Texte indisponible : la consultation a échoué (voir ci-dessous). Le bouton « Ouvrir sur Légifrance » reste disponible." : "Texte non disponible hors-ligne. Utilisez « Ouvrir sur Légifrance », ou configurez OpenLegi dans les Paramètres pour lire tous les articles ici.",
            badge: 'Indisponible', badgeClasse: 'bg-amber-100 text-amber-800',
            vigueur: `<i data-lucide="info" class="w-3.5 h-3.5 text-amber-600"></i> Non disponible`,
        });
        etat.articleModal = null;
    }
}

let jetonArticle = 0;
async function ouvrirArticle(code, num, forcer = false) {
    etat.articleOuvert = { code, num };
    etat.lienArticle = null;
    ouvrirModal('modal-legifrance');
    $('btn-refresh-legi').classList.toggle('hidden', !openlegiConfigure());
    if (!openlegiConfigure()) { afficherArticleLocal(code, num, ''); return; }
    const jeton = ++jetonArticle;
    afficherArticle({
        titre: `Article ${num}`, code: NOMS_CODES[code] || code, texte: 'Consultation de Légifrance en cours...',
        badge: 'Légifrance...', badgeClasse: 'bg-amber-100 text-amber-800 animate-pulse',
        vigueur: `<i data-lucide="loader-2" class="w-3.5 h-3.5 animate-spin"></i> Lecture via OpenLegi`,
    });
    try {
        const a = await appelRust('consulter_article', { code, num, forcer });
        if (jeton !== jetonArticle) return;
        const enVigueur = !a.etat || a.etat === 'VIGUEUR';
        afficherArticle({
            titre: `Article ${a.num || num}`, code: a.code_libelle || NOMS_CODES[code] || code, texte: a.texte, id: a.id, url: a.url,
            note: a.avertissement || '',
            badge: a.source === 'OPENLEGI' ? 'Légifrance - en direct' : 'Légifrance - copie locale',
            badgeClasse: a.avertissement || !enVigueur ? 'bg-amber-100 text-amber-800' : 'bg-emerald-100 text-emerald-800',
            vigueur: `<i data-lucide="${enVigueur ? 'check-circle' : 'alert-triangle'}" class="w-3.5 h-3.5 ${enVigueur ? 'text-emerald-600' : 'text-amber-600'}"></i> ${enVigueur ? 'En vigueur' : 'État : ' + esc(a.etat)}${a.date_debut ? ' depuis le ' + fmt(a.date_debut) : ''} - ${a.source === 'OPENLEGI' ? 'lu via OpenLegi' : 'copie locale'} le ${esc(a.recupere_le)}`,
        });
        etat.articleModal = { article: `article ${a.num || num}`, code: a.code_libelle || NOMS_CODES[code], texte: a.texte };
        etat.lienArticle = (a.url || '').startsWith('https://www.legifrance.gouv.fr/') ? a.url : null;
    } catch (e) {
        if (jeton !== jetonArticle) return;
        afficherArticleLocal(code, num, e.message === 'NON_CONFIGURE' ? '' : `OpenLegi indisponible : ${e.message}`);
    }
}

function initModalLegifrance() {
    document.addEventListener('click', (e) => {
        const b = e.target.closest('.btn-legal-article');
        if (b) { e.preventDefault(); e.stopPropagation(); ouvrirArticle(b.dataset.code, b.dataset.article); }
    });
    $('btn-close-modal-legifrance').onclick = () => fermerModal('modal-legifrance');
    $('btn-open-legifrance').onclick = (e) => avecBouton(e.currentTarget, async () => {
        const o = etat.articleOuvert;
        if (!o) return;
        const precis = !!etat.lienArticle;
        const url = etat.lienArticle || await appelRust('lien_legifrance', { code: o.code, num: o.num });
        await appelRust('ouvrir_legifrance', { url });
        toast(precis ? 'Article ouvert sur Légifrance dans votre navigateur.' : "Article non identifié précisément : le code complet s'ouvre sur Légifrance.", precis ? 'ok' : 'info');
    });
    $('btn-refresh-legi').onclick = () => { if (etat.articleOuvert) ouvrirArticle(etat.articleOuvert.code, etat.articleOuvert.num, true); };
    $('btn-copy-legi-text').onclick = async () => {
        try { await navigator.clipboard.writeText($('modal-legi-text').innerText); toast('Texte copié.', 'ok'); } catch (e) { erreur(e); }
    };
    $('btn-insert-legi-to-ai').onclick = () => {
        if (!etat.articleModal) return toast("Aucun texte à insérer.", 'alerte');
        fermerModal('modal-legifrance');
        ouvrirDock();
        const a = etat.articleModal;
        $('chat-user-input').value = `Voici le texte en vigueur de l'${a.article} du ${a.code} :\n\n« ${a.texte} »\n\nAnalyse son application à notre dossier et indique comment le valoriser dans nos écritures.`;
        $('chat-user-input').focus();
    };
}

// -----------------------------------------------------------------------------
// 3. INITIALISATION
// -----------------------------------------------------------------------------
window.addEventListener('DOMContentLoaded', async () => {
    adapterAuSysteme();
    initNavigation();
    initModalLegifrance();
    initModals();
    initDiligences();
    initPalette();
    initDockIA();
    initExpress();
    initPrescription();
    initIncidents();
    initCompetence();
    initParametres();
    initJournal();
    initAgenda();
    initAnalyse();
    initLangue();
    I18N.appliquer(lirePreference('lexprocedura_langue', 'en'));

    try {
        const [donnees, regles, infos, ref] = await Promise.all([appelRust('charger_donnees'), appelRust('lister_regles_delais'), appelRust('obtenir_infos_stockage'), appelRust('lister_referentiels')]);
        etat.donnees = donnees;
        etat.regles = regles;
        etat.infos = infos;
        etat.ref = ref;
        remplirSelectRegles($('exp-delai-select'), false);
        remplirSelectRegles($('modal-act-delai'), true);
        remplirReferentiels();
        if ((P().langue || 'en') !== langue()) I18N.appliquer(P().langue || 'en');
        ecrirePreference('lexprocedura_langue', langue());
        remplirParametres();
        (infos.avertissements || []).forEach(a => toast(a, 'alerte', 12000));
        await reprendreDonneesNavigateur();
    } catch (e) {
        afficherErreurFatale(e);
        return;
    }

    const today = aujourdHui();
    $('exp-date-input').value = today;
    $('sim-date-cible').value = today;
    $('retro-date-audience').value = '';
    $('exp-calendrier-select').value = P().calendrier_defaut || 'METROPOLE';
    majModeExpress();
    calculerRetro().catch(() => {});

    renderAll();
    renderEvenements();
    computePrescription();
    recomputeDispositif();
    ecouterMisesAJour();
    recomputeCompetence();
    renderAiWelcomeMessage();
});

/** Alertes envoyées en arrière-plan par le moteur : la base a changé, on la relit. */
function ecouterMisesAJour() {
    const ev = window.__TAURI__ && window.__TAURI__.event;
    if (!ev || typeof ev.listen !== 'function') return;
    ev.listen('donnees-modifiees', async () => {
        try { appliquer({ donnees: await appelRust('charger_donnees') }); toast('Récapitulatif des échéances envoyé par e-mail.', 'ok'); }
        catch (e) { console.error(e); }
    });
}

function afficherErreurFatale(e) {
    console.error(e);
    const main = document.querySelector('main');
    main.innerHTML = `<div class="m-auto max-w-lg bg-white border border-rose-300 rounded-xl p-5 text-xs text-slate-800 shadow-xs space-y-2">
        <h2 class="text-sm font-bold text-rose-700">Impossible de charger la base du cabinet</h2>
        <p>${esc(e.message || e)}</p>
        <p class="text-slate-500">Aucune donnée fictive n'est affichée. Relancez l'application ; si le problème persiste, vérifiez le dossier %APPDATA%\\LexProcedura.</p>
    </div>`;
}

/** Reprise unique des dossiers saisis dans l'ancienne interface (stockés dans le navigateur). */
async function reprendreDonneesNavigateur() {
    let brutAff = null, brutParams = null;
    try { brutAff = localStorage.getItem('lexprocedura_affaires'); brutParams = localStorage.getItem('lexprocedura_params'); } catch (_) { return; }
    if (!brutAff && !brutParams) return;
    let affaires = [], params = null;
    try { affaires = JSON.parse(brutAff || '[]'); } catch (_) { affaires = []; }
    try { params = JSON.parse(brutParams || 'null'); } catch (_) { params = null; }
    const demo = ['aff-001', 'aff-002', 'aff-003'];
    const reelles = Array.isArray(affaires) ? affaires.filter(a => a && !demo.includes(a.id)) : [];
    const aDesCles = params && (params.gemini_api_key || params.piste_client_id || params.resend_api_key);
    if (reelles.length || aDesCles) {
        const ok = await uiConfirm('Reprise des anciennes données',
            `L'ancienne interface conservait ${reelles.length} dossier(s) créé(s) par vous${aDesCles ? ' et des clés API' : ''} dans le stockage du navigateur, hors de la base du cabinet.\n\nLes reprendre dans la base locale (data.json) ? Les dossiers de démonstration sont ignorés et les échéances reprises seront marquées « à revérifier ».`,
            'Reprendre');
        if (!ok) return;
        appliquer(await appelRust('importer_donnees_navigateur', { affaires: reelles, parametres: params }));
        remplirParametres();
        toast('Anciennes données reprises dans la base du cabinet.', 'ok');
    }
    try { localStorage.removeItem('lexprocedura_affaires'); localStorage.removeItem('lexprocedura_params'); } catch (_) { /* rien */ }
}

function remplirOptions(select, liste, valeur, libelle, premiere) {
    select.innerHTML = (premiere ? `<option value="">${esc(premiere)}</option>` : '') + liste.map(x => `<option value="${esc(valeur(x))}">${esc(libelle(x))}</option>`).join('');
}

function remplirReferentiels() {
    const cals = etat.ref.calendriers || [];
    remplirOptions($('exp-calendrier-select'), cals, c => c.code, c => c.libelle);
    remplirOptions($('param-calendrier-defaut'), cals, c => c.code, c => c.libelle);
    remplirOptions($('modal-case-calendrier'), cals, c => c.code, c => c.libelle, 'Calendrier par défaut (Paramètres)');
    remplirOptions($('sim-regime'), etat.ref.regimes_prescription || [], r => r.code, r => `${r.libelle} (${r.fondement})`);
    remplirOptions($('tree-nat'), etat.ref.matieres_competence || [], m => m.code, m => m.libelle);
    remplirOptions($('select-modele-diligences'), etat.ref.modeles_diligences || [], m => m.code, m => `${m.libelle} (${m.elements.length} points)`, 'Choisir un modèle de diligences...');
}

function remplirSelectRegles(select, inclureFixe) {
    select.innerHTML = '';
    const groupes = {};
    etat.regles.forEach(r => {
        if (!inclureFixe && r.mode === 'FIXE') return;
        (groupes[r.categorie] = groupes[r.categorie] || []).push(r);
    });
    Object.entries(groupes).forEach(([cat, regles]) => {
        const og = document.createElement('optgroup');
        og.label = cat;
        regles.forEach(r => {
            const o = document.createElement('option');
            o.value = r.code;
            o.textContent = r.mode === 'CALCULE' ? `${r.libelle} - ${r.fondement}` : r.libelle;
            og.appendChild(o);
        });
        select.appendChild(og);
    });
}
const regle = (code) => etat.regles.find(r => r.code === code);

function renderAll() {
    renderKPIs();
    remplirDossiersAnalyse();
    renderCasesList();
    renderCaseDetails();
    updateAiContextPill();
    if (etat.vue === 'view-journal') renderJournal();
    if (etat.vue === 'view-agenda') renderAgenda();
    icones();
}

// -----------------------------------------------------------------------------
// 4. NAVIGATION
// -----------------------------------------------------------------------------
function initNavigation() {
    document.querySelectorAll('aside .nav-item').forEach(btn => {
        btn.onclick = () => { changerVue(btn.dataset.view); btn.blur(); };
    });
    $('btn-rail-nouvelle-affaire').onclick = (e) => { e.currentTarget.blur(); ouvrirModalAffaire(null); };
    $('btn-theme').onclick = () => appliquerTheme(!document.documentElement.classList.contains('theme-sombre'));
    majBoutonTheme();
    document.querySelectorAll('.filter-pill').forEach(p => { p.onclick = () => appliquerFiltre(p.dataset.filter); });
    $('filter-search-input').oninput = renderCasesList;
    $('liste-tri').onchange = renderCasesList;
    $('liste-archives').onchange = renderCasesList;
    document.querySelectorAll('#dossier-onglets .onglet').forEach(b => { b.onclick = () => changerOnglet(b.dataset.onglet); });
    $('btn-retour-liste').onclick = () => changerVue('view-dashboard');
    $('btn-retour-liste-vide').onclick = () => changerVue('view-dashboard');
    $('kpi-card-urgent').onclick = () => appliquerFiltre('URGENT');
    $('kpi-card-week').onclick = () => appliquerFiltre('WEEK');
    $('kpi-card-active').onclick = () => appliquerFiltre('ALL');
}

function changerVue(id) {
    etat.vue = id;
    const menu = id === 'view-dossier' ? 'view-dashboard' : id;
    document.querySelectorAll('aside .nav-item').forEach(b => b.classList.toggle('active', b.dataset.view === menu));
    document.querySelectorAll('.content-view').forEach(v => v.classList.toggle('hidden', v.id !== id));
    if (id === 'view-journal') renderJournal();
    if (id === 'view-agenda') renderAgenda();
    if (id === 'view-params') renderInfosStockage();
    updateAiContextPill();
    icones();
}

function majBoutonTheme() {
    const sombre = document.documentElement.classList.contains('theme-sombre');
    $('btn-theme').innerHTML = `<i data-lucide="${sombre ? 'sun' : 'moon'}" class="w-4 h-4 shrink-0"></i><span class="menu-libelle">${sombre ? 'Passer au thème clair' : 'Passer au thème sombre'}</span>`;
    icones();
}

function appliquerTheme(sombre) {
    document.documentElement.classList.toggle('theme-sombre', sombre);
    try { localStorage.setItem('lexprocedura_theme', sombre ? 'sombre' : 'clair'); } catch (_) { /* préférence non conservée */ }
    majBoutonTheme();
}

function appliquerFiltre(nom) {
    etat.filtre = nom;
    document.querySelectorAll('.filter-pill').forEach(p => { p.classList.toggle('active', p.dataset.filter === nom); p.classList.toggle('actif', p.dataset.filter === nom); });
    renderCasesList();
}

// -----------------------------------------------------------------------------
// 5. URGENCE DES ÉCHÉANCES (multi-actes)
// -----------------------------------------------------------------------------
function actesEnAttente(aff) {
    return (aff.actes || []).filter(a => a.statut !== 'ACCOMPLI' && a.date_echeance_calculee);
}

function niveauJours(j) {
    if (j === null) return 'AUCUNE';
    if (j < 0) return 'EXPIRE';
    if (j <= Number(P().seuil_rouge_jours ?? 3)) return 'CRITIQUE';
    if (j <= Number(P().seuil_orange_jours ?? 7)) return 'VIGILANCE';
    return 'NORMAL';
}

/** Situation d'un dossier : prochaine échéance à venir + présence d'échéances dépassées non traitées. */
function situation(aff) {
    const attente = actesEnAttente(aff).map(a => ({ acte: a, j: joursJusqua(a.date_echeance_calculee) })).sort((x, y) => x.j - y.j);
    const expirees = attente.filter(x => x.j < 0);
    const prochaine = attente.find(x => x.j >= 0) || null;
    let niveau = prochaine ? niveauJours(prochaine.j) : 'AUCUNE';
    if (expirees.length) niveau = 'EXPIRE';
    return { prochaine, expirees, niveau, attente };
}

const ORDRE_NIVEAU = { EXPIRE: 0, CRITIQUE: 1, VIGILANCE: 2, NORMAL: 3, AUCUNE: 4 };

function badgeNiveau(niveau, j) {
    switch (niveau) {
        case 'EXPIRE': return `<span class="etiquette-statut statut-danger">Échéance dépassée</span>`;
        case 'CRITIQUE': return `<span class="etiquette-statut statut-danger">${j === 0 ? "Aujourd'hui" : 'J-' + j}</span>`;
        case 'VIGILANCE': return `<span class="etiquette-statut statut-alerte">J-${j}</span>`;
        case 'NORMAL': return `<span class="etiquette-statut statut-info">J-${j}</span>`;
        default: return `<span class="etiquette-statut statut-neutre">Sans échéance</span>`;
    }
}

function renderKPIs() {
    const rouge = Number(P().seuil_rouge_jours ?? 3), orange = Number(P().seuil_orange_jours ?? 7);
    let crit = 0, proches = 0, total = 0, actifs = 0;
    const affaires = etat.donnees.affaires || [];
    affaires.forEach(a => {
        if (a.statut === 'ARCHIVEE') return;
        actifs++;
        total += Number(a.enjeu_financier) || 0;
        actesEnAttente(a).forEach(x => {
            const j = joursJusqua(x.date_echeance_calculee);
            if (j >= 0) { if (j <= rouge) crit++; if (j <= orange) proches++; }
        });
    });
    $('kpi-critique-count').textContent = crit;
    $('kpi-week-count').textContent = proches;
    $('kpi-active-count').textContent = actifs;
    $('kpi-total-financial').textContent = total ? `${euros(total)} d'enjeux` : 'aucun enjeu chiffré';
    $('kpi-lbl-rouge').textContent = `dans les ${rouge} jour${rouge > 1 ? 's' : ''}`;
    $('kpi-lbl-orange').textContent = `dans les ${orange} jour${orange > 1 ? 's' : ''}`;
    $('pill-cnt-all').textContent = affaires.length;
    $('pill-cnt-urg').textContent = affaires.filter(a => situation(a).niveau === 'CRITIQUE').length;
    $('pill-cnt-week').textContent = affaires.filter(a => ['CRITIQUE', 'VIGILANCE'].includes(situation(a).niveau)).length;
    $('pill-cnt-exp').textContent = affaires.filter(a => situation(a).niveau === 'EXPIRE').length;
    const urgents = affaires.filter(a => a.statut !== 'ARCHIVEE' && ['EXPIRE', 'CRITIQUE'].includes(situation(a).niveau)).length;
    $('badge-critiques').textContent = urgents;
    $('badge-critiques').classList.toggle('hidden', !urgents);
    $('badge-critiques-libelle').textContent = urgents ? `${urgents} ${t('urgent(s)')}` : '';
    $('badge-critiques-libelle').classList.toggle('hidden', !urgents);
}

// -----------------------------------------------------------------------------
// 6. COCKPIT : LISTE DES AFFAIRES (pleine largeur, triable)
// -----------------------------------------------------------------------------
function renderCasesList() {
    const c = $('cases-cards-container');
    const affaires = etat.donnees.affaires || [];
    $('dossiers-outils').classList.toggle('hidden', !affaires.length);
    $('liste-entete').classList.add('hidden');
    if (!affaires.length) {
        c.innerHTML = `<div class="etat-vide" style="min-height:22rem">
            <i data-lucide="briefcase" class="w-9 h-9"></i>
            <p class="text-base font-semibold texte-1">Bienvenue dans Deadline Breaker</p>
            <p class="max-w-sm">Créez votre première affaire pour suivre ses échéances, ses diligences et sa chronologie.</p>
            <div class="flex gap-2 mt-2">
                <button id="btn-onboard-new" type="button" class="btn btn-primaire">Créer ma première affaire</button>
                <button id="btn-onboard-restore" type="button" class="btn btn-secondaire">Restaurer une sauvegarde</button>
            </div>
            <p class="text-xs mt-2">Données stockées uniquement sur ce poste.</p>
        </div>`;
        $('btn-onboard-new').onclick = () => ouvrirModalAffaire(null);
        $('btn-onboard-restore').onclick = () => changerVue('view-params');
        icones();
        return;
    }

    const q = $('filter-search-input').value.toLowerCase().trim();
    const archives = $('liste-archives').checked;
    const tri = $('liste-tri').value;
    const liste = affaires
        .map(a => ({ a, s: situation(a) }))
        .filter(({ a, s }) => {
            if (a.statut === 'ARCHIVEE' && !archives) return false;
            const texte = [a.nom_dossier, a.numero_rg, a.juridiction, a.reference_cabinet, a.client, a.partie_adverse].join(' ').toLowerCase();
            if (q && !texte.includes(q)) return false;
            if (etat.filtre === 'URGENT') return s.niveau === 'CRITIQUE';
            if (etat.filtre === 'WEEK') return s.niveau === 'CRITIQUE' || s.niveau === 'VIGILANCE';
            if (etat.filtre === 'EXPIRED') return s.niveau === 'EXPIRE';
            return true;
        })
        .sort((x, y) => {
            if (tri === 'NOM') return x.a.nom_dossier.localeCompare(y.a.nom_dossier, 'fr');
            if (tri === 'RECENT') return String(y.a.date_modification || y.a.date_creation || '').localeCompare(String(x.a.date_modification || x.a.date_creation || ''));
            return (x.a.statut === 'ARCHIVEE') - (y.a.statut === 'ARCHIVEE')
                || ORDRE_NIVEAU[x.s.niveau] - ORDRE_NIVEAU[y.s.niveau]
                || (x.s.prochaine ? x.s.prochaine.j : 1e9) - (y.s.prochaine ? y.s.prochaine.j : 1e9);
        });

    c.innerHTML = '';
    if (!liste.length) {
        c.innerHTML = `<p class="etat-vide" style="min-height:10rem">Aucun dossier ne correspond${!archives && affaires.some(a => a.statut === 'ARCHIVEE') ? ' (les dossiers archivés sont masqués)' : ''}.</p>`;
        return;
    }
    $('liste-entete').classList.remove('hidden');
    liste.forEach(({ a, s }) => {
        const cible = s.expirees[0] || s.prochaine;
        const dils = a.diligences || [];
        const dDone = dils.filter(d => d.fait).length;
        const pctDil = dils.length ? Math.round(dDone / dils.length * 100) : 0;
        const etape = Math.min(5, Math.max(1, a.etape_procedure || 1));
        const meta = [a.juridiction, a.numero_rg, a.enjeu_financier ? euros(a.enjeu_financier) : ''].filter(Boolean).map(esc).join(' · ');
        const ligne = document.createElement('button');
        ligne.type = 'button';
        ligne.dataset.dossier = a.id;
        const teinte = { EXPIRE: ' niveau-expire', CRITIQUE: ' niveau-critique', VIGILANCE: ' niveau-vigilance' }[s.niveau] || '';
        ligne.className = `ligne-dossier liste-dossiers${teinte}${a.statut === 'ARCHIVEE' ? ' archive' : ''}`;
        ligne.innerHTML = `
            <span class="min-w-0">
                <span class="ligne-titre">${esc(a.nom_dossier)}${a.statut === 'ARCHIVEE' ? ' <span class="texte-3 font-normal text-xs">archivé</span>' : ''}</span>
                <span class="ligne-sous">${meta || 'Informations à compléter'}</span>
            </span>
            <span class="text-sm texte-2">${etape}/5 · ${esc(ETAPES[etape - 1])}</span>
            <span class="min-w-0 flex items-center gap-3">
                <span class="shrink-0">${badgeNiveau(s.niveau, s.prochaine ? s.prochaine.j : null)}</span>
                <span class="min-w-0">
                    <span class="block text-sm texte-1 truncate">${cible ? esc(cible.acte.nature) : 'Aucune échéance en cours'}</span>
                    <span class="block text-xs texte-3">${cible ? fmt(cible.acte.date_echeance_calculee) : ''}${s.expirees.length > 1 ? ` · ${s.expirees.length} dépassées` : ''}</span>
                </span>
            </span>
            <span>${cible ? (jaugeActe(cible.acte, true) || '<span class="text-xs texte-3">Date fixée</span>') : ''}</span>
            <span class="text-right">
                <span class="text-sm ${pctDil === 100 && dils.length ? 'text-emerald-700' : 'texte-2'}">${dDone}/${dils.length}</span>
                <span class="jauge block mt-1.5"><span class="block h-full bg-emerald-500" style="width:${pctDil}%"></span></span>
            </span>`;
        ligne.onclick = () => ouvrirDossier(a.id);
        c.appendChild(ligne);
    });
    icones();
}

// -----------------------------------------------------------------------------
// 7. COCKPIT : FICHE DU DOSSIER
// -----------------------------------------------------------------------------
const affaireActive = () => (etat.donnees.affaires || []).find(a => a.id === etat.selectedId) || null;
const LIB_RESIDENCE = { FRANCE_METROPOLE: 'Métropole', DOM_TOM: 'Outre-mer (autre collectivité)', DOM_TOM_LOCAL: 'Outre-mer (même collectivité)', ETRANGER: 'Étranger' };
const ETAPES = ['Pré-contentieux', 'Introduction', 'Mise en état', 'Audience / délibéré', 'Recours / exécution'];
const libelleCalendrier = (code) => ((etat.ref.calendriers || []).find(c => c.code === code) || {}).libelle || code;
const calendrierAffaire = (aff) => (aff && aff.calendrier_juridiction) || P().calendrier_defaut || 'METROPOLE';

/** Jauge de consommation du délai : part écoulée entre l'acte et l'échéance.
 *  Sans objet pour une échéance fixée par la juridiction (aucun délai ne « court »). */
function jaugeActe(acte, compacte = false) {
    if (!acte || acte.saisie_manuelle) return '';
    const total = joursEntre(acte.date_evenement, acte.date_echeance_calculee);
    const restant = joursJusqua(acte.date_echeance_calculee);
    if (total === null || restant === null || total <= 0) return '';
    const pct = Math.max(0, Math.min(100, Math.round((total - restant) / total * 100)));
    const niveau = niveauJours(restant);
    const couleur = { EXPIRE: 'bg-slate-500', CRITIQUE: 'bg-rose-500', VIGILANCE: 'bg-amber-500', NORMAL: 'bg-sky-500' }[niveau] || 'bg-slate-300';
    const barre = `<div class="jauge" title="${pct} % du délai écoulé"><div class="${couleur}" style="width:${pct}%"></div></div>`;
    if (compacte) return barre;
    return `<div class="flex items-center justify-between text-xs texte-2 mb-1.5"><span>${pct} % du délai écoulé (${total} jours au total)</span><span>${restant < 0 ? `dépassé de ${-restant} j` : `reste ${restant} j`}</span></div>${barre}`;
}

function ouvrirDossier(id, onglet) {
    etat.selectedId = id;
    if (onglet) etat.onglet = onglet;
    changerVue('view-dossier');
    renderAll();
}

function changerOnglet(nom) {
    etat.onglet = nom;
    document.querySelectorAll('#dossier-onglets .onglet').forEach(b => b.classList.toggle('active', b.dataset.onglet === nom));
    document.querySelectorAll('[data-panneau]').forEach(p => p.classList.toggle('hidden', p.dataset.panneau !== nom));
}


function renderCaseDetails() {
    const aff = affaireActive();
    $('cp-empty-state').classList.toggle('hidden', !!aff);
    $('cp-empty-state').classList.toggle('flex', !aff);
    $('cp-detail-body').classList.toggle('hidden', !aff);
    $('cp-detail-body').classList.toggle('flex', !!aff);
    if (!aff) return;

    const s = situation(aff);
    $('cp-dossier-titre').textContent = aff.nom_dossier;
    $('cp-badge-urgence').innerHTML = badgeNiveau(s.niveau, s.prochaine ? s.prochaine.j : null);
    const chips = [aff.juridiction, aff.numero_rg ? 'RG ' + aff.numero_rg.replace(/^RG\s*/i, '') : '', aff.reference_cabinet ? 'Réf. ' + aff.reference_cabinet : '', aff.enjeu_financier ? euros(aff.enjeu_financier) : '', aff.statut === 'ARCHIVEE' ? 'Archivé' : ''].filter(Boolean);
    $('cp-dossier-meta').innerHTML = chips.map(c => `<span>${esc(c)}</span>`).join('');

    // Onglets
    const actes = aff.actes || [];
    const dils = aff.diligences || [];
    const journal = (etat.donnees.historique || []).filter(e => e.id_affaire === aff.id);
    $('onglet-cnt-echeances').textContent = actes.length;
    $('onglet-cnt-diligences').textContent = `${dils.filter(d => d.fait).length}/${dils.length}`;
    $('onglet-cnt-journal').textContent = journal.length;
    changerOnglet(etat.onglet || 'synthese');

    // Étape de la procédure
    const etape = Math.min(5, Math.max(1, aff.etape_procedure || 1));
    $('cp-etape-label').textContent = `Étape ${etape} sur 5`;
    $('cp-etapes').innerHTML = ETAPES.map((lib, i) => {
        const n = i + 1;
        const cls = n < etape ? 'faite' : n === etape ? 'courante' : '';
        return `<button type="button" data-etape="${n}" class="etape ${cls}" title="Passer le dossier à cette étape"><span class="etape-num">${n < etape ? '✓' : n}</span>${esc(lib)}</button>`;
    }).join('');
    $('cp-etapes').querySelectorAll('[data-etape]').forEach(b => {
        b.onclick = () => {
            const n = Number(b.dataset.etape);
            if (n === etape) return;
            appelRust('modifier_affaire', { idAffaire: aff.id, affaire: saisieAffaireDepuis(aff, { etape_procedure: n }) }).then(appliquer).catch(erreur);
        };
    });

    // Prochaine échéance
    const cible = s.expirees[0] || s.prochaine;
    const badgeBox = $('cp-article-trigger-badge');
    if (cible) {
        const a = cible.acte;
        const etiquette = cible.j < 0 ? `<span class="etiquette-statut statut-danger">Dépassée depuis ${-cible.j} jour${cible.j < -1 ? 's' : ''}</span>` : `<span class="etiquette-statut ${classeStatut(cible.j)}">${esc(libelleJours(cible.j))}</span>`;
        $('cp-prochaine').innerHTML = `${etiquette}
            <p class="date-majeure" style="font-size:1.6rem">${esc(dateEnLettres(a.date_echeance_calculee))}</p>
            <p class="text-sm texte-2 mt-1">${esc(a.nature)}</p>
            <dl class="mt-5">
                <div class="ligne-info"><dt>Sanction</dt><dd>${esc(a.sanction || 'À préciser')}</dd></div>
                ${a.fondement_cpc ? `<div class="ligne-info"><dt>Fondement</dt><dd>${lierArticles(esc(a.fondement_cpc), 'CPC')}</dd></div>` : ''}
            </dl>
            ${s.expirees.length > 1 ? `<p class="note-alerte mt-4" style="list-style:none;padding-left:1rem">${s.expirees.length} échéances dépassées non traitées : voir l'onglet Échéances.</p>` : ''}`;
        $('cp-vigilance-texte').innerHTML = lierArticles(esc(a.details_calcul || ''), 'CPC');
        $('cp-detail-calcul').classList.toggle('hidden', !a.details_calcul);
        $('cp-jauge').innerHTML = jaugeActe(a) || '<p class="text-xs texte-3">Échéance fixée par la juridiction : pas de délai à consommer.</p>';
        badgeBox.innerHTML = a.article_num ? `<button type="button" class="btn-legal-article btn btn-fantome btn-mini" data-code="${esc(a.article_code || 'CPC')}" data-article="${esc(a.article_num)}"><i data-lucide="book-open" class="w-3.5 h-3.5"></i> Art. ${esc(a.article_num)}</button>` : '';
    } else {
        $('cp-prochaine').innerHTML = `<div class="etat-vide" style="min-height:9rem;padding:1rem"><i data-lucide="calendar-plus" class="w-7 h-7"></i><p>Aucune échéance en cours. Ajoutez un acte pour calculer le délai.</p></div>`;
        $('cp-vigilance-texte').textContent = '';
        $('cp-detail-calcul').classList.add('hidden');
        $('cp-jauge').innerHTML = '';
        badgeBox.innerHTML = '';
    }
    $('cp-prochaine').querySelectorAll('.btn-legal-article').forEach(b => b.classList.add('lien-article'));

    // Informations
    const infos = [
        ['Client', aff.client], ['Partie adverse', aff.partie_adverse], ['Juridiction', aff.juridiction], ['N° RG', aff.numero_rg],
        ['Référence', aff.reference_cabinet], ['Enjeu', aff.enjeu_financier ? euros(aff.enjeu_financier) : ''],
        ['Domicile de la partie', LIB_RESIDENCE[aff.lieu_residence_partie] || aff.lieu_residence_partie],
        ['Jours fériés', libelleCalendrier(calendrierAffaire(aff)) + (aff.calendrier_juridiction ? '' : ' (par défaut)')],
        ['Masqué pour l\'IA', (aff.termes_confidentiels || []).join(', ')],
        ['Statut', aff.statut === 'ARCHIVEE' ? 'Archivé' : 'Actif'],
    ];
    $('dossier-infos').innerHTML = infos.map(([k, v]) => `<div><dt>${esc(k)}</dt><dd class="${v ? '' : 'vide'}">${esc(v || 'Non renseigné')}</dd></div>`).join('');
    $('btn-infos-modifier').onclick = () => ouvrirModalAffaire(aff);

    // Notes (non écrasées pendant la saisie)
    if (document.activeElement !== $('dossier-notes') || etat.notesDossier !== aff.id) {
        $('dossier-notes').value = aff.notes || '';
        etat.notesDossier = aff.id;
    }

    // Diligences
    const pct = dils.length ? Math.round(dils.filter(d => d.fait).length / dils.length * 100) : 0;
    const cc = $('cp-checklist-items');
    cc.innerHTML = '';
    if (!dils.length) cc.innerHTML = `<p class="text-sm texte-3 py-2">Aucune diligence. Ajoutez vos points de contrôle ou appliquez un modèle.</p>`;
    dils.forEach(dil => {
        const item = document.createElement('label');
        item.className = `diligence${dil.fait ? ' faite' : ''}`;
        item.innerHTML = `
            <input type="checkbox" ${dil.fait ? 'checked' : ''}>
            <span class="diligence-libelle">${esc(dil.libelle)}</span>
            ${dil.fait && dil.date_fait ? `<span class="text-xs texte-3 shrink-0">${esc(String(dil.date_fait).slice(0, 10).split('-').reverse().join('/'))}</span>` : ''}
            <button type="button" title="Supprimer" class="suppr btn-icone btn-icone-danger" style="width:1.75rem;height:1.75rem"><i data-lucide="x" class="w-3.5 h-3.5"></i></button>`;
        item.querySelector('input').onchange = (e) => appelRust('basculer_diligence', { idAffaire: aff.id, idDiligence: dil.id, fait: e.target.checked }).then(appliquer).catch(erreur);
        item.querySelector('button').onclick = (e) => { e.preventDefault(); appelRust('supprimer_diligence', { idAffaire: aff.id, idDiligence: dil.id }).then(appliquer).catch(erreur); };
        cc.appendChild(item);
    });
    $('cp-checklist-percent').textContent = pct + '%';
    $('cp-checklist-progressbar').style.width = pct + '%';

    // Actes et échéances
    const tc = $('cp-timeline-items');
    tc.innerHTML = '';
    const tries = [...actes].sort((x, y) => String(x.date_echeance_calculee).localeCompare(String(y.date_echeance_calculee)));
    if (!tries.length) tc.innerHTML = `<div class="etat-vide" style="min-height:10rem"><i data-lucide="calendar-plus" class="w-7 h-7"></i><p>Aucun acte enregistré. Utilisez « Ajouter une échéance ».</p></div>`;
    tries.forEach(ac => {
        const j = joursJusqua(ac.date_echeance_calculee);
        const fait = ac.statut === 'ACCOMPLI';
        const etiquette = fait ? '<span class="etiquette-statut statut-ok">Accompli</span>'
            : j < 0 ? `<span class="etiquette-statut statut-danger">Dépassée de ${-j} j</span>`
            : `<span class="etiquette-statut ${classeStatut(j)}">${esc(libelleJours(j))}</span>`;
        const step = document.createElement('div');
        step.className = `acte${fait ? ' fait' : ''}`;
        step.innerHTML = `
            <div class="acte-date">${fmt(ac.date_echeance_calculee)}<small>${ac.saisie_manuelle ? 'date fixée' : 'depuis le ' + fmt(ac.date_evenement)}</small></div>
            <div class="min-w-0">
                <p class="acte-titre">${esc(ac.nature)}</p>
                <div class="acte-meta">${etiquette}
                    ${ac.article_num ? boutonArticle(ac.article_code || 'CPC', ac.article_num, 'art. ' + ac.article_num, 'lien-article') : ''}
                    ${ac.alerte_envoyee ? `<span class="text-indigo-600">${(ac.alertes_auto_envoyees || []).length ? 'alerte ' + ac.alertes_auto_envoyees.map(j => 'J-' + j).join(' et ') + ' envoyée' : 'alerte envoyée'}</span>` : ''}</div>
                ${fait ? '' : `<div class="mt-2.5" style="max-width:20rem">${jaugeActe(ac, true)}</div>`}
                <details class="options-avancees mt-2"><summary>Détail</summary>
                    <p class="etapes-calcul" style="list-style:none;padding-left:0">${lierArticles(esc(ac.details_calcul), 'CPC')}</p>
                    <p class="text-sm texte-2 mt-1"><span class="texte-3">Sanction :</span> ${esc(ac.sanction)}</p></details>
            </div>
            <div class="acte-actions">
                <button type="button" data-act="statut" class="btn btn-secondaire btn-mini">${fait ? 'Rouvrir' : '<i data-lucide="check" class="w-3.5 h-3.5"></i> Accompli'}</button>
                <button type="button" data-act="ics" class="btn-icone" style="width:1.9rem;height:1.9rem" title="Ajouter à l'agenda (.ics)"><i data-lucide="calendar-plus" class="w-4 h-4"></i></button>
                <button type="button" data-act="mail" class="btn-icone" style="width:1.9rem;height:1.9rem" title="Envoyer une alerte e-mail" ${fait ? 'disabled' : ''}><i data-lucide="mail" class="w-4 h-4"></i></button>
                <button type="button" data-act="suppr" class="btn-icone btn-icone-danger" style="width:1.9rem;height:1.9rem" title="Supprimer"><i data-lucide="trash-2" class="w-4 h-4"></i></button>
            </div>`;
        step.querySelector('[data-act="statut"]').onclick = () => appelRust('definir_statut_acte', { idAffaire: aff.id, idActe: ac.id, accompli: !fait }).then(appliquer).catch(erreur);
        step.querySelector('[data-act="ics"]').onclick = () => exporterIcs(`Echeance_${aff.nom_dossier}_${ac.nature}`, [evenementIcs(aff, ac)]).catch(erreur);
        step.querySelector('[data-act="mail"]').onclick = (e) => avecBouton(e.currentTarget, async () => {
            const r = await appelRust('envoyer_alerte_acte', { idAffaire: aff.id, idActe: ac.id });
            appliquer(r); toast(r.message || 'Alerte envoyée.', 'ok');
        });
        step.querySelector('[data-act="suppr"]').onclick = async () => {
            if (!await uiConfirm("Supprimer l'échéance", `Supprimer « ${ac.nature} » (échéance du ${fmt(ac.date_echeance_calculee)}) ?\nL'opération est tracée dans le journal d'activité.`, 'Supprimer')) return;
            appelRust('supprimer_acte', { idAffaire: aff.id, idActe: ac.id }).then(appliquer).catch(erreur);
        };
        tc.appendChild(step);
    });

    // Journal du dossier
    $('dossier-journal').innerHTML = journal.length ? journal.slice(0, 300).map(e => ligneJournal(e, null)).join('')
        : '<p class="etat-vide" style="min-height:8rem">Aucune entrée pour ce dossier.</p>';
}

/** Ligne du journal d'activité (journal général et journal du dossier). */
function ligneJournal(e, nomDossier) {
    const [jour, heure] = String(e.horodatage || '').split(/\s+/);
    return `<div class="ligne-journal">
        <div class="journal-quand">${esc(jour || '')}<br>${esc(heure || '')}</div>
        <div class="min-w-0">
            <p class="journal-titre"><span>${esc(e.titre)}</span><span class="puce-categorie">${esc(LIB_CATEGORIES[e.categorie] || e.categorie)}</span>${nomDossier ? `<span class="text-xs texte-3 font-normal">${esc(nomDossier)}</span>` : ''}</p>
            ${e.details ? `<p class="journal-details select-text">${esc(e.details)}</p>` : ''}
            ${e.fondement ? `<p class="journal-fondement">${esc(e.fondement)}</p>` : ''}
        </div>
    </div>`;
}
const LIB_CATEGORIES = { DOSSIER: 'Dossier', DELAI_CPC: 'Échéance', CALCUL: 'Calcul', DILIGENCE: 'Diligence', ALERTE: 'Alerte', IA: 'Copilote IA', LEGIFRANCE: 'Légifrance', SYSTEME: 'Système' };

/** Fiche de synthèse du dossier (Word). */
function exporterFicheDossier(aff) {
    const s = situation(aff);
    const lignes = (aff.actes || []).slice().sort((x, y) => String(x.date_echeance_calculee).localeCompare(String(y.date_echeance_calculee)))
        .map(a => `<tr><td>${esc(a.nature)}</td><td>${esc(fmt(a.date_echeance_calculee))}</td><td>${esc(a.fondement_cpc)}</td><td>${esc(a.sanction)}</td><td>${a.statut === 'ACCOMPLI' ? 'Accompli' : 'En cours'}</td></tr>`).join('');
    const dils = (aff.diligences || []).map(d => `<li>${d.fait ? '☑' : '☐'} ${esc(d.libelle)}</li>`).join('');
    const corps = `
        <p><b>Client :</b> ${esc(aff.client || '-')} &nbsp; <b>Partie adverse :</b> ${esc(aff.partie_adverse || '-')}</p>
        <p><b>Juridiction :</b> ${esc(aff.juridiction || '-')} &nbsp; <b>N° RG :</b> ${esc(aff.numero_rg || '-')} &nbsp; <b>Réf. :</b> ${esc(aff.reference_cabinet || '-')}</p>
        <p><b>Étape :</b> ${esc(ETAPES[(aff.etape_procedure || 1) - 1])} &nbsp; <b>Enjeu :</b> ${aff.enjeu_financier ? euros(aff.enjeu_financier) : '-'}</p>
        <p><b>Prochaine échéance :</b> ${s.prochaine ? esc(s.prochaine.acte.nature + ' - ' + fmtLong(s.prochaine.acte.date_echeance_calculee)) : 'aucune'}${s.expirees.length ? ` &nbsp; <b style="color:#be123c">${s.expirees.length} échéance(s) dépassée(s)</b>` : ''}</p>
        <h3>Échéances</h3><table border="1" cellpadding="5" style="border-collapse:collapse;font-size:10pt"><tr><th>Acte</th><th>Échéance</th><th>Fondement</th><th>Sanction</th><th>Statut</th></tr>${lignes || '<tr><td colspan="5">Aucune</td></tr>'}</table>
        <h3>Diligences</h3><ul>${dils || '<li>Aucune</li>'}</ul>
        ${aff.notes ? `<h3>Notes</h3><p>${enParagraphes(aff.notes)}</p>` : ''}`;
    return exporterWord(`Fiche_${aff.nom_dossier}`, `Fiche dossier - ${aff.nom_dossier}`, corps);
}

const evenementIcs = (aff, ac) => ({
    titre: `${aff.nom_dossier} - ${ac.nature}`,
    date: ac.date_echeance_calculee,
    description: `${ac.fondement_cpc}\nSanction : ${ac.sanction}\n${ac.details_calcul}`,
    lieu: aff.juridiction,
});

function initDiligences() {
    $('form-add-diligence').onsubmit = (e) => {
        e.preventDefault();
        const aff = affaireActive();
        const libelle = $('input-new-diligence').value.trim();
        if (!aff || !libelle) return;
        appelRust('ajouter_diligence', { idAffaire: aff.id, libelle }).then(r => { $('input-new-diligence').value = ''; appliquer(r); }).catch(erreur);
    };
    $('btn-appliquer-modele').onclick = (e) => avecBouton(e.currentTarget, async () => {
        const aff = affaireActive();
        const code = $('select-modele-diligences').value;
        if (!aff || !code) return;
        const r = await appelRust('appliquer_modele_diligences', { idAffaire: aff.id, codeModele: code });
        appliquer(r); toast(r.message || 'Modèle appliqué.', 'ok');
    });
    $('btn-copilote-dossier').onclick = () => { ouvrirDock(); $('chat-user-input').focus(); };
    $('btn-export-fiche').onclick = () => { const a = affaireActive(); if (a) exporterFicheDossier(a).catch(erreur); };
    $('btn-ics-dossier').onclick = () => {
        const a = affaireActive();
        const actes = a ? (a.actes || []).filter(x => x.statut !== 'ACCOMPLI' && joursJusqua(x.date_echeance_calculee) >= 0) : [];
        if (!actes.length) return toast('Aucune échéance à venir à exporter.', 'alerte');
        exporterIcs(`Echeances_${a.nom_dossier}`, actes.map(x => evenementIcs(a, x))).catch(erreur);
    };
    $('btn-save-notes').onclick = (e) => avecBouton(e.currentTarget, async () => {
        const a = affaireActive();
        if (!a) return;
        const r = await appelRust('modifier_affaire', { idAffaire: a.id, affaire: saisieAffaireDepuis(a, { notes: $('dossier-notes').value }) });
        etat.notesDossier = null;
        appliquer(r); toast('Notes enregistrées.', 'ok');
    });
    $('btn-recalc-dossier').onclick = (e) => avecBouton(e.currentTarget, () => { const a = affaireActive(); return a ? proposerRecalcul(a.id, true) : null; });
}

/** Simule le recalcul des échéances en cours, puis propose de l'appliquer. */
async function proposerRecalcul(idAffaire, manuel = false) {
    const simu = await appelRust('recalculer_echeances', { idAffaire: idAffaire || null, appliquer: false });
    if (!simu.changements.length) {
        if (manuel) toast(`Échéances à jour : aucune modification nécessaire${simu.ignores ? ` (${simu.ignores} échéance(s) non recalculable(s))` : ''}.`, 'ok');
        return;
    }
    const lignes = simu.changements.slice(0, 12).map(c => `• ${c.nom_dossier} - ${c.nature} : ${fmt(c.ancienne)} → ${fmt(c.nouvelle)}`).join('\n');
    const plus = simu.changements.length > 12 ? `\n… et ${simu.changements.length - 12} autre(s).` : '';
    const ok = await uiConfirm('Recalcul des échéances',
        `Avec le calendrier et la résidence actuels, ${simu.changements.length} échéance(s) en cours changeraient :\n\n${lignes}${plus}\n\nAppliquer ces nouvelles dates ? L'opération est tracée dans le journal d'audit.`, 'Appliquer');
    if (!ok) return;
    const r = await appelRust('recalculer_echeances', { idAffaire: idAffaire || null, appliquer: true });
    appliquer({ donnees: r.donnees });
    toast(`${r.changements.length} échéance(s) recalculée(s).`, 'ok');
}

// -----------------------------------------------------------------------------
// 8. MODALES : AFFAIRE (création / édition) ET ACTE
// -----------------------------------------------------------------------------
let affaireEnEdition = null;

function ouvrirModalAffaire(aff) {
    affaireEnEdition = aff ? aff.id : null;
    $('modal-case-title').textContent = aff ? "Modifier l'affaire" : 'Nouvelle affaire';
    $('btn-submit-modal-new-case').textContent = aff ? 'Enregistrer' : 'Créer le dossier';
    $('modal-case-nom').value = aff ? aff.nom_dossier : '';
    $('modal-case-ref').value = aff ? aff.reference_cabinet : '';
    $('modal-case-rg').value = aff ? aff.numero_rg : '';
    $('modal-case-juridiction').value = aff ? aff.juridiction : '';
    $('modal-case-enjeu').value = aff && aff.enjeu_financier ? aff.enjeu_financier : '';
    $('modal-case-client').value = aff ? aff.client : '';
    $('modal-case-adverse').value = aff ? aff.partie_adverse : '';
    $('modal-case-residence').value = aff ? aff.lieu_residence_partie : 'FRANCE_METROPOLE';
    $('modal-case-calendrier').value = aff ? (aff.calendrier_juridiction || '') : '';
    $('modal-case-statut').value = aff ? aff.statut : 'ACTIVE';
    $('modal-case-notes').value = aff ? aff.notes : '';
    $('modal-case-termes').value = aff ? (aff.termes_confidentiels || []).join(', ') : '';
    ouvrirModal('modal-new-case');
    $('modal-case-nom').focus();
}

function saisieAffaireDepuis(aff, surcharges = {}) {
    return {
        reference_cabinet: aff.reference_cabinet, nom_dossier: aff.nom_dossier, client: aff.client, partie_adverse: aff.partie_adverse,
        juridiction: aff.juridiction, numero_rg: aff.numero_rg, lieu_residence_partie: aff.lieu_residence_partie,
        calendrier_juridiction: aff.calendrier_juridiction || '',
        termes_confidentiels: aff.termes_confidentiels || [],
        enjeu_financier: Number(aff.enjeu_financier) || 0, etape_procedure: aff.etape_procedure || 1, statut: aff.statut, notes: aff.notes,
        ...surcharges,
    };
}

function ouvrirModalActe(prerempli = {}) {
    const aff = affaireActive();
    if (!aff) return toast("Sélectionnez d'abord un dossier.", 'alerte');
    $('modal-act-nature').value = prerempli.nature || '';
    $('modal-act-date').value = prerempli.date || aujourdHui();
    $('modal-act-date-manuelle').value = '';
    ['annees', 'mois', 'jours'].forEach(k => { $('modal-act-libre-' + k).value = 0; });
    $('modal-act-libre-fondement').value = '';
    $('modal-act-libre-sanction').value = '';
    $('modal-act-libre-distance').checked = !!prerempli.distance;
    if (prerempli.duree) ['annees', 'mois', 'jours'].forEach(k => { $('modal-act-libre-' + k).value = prerempli.duree[k] || 0; });
    if (prerempli.type) $('modal-act-delai').value = prerempli.type;
    majModalActe();
    ouvrirModal('modal-add-act');
    $('modal-act-nature').focus();
}

const entier = (id) => Math.max(0, parseInt($(id).value, 10) || 0);
const dureeLibre = (prefixe) => ({ annees: entier(prefixe + 'annees'), mois: entier(prefixe + 'mois'), jours: entier(prefixe + 'jours') });

let jetonApercu = 0;
async function majModalActe() {
    const code = $('modal-act-delai').value;
    const r = regle(code);
    const mode = r ? r.mode : 'CALCULE';
    $('modal-act-manual-wrap').classList.toggle('hidden', mode !== 'FIXE');
    $('modal-act-libre-wrap').classList.toggle('hidden', mode !== 'LIBRE');
    $('modal-act-date-label').textContent = mode === 'FIXE' ? "Date de l'acte (facultative) :" : "Date de l'acte / notification (origine du délai) :";
    const box = $('modal-act-preview');
    const aff = affaireActive();
    if (!r || !aff) { box.classList.add('hidden'); return; }
    if (mode === 'FIXE') {
        box.classList.remove('hidden');
        box.innerHTML = `Échéance fixée par la juridiction : saisissez la date. Aucun calcul n'est appliqué. ${esc(r.note)}`;
        return;
    }
    const date = $('modal-act-date').value;
    if (!date) { box.classList.add('hidden'); return; }
    const args = { dateDepart: date, typeDelai: code, residence: aff.lieu_residence_partie, calendrier: calendrierAffaire(aff), journaliserCalcul: false };
    if (mode === 'LIBRE') { args.dureeLibre = dureeLibre('modal-act-libre-'); args.appliquerDistance = $('modal-act-libre-distance').checked; }
    const jeton = ++jetonApercu;
    try {
        const calc = await appelRust('calculer_delai', args);
        if (jeton !== jetonApercu) return;
        const res = calc.resultat;
        box.classList.remove('hidden');
        box.innerHTML = `<strong>Échéance calculée : ${esc(fmtLong(res.date_echeance_legale))} à 24 h</strong>${res.prorogation_appliquee ? ' (prorogée, art. 642)' : ''}${res.mois_distance ? ` - distance +${res.mois_distance} mois (art. ${esc(res.article_distance)})` : ''}<br><span class="text-sky-800">Calendrier : ${esc(libelleCalendrier(res.calendrier))}${mode === 'CALCULE' ? ' - Sanction : ' + esc(r.sanction) : ''}</span>${r.note ? `<br><span class="text-amber-800">${esc(r.note)}</span>` : ''}`;
    } catch (e) {
        if (jeton !== jetonApercu) return;
        box.classList.remove('hidden');
        box.textContent = e.message;
    }
}

function initModals() {
    $('btn-open-modal-new-case').onclick = () => ouvrirModalAffaire(null);
    $('btn-edit-active-case').onclick = () => { const a = affaireActive(); if (a) ouvrirModalAffaire(a); };
    ['btn-close-modal-new-case', 'btn-cancel-modal-new-case'].forEach(id => { $(id).onclick = () => fermerModal('modal-new-case'); });
    $('btn-submit-modal-new-case').onclick = (e) => avecBouton(e.currentTarget, async () => {
        const saisie = {
            nom_dossier: $('modal-case-nom').value.trim(),
            reference_cabinet: $('modal-case-ref').value.trim(),
            numero_rg: $('modal-case-rg').value.trim(),
            juridiction: $('modal-case-juridiction').value.trim(),
            enjeu_financier: parseFloat(String($('modal-case-enjeu').value).replace(',', '.')) || 0,
            client: $('modal-case-client').value.trim(),
            partie_adverse: $('modal-case-adverse').value.trim(),
            lieu_residence_partie: $('modal-case-residence').value,
            calendrier_juridiction: $('modal-case-calendrier').value,
            statut: $('modal-case-statut').value,
            notes: $('modal-case-notes').value.trim(),
            termes_confidentiels: $('modal-case-termes').value.split(/[,;]/).map(x => x.trim()).filter(Boolean),
            etape_procedure: 1,
        };
        if (!saisie.nom_dossier) { toast('Le nom du dossier (parties) est obligatoire.', 'alerte'); $('modal-case-nom').focus(); return; }
        const avant = affaireEnEdition ? (etat.donnees.affaires || []).find(a => a.id === affaireEnEdition) : null;
        if (avant) saisie.etape_procedure = avant.etape_procedure || 1;
        const r = avant
            ? await appelRust('modifier_affaire', { idAffaire: avant.id, affaire: saisie })
            : await appelRust('creer_affaire', { affaire: saisie });
        if (r.id) etat.selectedId = r.id;
        fermerModal('modal-new-case');
        appliquer(r);
        if (!avant && r.id) ouvrirDossier(r.id, 'synthese');
        toast(r.message || 'Enregistré.', 'ok');
        if (avant && (avant.lieu_residence_partie !== saisie.lieu_residence_partie || (avant.calendrier_juridiction || '') !== saisie.calendrier_juridiction)) {
            await proposerRecalcul(avant.id);
        }
    });

    $('btn-delete-active-case').onclick = async () => {
        const aff = affaireActive();
        if (!aff) return;
        const ok = await uiConfirm('Supprimer le dossier', `Supprimer définitivement « ${aff.nom_dossier} » et ses ${(aff.actes || []).length} échéance(s) ?\n\nUne sauvegarde horodatée de la base est créée automatiquement avant la suppression.`, 'Supprimer');
        if (!ok) return;
        try { const r = await appelRust('supprimer_affaire', { idAffaire: aff.id }); etat.selectedId = null; appliquer(r); changerVue('view-dashboard'); toast(r.message, 'ok'); } catch (e) { erreur(e); }
    };

    $('btn-open-modal-add-act').onclick = () => ouvrirModalActe();
    ['btn-close-modal-add-act', 'btn-cancel-modal-add-act'].forEach(id => { $(id).onclick = () => fermerModal('modal-add-act'); });
    ['modal-act-delai', 'modal-act-date', 'modal-act-libre-annees', 'modal-act-libre-mois', 'modal-act-libre-jours', 'modal-act-libre-distance'].forEach(id => { $(id).addEventListener('change', majModalActe); });
    $('btn-submit-modal-add-act').onclick = (e) => avecBouton(e.currentTarget, async () => {
        const aff = affaireActive();
        if (!aff) return;
        const nature = $('modal-act-nature').value.trim();
        const code = $('modal-act-delai').value;
        const r = regle(code);
        const mode = r ? r.mode : 'CALCULE';
        const acte = { nature, type_delai: code, date_depart: $('modal-act-date').value, date_echeance_manuelle: mode === 'FIXE' ? $('modal-act-date-manuelle').value : null };
        if (mode === 'LIBRE') {
            acte.duree_libre = dureeLibre('modal-act-libre-');
            acte.appliquer_distance = $('modal-act-libre-distance').checked;
            acte.fondement_libre = $('modal-act-libre-fondement').value.trim();
            acte.sanction_libre = $('modal-act-libre-sanction').value.trim();
        }
        if (!nature) { toast("Renseignez la nature de l'acte.", 'alerte'); return; }
        if (mode !== 'FIXE' && !acte.date_depart) { toast("Renseignez la date de l'acte.", 'alerte'); return; }
        if (mode === 'FIXE' && !acte.date_echeance_manuelle) { toast("Renseignez la date de l'échéance fixée.", 'alerte'); return; }
        const rep = await appelRust('ajouter_acte_au_dossier', { idAffaire: aff.id, acte });
        fermerModal('modal-add-act');
        appliquer(rep);
        toast('Échéance enregistrée et journalisée.', 'ok');
    });

    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape') ['modal-new-case', 'modal-add-act', 'modal-legifrance', 'palette-modal'].forEach(fermerModal);
    });
}

// -----------------------------------------------------------------------------
// 9. EXPORTS : AGENDA (.ics, RFC 5545) ET DOCUMENT WORD (.doc HTML)
// -----------------------------------------------------------------------------
function icsTexte(s) {
    return String(s || '').replace(/\\/g, '\\\\').replace(/;/g, '\\;').replace(/,/g, '\\,').replace(/\r?\n/g, '\\n');
}
function icsPlier(ligne) {
    const octets = new TextEncoder();
    if (octets.encode(ligne).length <= 75) return ligne;
    const morceaux = []; let courant = '';
    for (const ch of ligne) {
        if (octets.encode(courant + ch).length > (morceaux.length ? 74 : 75)) { morceaux.push(courant); courant = ch; } else courant += ch;
    }
    morceaux.push(courant);
    return morceaux.join('\r\n ');
}
async function exporterIcs(nomFichier, evenements) {
    const maintenant = new Date().toISOString().replace(/[-:]/g, '').replace(/\.\d{3}/, '');
    const lignes = ['BEGIN:VCALENDAR', 'VERSION:2.0', 'PRODID:-//Deadline Breaker//Procedure civile//FR', 'CALSCALE:GREGORIAN', 'METHOD:PUBLISH'];
    evenements.forEach((ev, i) => {
        if (!parseIso(ev.date)) return;
        lignes.push('BEGIN:VEVENT',
            `UID:lexprocedura-${Date.now()}-${i}@local`,
            `DTSTAMP:${maintenant}`,
            `DTSTART;VALUE=DATE:${ev.date.replace(/-/g, '')}`,
            `DTEND;VALUE=DATE:${ajouterJours(ev.date, 1).replace(/-/g, '')}`,
            `SUMMARY:${icsTexte('ÉCHÉANCE : ' + ev.titre)}`,
            `DESCRIPTION:${icsTexte(ev.description)}`,
            `LOCATION:${icsTexte(ev.lieu)}`,
            'TRANSP:TRANSPARENT',
            'BEGIN:VALARM', 'TRIGGER:-P7D', 'ACTION:DISPLAY', 'DESCRIPTION:Rappel J-7 : échéance procédurale', 'END:VALARM',
            'BEGIN:VALARM', 'TRIGGER:-P2D', 'ACTION:DISPLAY', 'DESCRIPTION:URGENT J-2 : échéance procédurale imminente', 'END:VALARM',
            'END:VEVENT');
    });
    lignes.push('END:VCALENDAR');
    return exporterFichier(`${nomFichier}.ics`, lignes.map(icsPlier).join('\r\n') + '\r\n');
}

async function exporterWord(nomFichier, titre, corpsHtml) {
    const cabinet = P().nom_cabinet ? esc(P().nom_cabinet) + ' &bull; ' : '';
    const html = `<html xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:w="urn:schemas-microsoft-com:office:word" xmlns="http://www.w3.org/TR/REC-html40">
<head><meta charset="utf-8"><title>${esc(titre)}</title>
<style>body{font-family:Calibri,Arial,sans-serif;font-size:11pt;color:#111;line-height:1.5}h1{font-size:15pt;color:#0369a1;border-bottom:2px solid #0369a1;padding-bottom:4px}.meta{color:#64748b;font-size:9.5pt;margin-bottom:16px}.footer{font-size:8.5pt;color:#64748b;border-top:1px solid #e2e8f0;margin-top:28px;padding-top:6px}</style></head>
<body><h1>${esc(titre)}</h1><div class="meta">${cabinet}${t('Document généré par Deadline Breaker le')} ${new Date().toLocaleString(locale())}</div>${corpsHtml}
<div class="footer">Projet de travail à vérifier par l'avocat au regard des textes en vigueur et des circonstances du dossier.</div></body></html>`;
    return exporterFichier(`${nomFichier}.doc`, html, true);
}
const enParagraphes = (texte) => esc(texte).replace(/\n/g, '<br>');

// -----------------------------------------------------------------------------
// 10. CALCUL DES DÉLAIS (moteur Rust) : résultat instantané, journalisé lorsqu'il est utilisé
// -----------------------------------------------------------------------------
function majModeExpress() {
    const r = regle($('exp-delai-select').value);
    const libre = r && r.mode === 'LIBRE';
    $('exp-libre-wrap').classList.toggle('hidden', !libre);
    $('exp-distance-force-wrap').classList.toggle('hidden', !libre);
    planifierExpress();
}

function basculerSegments(actif, inactifs) {
    actif.classList.add('actif');
    inactifs.forEach(b => b.classList.remove('actif'));
}

let minuterieExpress = null;
function planifierExpress() {
    clearTimeout(minuterieExpress);
    minuterieExpress = setTimeout(() => { calculerExpress().catch(erreur); }, 120);
}

function argsExpress(journaliser) {
    const code = $('exp-delai-select').value;
    const rg = regle(code);
    const args = {
        dateDepart: $('exp-date-input').value, typeDelai: code, residence: $('exp-distance-select').value,
        calendrier: $('exp-calendrier-select').value, journaliserCalcul: journaliser,
    };
    if (rg && rg.mode === 'LIBRE') { args.dureeLibre = dureeLibre('exp-libre-'); args.appliquerDistance = $('chk-exp-distance').checked; }
    return args;
}

function libelleJours(j) {
    if (j === 0) return "aujourd'hui";
    if (j > 0) return `dans ${j} jour${j > 1 ? 's' : ''}`;
    return `expiré depuis ${-j} jour${j < -1 ? 's' : ''}`;
}

function classeStatut(j) {
    if (j < 0) return 'statut-neutre';
    if (j <= (P().seuil_rouge_jours ?? 3)) return 'statut-danger';
    if (j <= (P().seuil_orange_jours ?? 7)) return 'statut-alerte';
    return 'statut-info';
}

let jetonExpress = 0;
async function calculerExpress() {
    const vide = !$('exp-date-input').value || !$('exp-delai-select').value;
    const jeton = ++jetonExpress;
    if (vide) { afficherExpress(null); return; }
    let calc;
    try {
        calc = await appelRust('calculer_delai', argsExpress(false));
    } catch (e) {
        if (jeton !== jetonExpress) return;
        afficherExpress(null, e.message);
        return;
    }
    if (jeton !== jetonExpress) return;
    etat.dernierExpress = calc;
    etat.expressJournalise = false;
    afficherExpress(calc);
}

function afficherExpress(calc, messageErreur) {
    const box = $('express-result-text');
    $('express-vide').classList.toggle('hidden', !!calc);
    box.classList.toggle('hidden', !calc);
    $('express-actions').classList.toggle('hidden', !calc);
    $('express-vide').querySelector('p').textContent = messageErreur || "Indiquez la date de l'acte : l'échéance s'affiche ici.";
    if (!calc) { etat.dernierExpress = null; return; }
    const r = calc.resultat, rg = calc.regle;
    const residenceHorsSiege = $('exp-distance-select').value !== 'FRANCE_METROPOLE';
    const distanceNonAppliquee = residenceHorsSiege && !r.mois_distance && rg.mode === 'CALCULE' && !rg.distance_643 && !rg.distance_644;
    const titre = rg.mode === 'LIBRE' ? `Délai libre de ${dureeTexte(r.duree)}` : rg.libelle;
    const fondement = rg.article_num ? boutonArticle(rg.article_code, rg.article_num, rg.fondement, 'lien-article') : esc(rg.fondement || '');
    const remarques = [];
    if (distanceNonAppliquee) remarques.push(`Augmentation de distance non applicable à ce délai.${rg.note ? ' ' + rg.note : ''}`);
    else if (rg.note) remarques.push(rg.note);
    box.innerHTML = `
        <span class="etiquette-statut ${classeStatut(r.jours_restants)}">${esc(libelleJours(r.jours_restants))}</span>
        <p class="date-majeure">${esc(dateEnLettres(r.date_echeance_legale))}</p>
        <p class="texte-2 text-sm mt-1">à 24 h${r.prorogation_appliquee ? ' · prorogé au premier jour ouvrable (art. 642 CPC)' : ''}</p>
        <dl class="mt-6">
            <div class="ligne-info"><dt>Délai</dt><dd>${esc(titre)}</dd></div>
            <div class="ligne-info"><dt>Fondement</dt><dd>${fondement}${r.mois_distance ? ` · distance +${r.mois_distance} mois (${boutonArticle('CPC', r.article_distance, 'art. ' + r.article_distance + ' CPC', 'lien-article')})` : ''}</dd></div>
            ${rg.mode === 'CALCULE' && rg.sanction ? `<div class="ligne-info"><dt>Sanction</dt><dd>${esc(rg.sanction)}</dd></div>` : ''}
            <div class="ligne-info"><dt>Jours fériés</dt><dd>${esc(libelleCalendrier(r.calendrier))}</dd></div>
        </dl>
        ${remarques.length ? `<p class="note-alerte mt-4" style="list-style:none;padding-left:1rem">${lierArticles(esc(remarques.join(' ')), 'CPC')}</p>` : ''}
        <details class="options-avancees mt-5">
            <summary>Détail du calcul</summary>
            <ol class="etapes-calcul">${r.etapes.map(x => `<li>${lierArticles(esc(x), 'CPC')}</li>`).join('')}</ol>
        </details>`;
    box.querySelectorAll('.btn-legal-article').forEach(b => b.classList.add('lien-article'));
    icones();
}

/** Le calcul est inscrit au journal d'audit dès qu'il est utilisé (copie, export, dossier). */
async function journaliserExpress() {
    if (!etat.dernierExpress || etat.expressJournalise) return;
    etat.expressJournalise = true;
    try { await appelRust('calculer_delai', argsExpress(true)); } catch (_) { etat.expressJournalise = false; }
}

function texteExpress() {
    const c = etat.dernierExpress;
    const r = c.resultat, rg = c.regle;
    return [`${rg.mode === 'LIBRE' ? 'Délai libre de ' + dureeTexte(r.duree) : rg.libelle} (${rg.fondement})`,
        `Point de départ : ${fmtLong(r.date_declencheur)}`,
        `Échéance : ${fmtLong(r.date_echeance_legale)} à 24 h`,
        ...(rg.mode === 'CALCULE' && rg.sanction ? [`Sanction : ${rg.sanction}`] : []),
        '', ...r.etapes.map((x, i) => `${i + 1}. ${x}`)].join('\n');
}

function initExpress() {
    const bD = $('btn-mode-cpc-direct'), bR = $('btn-mode-cpc-retro');
    bD.onclick = () => { basculerSegments(bD, [bR]); $('section-cpc-direct').classList.remove('hidden'); $('section-cpc-retro').classList.add('hidden'); };
    bR.onclick = () => { basculerSegments(bR, [bD]); $('section-cpc-retro').classList.remove('hidden'); $('section-cpc-direct').classList.add('hidden'); };
    $('exp-delai-select').addEventListener('change', majModeExpress);
    ['exp-date-input', 'exp-distance-select', 'exp-calendrier-select', 'chk-exp-distance', 'exp-libre-annees', 'exp-libre-mois', 'exp-libre-jours']
        .forEach(id => { $(id).addEventListener('input', planifierExpress); $(id).addEventListener('change', planifierExpress); });

    $('btn-copy-express').onclick = async () => {
        if (!etat.dernierExpress) return;
        try { await navigator.clipboard.writeText(texteExpress()); journaliserExpress(); toast('Calcul copié.', 'ok'); } catch (e) { erreur(e); }
    };
    $('btn-express-dossier').onclick = () => {
        const c = etat.dernierExpress;
        if (!c) return;
        if (!affaireActive()) return toast("Ouvrez d'abord un dossier (rubrique Dossiers) pour y ajouter cette échéance.", 'alerte');
        journaliserExpress();
        ouvrirModalActe({ type: c.regle.code, date: c.resultat.date_declencheur, duree: c.regle.mode === 'LIBRE' ? c.resultat.duree : null, distance: $('chk-exp-distance').checked });
    };
    $('btn-export-cpc-ics').onclick = () => {
        const c = etat.dernierExpress;
        if (!c) return;
        journaliserExpress();
        exporterIcs(`Echeance_${c.regle.code}_${c.resultat.date_echeance_legale}`, [{ titre: c.regle.mode === 'LIBRE' ? `Délai de ${dureeTexte(c.resultat.duree)}` : c.regle.libelle, date: c.resultat.date_echeance_legale, description: `${c.regle.fondement}\n${c.resultat.justification_pas_a_pas}` }]).catch(erreur);
    };
    $('btn-export-cpc-word').onclick = () => {
        const c = etat.dernierExpress;
        if (!c) return;
        journaliserExpress();
        const r = c.resultat;
        exporterWord(`Calcul_delai_${r.date_echeance_legale}`, 'Computation de délai de procédure civile',
            `<p><b>Délai :</b> ${esc(c.regle.mode === 'LIBRE' ? 'Délai libre de ' + dureeTexte(r.duree) : c.regle.libelle)} (${esc(c.regle.fondement)})</p><p><b>Origine :</b> ${esc(fmtLong(r.date_declencheur))}</p><p><b>Calendrier :</b> ${esc(libelleCalendrier(r.calendrier))}</p><p><b>Échéance :</b> ${esc(fmtLong(r.date_echeance_legale))} à 24 h</p>${c.regle.mode === 'CALCULE' ? `<p><b>Sanction :</b> ${esc(c.regle.sanction)}</p>` : ''}<ol>${r.etapes.map(x => `<li>${esc(x)}</li>`).join('')}</ol>`).catch(erreur);
    };

    $('retro-date-audience').addEventListener('change', () => calculerRetro().catch(erreur));
    $('retro-date-audience').addEventListener('input', () => calculerRetro().catch(erreur));
    $('retro-type-instance').addEventListener('change', () => calculerRetro().catch(erreur));
    $('btn-export-retro-ics').onclick = () => {
        if (!etat.dernierRetro.length) return;
        journaliserRetro();
        exporterIcs(`Retroplanning_audience_${$('retro-date-audience').value}`, etat.dernierRetro.map(x => ({ titre: x.label, date: x.date, description: "Rétro-planning d'audience indicatif (Deadline Breaker)" }))).catch(erreur);
    };
    $('btn-export-retro-word').onclick = () => {
        if (!etat.dernierRetro.length) return;
        journaliserRetro();
        exporterWord(`Retroplanning_audience_${$('retro-date-audience').value}`, "Rétro-planning d'audience (indicatif)",
            `<table border="1" cellpadding="6" style="border-collapse:collapse">${etat.dernierRetro.map(x => `<tr><td>${esc(x.label)}</td><td><b>${esc(fmtLong(x.date))}</b></td></tr>`).join('')}</table>`).catch(erreur);
    };
}

const JALONS = [
    { j: 30, label: 'Conclusions récapitulatives' },
    { j: 20, label: 'Communication ultime des pièces et bordereau' },
    { j: 15, label: 'Clôture prévisionnelle (à confirmer avec le calendrier du juge)' },
    { j: 8, label: 'Dépôt du dossier de plaidoirie' },
];

let jetonRetro = 0;
async function calculerRetro() {
    const dAud = $('retro-date-audience').value;
    const jeton = ++jetonRetro;
    const montrer = (ok) => {
        $('retro-vide').classList.toggle('hidden', ok);
        ['retro-steps-container', 'retro-note', 'retro-actions'].forEach(id => $(id).classList.toggle('hidden', !ok));
    };
    if (!dAud) { etat.dernierRetro = []; montrer(false); return; }
    const aff = affaireActive();
    const etapes = await appelRust('calculer_rebours', { dateReference: dAud, joursAvant: JALONS.map(x => x.j), calendrier: aff ? calendrierAffaire(aff) : null });
    if (jeton !== jetonRetro) return;
    etat.dernierRetro = etapes.map((x, i) => ({ label: JALONS[i].label, marge: JALONS[i].j, date: x.date_retenue, brute: x.date_brute, anticipe: x.jours_anticipes }));
    etat.dernierRetro.push({ label: `Audience - ${$('retro-type-instance').selectedOptions[0].text}`, date: dAud, brute: dAud, anticipe: 0, audience: true });
    etat.retroJournalise = false;
    $('retro-steps-container').innerHTML = etat.dernierRetro.map(evt => {
        const j = joursJusqua(evt.date);
        return `<div class="jalon${j < 0 ? ' passe' : ''}${evt.audience ? ' audience' : ''}">
            <p class="jalon-date">${esc(dateEnLettres(evt.date))}${evt.marge ? ` <span class="texte-3 font-normal text-xs">J-${evt.marge}</span>` : ''}</p>
            <p class="jalon-libelle">${esc(evt.label)}${evt.anticipe ? ` · avancé de ${evt.anticipe} j (${fmt(evt.brute)} non ouvrable)` : ''}${j < 0 ? ' · date passée' : ''}</p>
        </div>`;
    }).join('');
    montrer(true);
}

function journaliserRetro() {
    if (etat.retroJournalise || !etat.dernierRetro.length) return;
    etat.retroJournalise = true;
    const aff = affaireActive();
    appelRust('enregistrer_historique', { categorie: 'CALCUL', titre: "Rétro-planning d'audience", details: etat.dernierRetro.map(x => `${x.label} : ${fmt(x.date)}`).join(' | '), fondement: 'Planning interne indicatif', idAffaire: aff ? aff.id : null }).catch(() => {});
}

function dureeTexte(d) {
    if (!d) return '';
    const p = [];
    const en = langue() === 'en';
    if (d.annees) p.push(en ? `${d.annees} year${d.annees > 1 ? 's' : ''}` : `${d.annees} an${d.annees > 1 ? 's' : ''}`);
    if (d.mois) p.push(en ? `${d.mois} month${d.mois > 1 ? 's' : ''}` : `${d.mois} mois`);
    if (d.jours) p.push(en ? `${d.jours} day${d.jours > 1 ? 's' : ''}` : `${d.jours} jour${d.jours > 1 ? 's' : ''}`);
    return p.join(en ? ' and ' : ' et ') || (en ? '0 days' : '0 jour');
}

// -----------------------------------------------------------------------------
// 11. PRESCRIPTION & FORCLUSION (moteur Rust) : résultat instantané
//     Interruption : art. 2231, 2241, 2242, 2243 C. civ. - Suspension : art. 2238, 2239 - Butoir : art. 2232.
// -----------------------------------------------------------------------------
const EVENEMENTS_PRESCRIPTION = {
    DEMANDE_JUSTICE: { titre: 'Demande en justice', champs: () => ({ date_assign: '', date_fin: '', non_avenue: false }) },
    REFERE_EXPERTISE: { titre: 'Référé-expertise', champs: () => ({ date_assign: '', date_ord: '', date_depot: '' }) },
    MEDIATION: { titre: 'Médiation ou conciliation', champs: () => ({ date_debut: '', date_fin: '' }) },
};

function ajouterEvenementPrescription(type) {
    const modele = EVENEMENTS_PRESCRIPTION[type];
    if (!modele) return;
    etat.prescription.push({ id: 'evt-' + Date.now() + '-' + etat.prescription.length, type, ...modele.champs() });
    $('menu-evenements').classList.add('hidden');
    renderEvenements();
    computePrescription();
}

function initPrescription() {
    $('btn-add-event').onclick = (e) => { e.stopPropagation(); $('menu-evenements').classList.toggle('hidden'); };
    $('menu-evenements').addEventListener('click', (e) => {
        const b = e.target.closest('[data-evenement]');
        if (b) ajouterEvenementPrescription(b.dataset.evenement);
    });
    document.addEventListener('click', (e) => { if (!e.target.closest('#menu-evenements')) $('menu-evenements').classList.add('hidden'); });
    document.querySelectorAll('[data-qualification]').forEach(b => {
        b.onclick = () => {
            document.querySelectorAll('[data-qualification]').forEach(x => x.classList.toggle('actif', x === b));
            $('sim-qualification').value = b.dataset.qualification;
            computePrescription();
        };
    });
    $('btn-reset-prescription').onclick = () => {
        etat.prescription = [];
        $('sim-date-depart').value = '';
        $('sim-date-naissance').value = '';
        $('sim-date-cible').value = aujourdHui();
        renderEvenements(); computePrescription();
    };
    ['sim-regime', 'sim-date-depart', 'sim-date-cible', 'sim-date-naissance'].forEach(id => { $(id).onchange = computePrescription; });

    $('events-timeline-list').addEventListener('change', (e) => {
        const champ = e.target.dataset.champ, id = e.target.dataset.id;
        if (!champ) return;
        const ev = etat.prescription.find(x => x.id === id);
        if (ev) { ev[champ] = e.target.type === 'checkbox' ? e.target.checked : e.target.value; computePrescription(); }
    });
    $('events-timeline-list').addEventListener('click', (e) => {
        const b = e.target.closest('[data-suppr]');
        if (!b) return;
        etat.prescription = etat.prescription.filter(x => x.id !== b.dataset.suppr);
        renderEvenements(); computePrescription();
    });

    const texteAnalyse = () => $('syllogisme-output').textContent;
    $('btn-copy-syllogisme').onclick = async () => { try { await navigator.clipboard.writeText(texteAnalyse()); toast('Raisonnement copié.', 'ok'); } catch (e) { erreur(e); } };
    $('btn-export-presc-word').onclick = () => {
        const points = Array.from($('presc-vigilance').querySelectorAll('li')).map(li => li.textContent);
        exporterWord(`Analyse_prescription_${aujourdHui()}`, 'Analyse de prescription / forclusion', enParagraphes(texteAnalyse() + (points.length ? '\n\nPoints à vérifier :\n- ' + points.join('\n- ') : ''))).catch(erreur);
    };
    $('btn-export-presc-ics').onclick = () => {
        const r = etat.dernierePrescription;
        if (!r || !r.fin) return toast("Aucune date d'expiration calculable.", 'alerte');
        exporterIcs(`Prescription_${r.fin}`, [{ titre: 'Expiration du délai de prescription / forclusion', date: r.fin, description: texteAnalyse() }]).catch(erreur);
    };
    $('btn-save-presc-to-case').onclick = (e) => avecBouton(e.currentTarget, async () => {
        const aff = affaireActive();
        const r = etat.dernierePrescription;
        if (!aff) return toast("Ouvrez d'abord un dossier (rubrique Dossiers) pour y ajouter cette date.", 'alerte');
        if (!r || !r.fin) return toast("Aucune date d'expiration calculable à rattacher.", 'alerte');
        const rep = await appelRust('ajouter_acte_au_dossier', { idAffaire: aff.id, acte: { nature: `Expiration du délai de ${$('sim-qualification').value === 'FORCLUSION' ? 'forclusion' : 'prescription'} (simulateur)`, type_delai: 'ECHEANCE_FIXEE', date_depart: $('sim-date-depart').value, date_echeance_manuelle: r.fin } });
        await appelRust('enregistrer_historique', { categorie: 'CALCUL', titre: 'Analyse de prescription rattachée au dossier', details: texteAnalyse(), fondement: r.fondement || 'Art. 2219 et s. C. civ.', idAffaire: aff.id });
        appliquer(rep);
        toast(`Date limite du ${fmt(r.fin)} ajoutée au dossier ${aff.nom_dossier}.`, 'ok');
    });
}

function champDate(ev, champ, libelle) {
    return `<div><label>${libelle}</label><input type="date" value="${esc(ev[champ] || '')}" data-id="${esc(ev.id)}" data-champ="${champ}" class="champ"></div>`;
}

function renderEvenements() {
    const c = $('events-timeline-list');
    c.innerHTML = etat.prescription.map(ev => {
        const entete = (titre, refs) => `<div class="evenement-titre"><span>${titre} <span class="font-normal texte-3">${refs}</span></span>
            <button type="button" class="btn-icone" style="width:1.75rem;height:1.75rem" title="Retirer" data-suppr="${esc(ev.id)}"><i data-lucide="x" class="w-3.5 h-3.5"></i></button></div>`;
        if (ev.type === 'REFERE_EXPERTISE') return `<div class="evenement">${entete('Référé-expertise', `${boutonArticle('CCIV', '2241', 'art. 2241', 'lien-article')} et ${boutonArticle('CCIV', '2239', '2239', 'lien-article')}`)}
            <div class="grid grid-cols-3 gap-2">${champDate(ev, 'date_assign', 'Assignation')}${champDate(ev, 'date_ord', 'Ordonnance')}${champDate(ev, 'date_depot', 'Dépôt du rapport')}</div></div>`;
        if (ev.type === 'DEMANDE_JUSTICE') return `<div class="evenement">${entete('Demande en justice', `${boutonArticle('CCIV', '2241', 'art. 2241', 'lien-article')} et ${boutonArticle('CCIV', '2242', '2242', 'lien-article')}`)}
            <div class="grid grid-cols-2 gap-2">${champDate(ev, 'date_assign', 'Date de la demande')}${champDate(ev, 'date_fin', "Fin de l'instance")}</div>
            <label class="case-a-cocher mt-2" style="font-size:12px"><input type="checkbox" data-id="${esc(ev.id)}" data-champ="non_avenue" ${ev.non_avenue ? 'checked' : ''}> <span>Désistement, péremption ou rejet (${boutonArticle('CCIV', '2243', 'art. 2243', 'lien-article')})</span></label></div>`;
        return `<div class="evenement">${entete('Médiation ou conciliation', boutonArticle('CCIV', '2238', 'art. 2238', 'lien-article'))}
            <div class="grid grid-cols-2 gap-2">${champDate(ev, 'date_debut', 'Début')}${champDate(ev, 'date_fin', 'Fin')}</div></div>`;
    }).join('');
    icones();
}

/** Durée lisible entre deux dates ISO : « 4 ans et 3 mois », « 12 jours ». */
function dureeEntre(isoA, isoB) {
    const a = parseIso(isoA), b = parseIso(isoB);
    let mois = (b.getFullYear() - a.getFullYear()) * 12 + (b.getMonth() - a.getMonth());
    if (b.getDate() < a.getDate()) mois -= 1;
    if (mois < 1) { const j = joursEntre(isoA, isoB); return `${j} jour${j > 1 ? 's' : ''}`; }
    const ans = Math.floor(mois / 12), m = mois % 12;
    return [ans ? `${ans} an${ans > 1 ? 's' : ''}` : '', m ? `${m} mois` : ''].filter(Boolean).join(' et ');
}

let jetonPrescription = 0;
async function computePrescription() {
    const t0 = $('sim-date-depart').value;
    const badge = $('verdict-badge');
    const out = $('syllogisme-output');
    const vig = $('presc-vigilance');
    etat.dernierePrescription = null;
    const jeton = ++jetonPrescription;
    const montrer = (ok, message) => {
        $('presc-vide').classList.toggle('hidden', ok);
        $('presc-resultat').classList.toggle('hidden', !ok);
        $('presc-actions').classList.toggle('hidden', !ok);
        if (!ok) $('presc-vide').querySelector('p').textContent = message;
    };

    if (!t0) {
        badge.className = 'etiquette-statut statut-neutre';
        badge.textContent = 'Point de départ à saisir';
        out.textContent = '';
        vig.innerHTML = '';
        montrer(false, "Choisissez l'action et son point de départ : le résultat s'affiche ici.");
        return;
    }
    const demande = {
        regime: $('sim-regime').value,
        qualification: $('sim-qualification').value,
        date_depart: t0,
        date_naissance_droit: $('sim-date-naissance').value || null,
        date_cible: $('sim-date-cible').value || aujourdHui(),
        evenements: etat.prescription.map(ev => ({
            type: ev.type, date_assign: ev.date_assign || null, date_ord: ev.date_ord || null, date_depot: ev.date_depot || null,
            date_debut: ev.date_debut || null, date_fin: ev.date_fin || null, non_avenue: !!ev.non_avenue,
        })),
    };
    let r;
    try {
        r = await appelRust('analyser_prescription', { demande });
    } catch (e) {
        if (jeton !== jetonPrescription) return;
        badge.className = 'etiquette-statut statut-alerte';
        badge.textContent = 'Saisie à corriger';
        montrer(false, e.message);
        return;
    }
    if (jeton !== jetonPrescription) return;
    montrer(true);

    const cible = demande.date_cible;
    const nature = demande.qualification === 'FORCLUSION' ? 'Forclos' : 'Prescrit';
    if (r.verdict === 'EN_SUSPENS') {
        badge.className = 'etiquette-statut statut-alerte';
        badge.textContent = 'Délai interrompu ou suspendu, en cours';
        $('presc-date').textContent = 'Point de reprise à venir';
        $('presc-reste').textContent = "Le nouveau délai court à compter de la fin de l'instance ou de la mesure. Renseignez sa date pour obtenir l'échéance.";
    } else if (r.verdict === 'NON_ACQUISE') {
        badge.className = 'etiquette-statut statut-ok';
        badge.textContent = `Non acquise au ${fmt(cible)}`;
        $('presc-date').textContent = `${nature} le ${dateEnLettres(ajouterJours(r.date_expiration, 1))}`;
        $('presc-reste').textContent = `Dernier jour pour agir : ${dateEnLettres(r.date_expiration)} · reste ${dureeEntre(cible, r.date_expiration)}`;
    } else {
        badge.className = 'etiquette-statut statut-danger';
        badge.textContent = `Acquise au ${fmt(cible)}`;
        $('presc-date').textContent = `${nature} depuis le ${dateEnLettres(ajouterJours(r.date_expiration, 1))}`;
        $('presc-reste').textContent = `Dernier jour pour agir : ${dateEnLettres(r.date_expiration)}`;
    }
    if (r.date_expiration) etat.dernierePrescription = { fin: r.date_expiration, fondement: r.fondement };
    const regime = (etat.ref.regimes_prescription || []).find(x => x.code === demande.regime);
    $('presc-fondement').innerHTML = regime && regime.article_num
        ? `Fondement : ${boutonArticle(regime.article_code, regime.article_num, regime.fondement, 'lien-article')}${r.date_butoir ? ` · délai butoir au ${esc(fmt(r.date_butoir))} (${boutonArticle('CCIV', '2232', 'art. 2232 C. civ.', 'lien-article')})` : ''}`
        : esc(r.fondement || '');
    out.innerHTML = lierArticles(esc(r.etapes.join('\n') + (r.date_butoir ? `\n\nDélai butoir (art. 2232 C. civ.) : ${fmt(r.date_butoir)}.` : '')), 'CCIV');
    out.querySelectorAll('.btn-legal-article').forEach(b => b.classList.add('lien-article'));
    vig.innerHTML = (r.vigilance || []).map(v => `<li>${lierArticles(esc(v), 'CCIV')}</li>`).join('');
    vig.classList.toggle('hidden', !(r.vigilance || []).length);

    $('timeline-lbl-start').textContent = fmt(t0);
    $('timeline-lbl-end').textContent = r.date_expiration ? fmt(r.date_expiration) : 'en suspens';
    const total = Math.max(1, r.frise_total);
    const pInit = Math.round(r.frise_initial / total * 100);
    const pNeut = Math.min(100 - pInit, Math.round(r.frise_neutralise / total * 100));
    $('bar-initial').style.width = pInit + '%';
    $('bar-suspended').style.width = pNeut + '%';
    $('bar-remaining').style.width = Math.max(0, 100 - pInit - pNeut) + '%';
    $('timeline-lbl-susp').classList.toggle('invisible', pNeut === 0);
    icones();
}

// -----------------------------------------------------------------------------
// 12. STRATÉGIE DE DÉFENSE (ART. 74 CPC)
// -----------------------------------------------------------------------------
function initIncidents() {
    document.querySelectorAll('#incident-checkboxes-grid input').forEach(c => { c.onchange = recomputeDispositif; });
    $('chk-timing-conclu-fond').onchange = recomputeDispositif;
    $('btn-copy-dispositif').onclick = async () => { try { await navigator.clipboard.writeText($('dispositif-output').textContent); toast('Dispositif copié.', 'ok'); } catch (e) { erreur(e); } };
    $('btn-export-dispositif-word').onclick = () => exporterWord(`Projet_dispositif_${aujourdHui()}`, 'Projet de dispositif des conclusions', enParagraphes($('dispositif-output').textContent)).catch(erreur);
}

function recomputeDispositif() {
    const tardif = $('chk-timing-conclu-fond').checked;
    const coches = Array.from(document.querySelectorAll('#incident-checkboxes-grid input:checked')).map(c => c.dataset.incident);
    const en = langue() === 'en';
    const T = en ? {
        tete: 'ON THESE GROUNDS\n',
        incomp: "IN LIMINE LITIS:\n- Having regard to articles 74 and 75 of the Code of Civil Procedure;\n- DECLARE that the court lacks jurisdiction in favour of [designated court];\n- REFER the case and the parties to that court.\n",
        incompTard: "Objection of lack of jurisdiction (art. 75 CPC): inadmissible when raised after a defence on the merits or a plea of inadmissibility (art. 74 CPC). Not included in the operative part.",
        forme: "IN LIMINE LITIS:\n- Having regard to articles 74 and 114 of the Code of Civil Procedure;\n- DECLARE the originating document null and void for a formal defect, the irregularity causing the following prejudice to the party: [prejudice to be demonstrated].\n",
        formeTard: 'Nullity for a formal defect (art. 114 CPC): a procedural objection that is inadmissible after a defence on the merits (art. 74 CPC; see also art. 112). Not included in the operative part.',
        fond: "IN ANY EVENT:\n- Having regard to articles 117 to 119 of the Code of Civil Procedure;\n- DECLARE the document null and void for a substantive irregularity (lack of capacity or authority), without the party having to show any prejudice.\n",
        fnr: "IN ANY EVENT:\n- Having regard to articles 122 and 123 of the Code of Civil Procedure and [legal basis of the limitation period];\n- DECLARE [the opposing party]'s claims inadmissible.\n",
        fin: "IN THE ALTERNATIVE, ON THE MERITS:\n- DISMISS all of [the opposing party]'s claims.\n\nIN ANY EVENT:\n- ORDER [the opposing party] to pay the party the sum of €[amount] under article 700 of the Code of Civil Procedure;\n- ORDER [the opposing party] to pay all costs of the proceedings.",
    } : {
        tete: 'PAR CES MOTIFS\n',
        incomp: "IN LIMINE LITIS :\n- Vu les articles 74 et 75 du Code de procédure civile ;\n- SE DÉCLARER incompétent au profit de [juridiction désignée] ;\n- RENVOYER la cause et les parties devant cette juridiction.\n",
        incompTard: "Exception d'incompétence (art. 75 CPC) : irrecevable si elle est soulevée après une défense au fond ou une fin de non-recevoir (art. 74 CPC). Non reprise dans le dispositif.",
        forme: "IN LIMINE LITIS :\n- Vu les articles 74 et 114 du Code de procédure civile ;\n- PRONONCER la nullité de l'acte introductif pour vice de forme, l'irrégularité causant au concluant le grief suivant : [grief à démontrer].\n",
        formeTard: 'Nullité pour vice de forme (art. 114 CPC) : exception de procédure irrecevable après défense au fond (art. 74 CPC ; voir aussi art. 112). Non reprise dans le dispositif.',
        fond: "EN TOUT ÉTAT DE CAUSE :\n- Vu les articles 117 à 119 du Code de procédure civile ;\n- PRONONCER la nullité de l'acte pour irrégularité de fond (défaut de capacité ou de pouvoir), sans que le concluant ait à justifier d'un grief.\n",
        fnr: "EN TOUT ÉTAT DE CAUSE :\n- Vu les articles 122 et 123 du Code de procédure civile et [fondement de la prescription] ;\n- DÉCLARER [la partie adverse] irrecevable en ses demandes.\n",
        fin: "À TITRE SUBSIDIAIRE, AU FOND :\n- DÉBOUTER [la partie adverse] de l'ensemble de ses demandes, fins et conclusions.\n\nEN TOUT ÉTAT DE CAUSE :\n- CONDAMNER [la partie adverse] à payer au concluant la somme de [montant] € au titre de l'article 700 du Code de procédure civile ;\n- CONDAMNER [la partie adverse] aux entiers dépens.",
    };
    const lignes = [T.tete];
    const alertes = [];
    if (coches.includes('INCOMPETENCE')) { if (tardif) alertes.push(T.incompTard); else lignes.push(T.incomp); }
    if (coches.includes('VICE_FORME')) { if (tardif) alertes.push(T.formeTard); else lignes.push(T.forme); }
    if (coches.includes('VICE_FOND')) lignes.push(T.fond);
    if (coches.includes('FNR_PRESCRIPTION')) lignes.push(T.fnr);
    lignes.push(T.fin);
    $('dispositif-output').textContent = lignes.join('\n');
    $('dispositif-alertes').innerHTML = alertes.map(a => `<li>${lierArticles(esc(a), 'CPC')}</li>`).join('');
    $('dispositif-alertes').classList.toggle('hidden', !alertes.length);
}

// -----------------------------------------------------------------------------
// 13. COMPÉTENCE, REPRÉSENTATION ET VOIE DE RECOURS (moteur Rust)
// -----------------------------------------------------------------------------
function initCompetence() {
    ['tree-nat', 'tree-montant', 'tree-regle-ter', 'tree-ville'].forEach(id => { $(id).oninput = recomputeCompetence; $(id).onchange = recomputeCompetence; });
    $('btn-competence-prefill').onclick = () => {
        const aff = affaireActive();
        if (!aff) return toast("Ouvrez d'abord un dossier depuis la rubrique Dossiers.", 'alerte');
        if (aff.enjeu_financier) $('tree-montant').value = aff.enjeu_financier;
        toast(`Montant repris du dossier « ${aff.nom_dossier} ».`, 'ok');
        recomputeCompetence();
    };
    $('btn-apply-competence-active-case').onclick = (e) => avecBouton(e.currentTarget, async () => {
        const aff = affaireActive();
        if (!aff) return toast("Ouvrez d'abord un dossier depuis la liste des affaires.", 'alerte');
        const nom = etat.derniereCompetence ? etat.derniereCompetence.juridiction : '';
        if (!nom) return;
        if (!await uiConfirm('Appliquer la juridiction', `Remplacer la juridiction du dossier « ${aff.nom_dossier} » par :\n${nom} ?`, 'Appliquer')) return;
        const r = await appelRust('modifier_affaire', { idAffaire: aff.id, affaire: saisieAffaireDepuis(aff, { juridiction: nom }) });
        appliquer(r); toast('Juridiction appliquée au dossier.', 'ok');
    });
    $('btn-copy-competence').onclick = async () => { try { await navigator.clipboard.writeText($('tree-verdict-details').innerText); toast('Orientation copiée.', 'ok'); } catch (e) { erreur(e); } };
    $('btn-export-competence-word').onclick = () => exporterWord(`Analyse_competence_${aujourdHui()}`, 'Orientation - compétence, représentation et recours', $('tree-verdict-details').innerHTML).catch(erreur);
}

function ligneCompetence(titre, texte, ref, icone, extra = '') {
    const corps = lierArticles(esc(texte));
    // Référence ajoutée seulement si le texte ne cite pas déjà l'article (évite le doublon).
    const lien = ref && !corps.includes('btn-legal-article') ? ' ' + boutonArticle(ref.code, ref.article, ref.libelle, 'lien-article') : '';
    return `<dt>${esc(titre)}</dt><dd>${corps}${lien}${extra}</dd>`;
}

let jetonCompetence = 0;
async function recomputeCompetence() {
    const brut = String($('tree-montant').value).replace(',', '.').trim();
    const demande = { matiere: $('tree-nat').value, montant: brut === '' ? null : Number(brut), regle_territoriale: $('tree-regle-ter').value, ville: $('tree-ville').value.trim() };
    const box = $('tree-verdict-details');
    if (demande.montant !== null && !Number.isFinite(demande.montant)) { box.textContent = 'Montant invalide.'; etat.cleCompetence = null; return; }
    // Pas de nouveau rendu si la demande est inchangée (sinon un clic suivant la sortie d'un champ serait perdu).
    const cle = JSON.stringify(demande);
    if (cle === etat.cleCompetence) return;
    etat.cleCompetence = cle;
    const jeton = ++jetonCompetence;
    let r;
    try { r = await appelRust('orienter_competence', { demande }); }
    catch (e) { if (jeton === jetonCompetence) { box.textContent = e.message; etat.cleCompetence = null; } return; }
    if (jeton !== jetonCompetence) return;
    etat.derniereCompetence = r;
    const regleRecours = r.code_delai_recours ? regle(r.code_delai_recours) : null;
    const boutonRecours = regleRecours ? `<span class="block mt-1"><button type="button" id="btn-competence-delai" class="lien-article text-sm">Calculer ce délai à partir d'une notification →</button></span>` : '';
    box.innerHTML = `
        <p class="text-xs font-medium texte-3">Juridiction compétente</p>
        <p id="tree-jur-name" class="titre-resultat">${esc(r.juridiction)}</p>
        <dl class="bloc-resultat mt-2">
            ${ligneCompetence('Compétence matérielle', r.materielle, r.reference_materielle, 'scale')}
            ${ligneCompetence('Compétence territoriale', r.territoriale, null, 'map-pin')}
            ${ligneCompetence('Représentation par avocat', r.representation, r.reference_representation, 'user-check')}
            ${ligneCompetence('Taux du ressort', r.ressort, r.reference_ressort, 'landmark')}
            ${ligneCompetence('Voie de recours', r.recours, null, 'corner-up-right', boutonRecours)}
        </dl>
        ${r.observations.length ? `<p class="note-alerte mt-5" style="list-style:none;padding-left:1rem">${r.observations.map(esc).join(' ')}</p>` : ''}`;
    box.querySelectorAll('.btn-legal-article').forEach(b => b.classList.add('lien-article'));
    if (regleRecours) {
        $('btn-competence-delai').onclick = () => {
            changerVue('view-cpc-express');
            $('btn-mode-cpc-direct').click();
            $('exp-delai-select').value = regleRecours.code;
            majModeExpress();
            $('exp-date-input').focus();
            toast(`Délai sélectionné : ${regleRecours.libelle}. Saisissez la date de notification.`, 'info');
        };
    }
    icones();
}

// -----------------------------------------------------------------------------
// 14. COPILOTE IA (GEMINI OU CLAUDE) - appels par le moteur Rust : la clé API ne transite pas par l'interface,
//     l'anonymisation est appliquée avant envoi et les noms sont rétablis localement dans la réponse.
// -----------------------------------------------------------------------------
const LARGEUR_DOCK = { normal: 440, large: 680 };

function lirePreference(cle, defaut) { try { const v = localStorage.getItem(cle); return v === null ? defaut : v; } catch (_) { return defaut; } }
function ecrirePreference(cle, valeur) { try { localStorage.setItem(cle, valeur); } catch (_) { /* préférence non conservée */ } }

function dockOuvert() { return !$('ai-global-dock').classList.contains('translate-x-full'); }

/** Le panneau s'insère à côté du contenu (sans le recouvrir) ; la poignée suit son bord gauche. */
function majDisposition() {
    const largeur = etat.dockLarge ? LARGEUR_DOCK.large : LARGEUR_DOCK.normal;
    const ouvert = dockOuvert();
    $('ai-global-dock').style.width = largeur + 'px';
    document.querySelector('main').style.marginRight = ouvert ? largeur + 'px' : '';
    const poignee = $('ai-collapsed-tab');
    poignee.style.right = ouvert ? largeur + 'px' : '0px';
    poignee.classList.toggle('ouverte', ouvert);
    poignee.title = ouvert ? 'Fermer le copilote IA' : 'Ouvrir le copilote IA';
    poignee.setAttribute('aria-expanded', ouvert ? 'true' : 'false');
}

function ouvrirDock() {
    $('ai-global-dock').classList.remove('translate-x-full');
    updateAiContextPill();
    majDisposition();
}

function fermerDock() {
    $('ai-global-dock').classList.add('translate-x-full');
    majDisposition();
}

function initDockIA() {
    etat.dockLarge = lirePreference('lexprocedura_dock_large', '0') === '1';
    $('ai-collapsed-tab').onclick = () => { if (dockOuvert()) fermerDock(); else { ouvrirDock(); $('chat-user-input').focus(); } };
    $('btn-toggle-ai-width').onclick = () => {
        etat.dockLarge = !etat.dockLarge;
        ecrirePreference('lexprocedura_dock_large', etat.dockLarge ? '1' : '0');
        $('btn-toggle-ai-width').innerHTML = `<i data-lucide="${etat.dockLarge ? 'minimize-2' : 'maximize-2'}" class="w-4 h-4"></i>`;
        $('btn-toggle-ai-width').title = etat.dockLarge ? 'Réduire le panneau' : 'Élargir le panneau';
        majDisposition();
        icones();
    };
    $('btn-clear-chat').onclick = () => { etat.chat = []; renderAiWelcomeMessage(); };
    document.querySelectorAll('.btn-quick-prompt').forEach(b => { b.onclick = () => envoyerCopilote(b.dataset.prompt); });
    $('chat-input-form').onsubmit = (e) => {
        e.preventDefault();
        const t = $('chat-user-input').value.trim();
        if (t) { envoyerCopilote(t); $('chat-user-input').value = ''; }
    };
    $('chat-user-input').addEventListener('keydown', (e) => {
        if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); $('chat-input-form').requestSubmit(); }
    });
    majDisposition();
}

const fournisseurIa = () => (P().ia_fournisseur === 'CLAUDE' ? 'CLAUDE' : 'GEMINI');
const nomFournisseur = () => (fournisseurIa() === 'CLAUDE' ? 'Claude' : 'Gemini');
const modeleIa = () => (fournisseurIa() === 'CLAUDE' ? (P().claude_model || 'claude-opus-5-5') : (P().gemini_model || 'gemini-2.5-flash'));
const copiloteConfigure = () => !!((fournisseurIa() === 'CLAUDE' ? P().claude_api_key : P().gemini_api_key) || '').trim();

function renderAiWelcomeMessage() {
    const anon = P().gemini_anonymize !== false;
    $('chat-messages-container').innerHTML = `
        <div class="flex gap-2">
            <div class="w-6 h-6 rounded-full bg-indigo-600 text-white flex items-center justify-center shrink-0 mt-0.5"><i data-lucide="bot" class="w-3.5 h-3.5"></i></div>
            <div class="flex-1 bg-white p-3 rounded-2xl rounded-tl-none border border-slate-200/80 text-slate-800 space-y-1">
                <p class="font-bold text-indigo-950">Copilote de procédure civile</p>
                <p class="text-slate-600 text-[11px] leading-relaxed">${copiloteConfigure()
                    ? `Le contexte du dossier sélectionné est transmis à ${nomFournisseur()}${anon ? ' après anonymisation sur ce poste (parties, n° RG, termes confidentiels du dossier, e-mails, téléphones, IBAN). Les noms sont rétablis localement dans les réponses' : ' sans anonymisation (désactivée dans les Paramètres)'}. Les réponses sont des projets à vérifier.`
                    : `Renseignez votre clé API ${nomFournisseur()} dans <strong>Paramètres</strong> pour activer le copilote.`}</p>
            </div>
        </div>`;
    updateAiContextPill();
    icones();
}

function updateAiContextPill() {
    const aff = affaireActive();
    $('chat-active-context-label').textContent = aff ? `${aff.nom_dossier}${aff.numero_rg ? ' (' + aff.numero_rg + ')' : ''}` : 'Aucun dossier sélectionné';
    const noms = { 'view-dossier': 'Fiche dossier', 'view-agenda': 'Agenda', 'view-dashboard': 'Dossiers', 'view-cpc-express': 'Calcul des délais', 'view-prescription': 'Prescription', 'view-incidents': "Rédacteur d'actes", 'view-competence': 'Juridiction compétente', 'view-params': 'Paramètres', 'view-journal': "Journal d'activité" };
    $('chat-tool-context-label').textContent = noms[etat.vue] || 'Général';
    const anon = P().gemini_anonymize !== false;
    const lab = $('chat-anon-label');
    lab.textContent = anon ? 'Anonymisé' : 'Non anonymisé';
    lab.className = `text-[9px] font-semibold px-1.5 py-0.5 rounded border ${anon ? 'bg-emerald-50 text-emerald-800 border-emerald-200' : 'bg-rose-50 text-rose-800 border-rose-200'}`;
    $('chat-model-label').textContent = copiloteConfigure() ? `${nomFournisseur()} · ${modeleIa()}` : 'Non configuré';
}

/** Contexte de l'outil ouvert, transmis au copilote (anonymisé côté Rust). */
function contexteOutil() {
    if (etat.vue === 'view-prescription') return `Simulateur de prescription :\n${$('syllogisme-output').innerText}`;
    if (etat.vue === 'view-competence') return `Outil de compétence :\n${$('tree-verdict-details').innerText}`;
    if (etat.vue === 'view-incidents') return `Projet de dispositif :\n${$('dispositif-output').textContent}`;
    if (etat.vue === 'view-cpc-express' && etat.dernierExpress) return `Dernier calcul de délai :\n${etat.dernierExpress.resultat.justification_pas_a_pas}`;
    return null;
}

let copiloteOccupe = false;
async function envoyerCopilote(texte) {
    if (copiloteOccupe) return toast('Une réponse est déjà en cours.', 'alerte');
    ajouterMessage('user', texte);
    if (!copiloteConfigure()) {
        ajouterMessage('model', `**Clé API ${nomFournisseur()} non configurée.** Renseignez-la dans l'onglet **Paramètres**.`);
        return;
    }
    const aff = affaireActive();
    const attente = document.createElement('div');
    attente.className = 'flex gap-2';
    attente.innerHTML = `<div class="w-6 h-6 rounded-full bg-indigo-600 text-white flex items-center justify-center shrink-0 mt-0.5"><i data-lucide="loader-2" class="w-3.5 h-3.5 animate-spin"></i></div><div class="bg-white p-2.5 rounded-2xl rounded-tl-none border border-slate-200 text-slate-500 text-xs italic">Analyse en cours...</div>`;
    $('chat-messages-container').appendChild(attente);
    $('chat-messages-container').scrollTop = $('chat-messages-container').scrollHeight;
    icones();
    copiloteOccupe = true;
    try {
        const r = await appelRust('copilote_message', { historique: etat.chat, message: texte, idAffaire: aff ? aff.id : null, contexteOutil: contexteOutil() });
        etat.chat.push({ role: 'user', texte }, { role: 'model', texte: r.reponse });
        attente.remove();
        ajouterMessage('model', r.secours ? `${r.reponse}\n\n> ${t('Réponse fournie par le modèle de secours')} : ${r.modele}` : r.reponse);
    } catch (e) {
        attente.remove();
        ajouterMessage('model', `**Erreur :** ${e.message}`);
    } finally {
        copiloteOccupe = false;
    }
}

function ajouterMessage(role, texte) {
    const c = $('chat-messages-container');
    const div = document.createElement('div');
    div.className = 'flex gap-2';
    if (role === 'user') {
        div.innerHTML = `<div class="flex-1 flex justify-end"><div class="bg-indigo-600 text-white p-2.5 rounded-2xl rounded-tr-none text-xs max-w-[85%] leading-relaxed select-text">${esc(texte).replace(/\n/g, '<br>')}</div></div>`;
    } else {
        div.innerHTML = `<div class="w-6 h-6 rounded-full bg-gradient-to-tr from-indigo-600 to-sky-500 text-white flex items-center justify-center shrink-0 mt-0.5"><i data-lucide="bot" class="w-3.5 h-3.5"></i></div>
            <div class="flex-1 min-w-0 bg-white p-3 rounded-2xl rounded-tl-none border border-slate-200/90 text-slate-800 text-xs space-y-1.5 leading-relaxed">
                <div class="markdown-body select-text space-y-1 break-words">${formatMarkdown(texte)}</div>
                <div class="pt-1.5 border-t border-slate-100 flex justify-end gap-1.5">
                    <button type="button" data-a="word" class="px-2 py-0.5 text-[10px] font-semibold text-slate-500 hover:text-sky-700 rounded flex items-center gap-1"><i data-lucide="file-down" class="w-3 h-3 text-blue-600"></i> Word</button>
                    <button type="button" data-a="copie" class="px-2 py-0.5 text-[10px] font-semibold text-slate-500 hover:text-indigo-600 rounded flex items-center gap-1"><i data-lucide="copy" class="w-3 h-3"></i> Copier</button>
                </div>
            </div>`;
        const corps = div.querySelector('.markdown-body');
        div.querySelector('[data-a="copie"]').onclick = async () => { try { await navigator.clipboard.writeText(corps.innerText); toast('Réponse copiée.', 'ok'); } catch (e) { erreur(e); } };
        div.querySelector('[data-a="word"]').onclick = () => exporterWord(`Consultation_IA_${aujourdHui()}`, 'Projet - copilote de procédure civile', corps.innerHTML).catch(erreur);
    }
    c.appendChild(div);
    c.scrollTop = c.scrollHeight;
    icones();
}

function formatMarkdown(md) {
    let h = esc(md);
    h = h.replace(/^### (.*)$/gm, '<h5 class="font-bold text-xs mt-1.5 text-indigo-950">$1</h5>');
    h = h.replace(/^## (.*)$/gm, '<h4 class="font-bold text-xs mt-2 text-indigo-900">$1</h4>');
    h = h.replace(/^# (.*)$/gm, '<h4 class="font-bold text-sm mt-2 text-indigo-900">$1</h4>');
    h = h.replace(/\*\*(.+?)\*\*/g, '<strong class="font-bold text-slate-900">$1</strong>');
    h = h.replace(/^&gt; (.*)$/gm, '<blockquote class="border-l-2 border-indigo-500 pl-2 py-0.5 bg-indigo-50/50 text-indigo-950 text-[11px]">$1</blockquote>');
    h = h.replace(/^[-*] (.*)$/gm, '<li class="ml-3 list-disc text-slate-700">$1</li>');
    h = h.replace(/\n\n/g, '<div class="h-1"></div>').replace(/\n/g, '<br>');
    return lierArticles(h);
}


// -----------------------------------------------------------------------------
// 14 bis. PIÈCES ADVERSES : dépôt d'un PDF ou d'un texte, extraction locale, analyse par l'IA
//         + cas test de démonstration (réponse préparée, sans connexion).
// -----------------------------------------------------------------------------
const CAS_TEST_NOM = 'Assignation_Ateliers_Rhodaniens_c_Lambert.pdf';
const CAS_TEST_TEXTE = `SELARL MOREAU & ASSOCIÉS - Avocats au Barreau de Lyon
14, rue de la République - 69002 LYON

ASSIGNATION DEVANT LE TRIBUNAL JUDICIAIRE DE LYON
(procédure écrite ordinaire)

L'AN DEUX MILLE VINGT-SIX ET LE DOUZE NOVEMBRE

À LA REQUÊTE DE :
La société ATELIERS RHODANIENS, société par actions simplifiée au capital de 50 000 euros, immatriculée au RCS de Lyon sous le numéro 912 345 678, dont le siège social est situé 8, quai Perrache, 69002 Lyon, prise en la personne de son Président domicilié en cette qualité audit siège,

Ayant pour avocat constitué Maître Claire MOREAU, avocat au Barreau de Lyon, 14, rue de la République, 69002 Lyon, au cabinet de laquelle elle élit domicile,

J'AI, Maître Jean-Luc FAVRE, commissaire de justice associé à Lyon, soussigné,

DONNÉ ASSIGNATION À :
Monsieur Paul LAMBERT, né le 4 mars 1979 à Chambéry (73), de nationalité française, ingénieur, demeurant 12, rue des Alpes, 38000 Grenoble,
Où étant et parlant à : sa personne, ainsi déclaré.

D'AVOIR À COMPARAÎTRE devant le Tribunal judiciaire de Lyon, 67, rue Servient, 69003 Lyon, à une audience dont la date vous sera ultérieurement communiquée par le greffe.

TRÈS IMPORTANT : vous êtes tenu de constituer avocat pour être représenté devant ce tribunal. À défaut, vous vous exposez à ce qu'un jugement soit rendu contre vous sur les seuls éléments fournis par votre adversaire.

OBJET DE LA DEMANDE

I. - FAITS
Par devis accepté le 6 février 2026, Monsieur LAMBERT a confié à la société ATELIERS RHODANIENS la fabrication et la pose de menuiseries extérieures dans sa maison située 12, rue des Alpes à Grenoble, pour un prix de 18 400 euros TTC.
Les travaux ont été achevés et réceptionnés sans réserve le 15 mai 2026. La facture n° 2026-118 du 20 mai 2026, payable à trente jours, est demeurée impayée malgré une mise en demeure adressée par lettre recommandée le 1er juillet 2026.

II. - DISCUSSION
Aux termes de l'article 1103 du Code civil, les contrats légalement formés tiennent lieu de loi à ceux qui les ont faits. En application de l'article 1231-6 du même code, les sommes dues produisent intérêts au taux légal à compter de la mise en demeure.
Monsieur LAMBERT, qui a réceptionné les ouvrages sans réserve, est tenu d'en payer le prix.

PAR CES MOTIFS
Vu les articles 1103 et 1231-6 du Code civil,
Il est demandé au Tribunal judiciaire de Lyon de :
- CONDAMNER Monsieur Paul LAMBERT à payer à la société ATELIERS RHODANIENS la somme de 18 400 euros, avec intérêts au taux légal à compter du 1er juillet 2026 ;
- CONDAMNER Monsieur Paul LAMBERT à payer la somme de 2 000 euros au titre de l'article 700 du Code de procédure civile ;
- CONDAMNER Monsieur Paul LAMBERT aux entiers dépens.

PIÈCES : les pièces seront communiquées en cours de procédure.

Coût de l'acte : 68,42 euros.`;

/** Analyse préparée du cas test (vérifiée sur les art. 42, 46, 56, 74, 75, 114, 752, 763 CPC en vigueur). */
const CAS_TEST_ANALYSE = {
    en: {
        resume: "Writ of summons before the Judicial Court of Lyon (ordinary written procedure), served on 12 November 2026 at the request of SAS Ateliers Rhodaniens on Mr Paul Lambert, domiciled in Grenoble. Claim for payment of an unpaid joinery invoice of €18,400 for works carried out at the defendant's home in Grenoble.",
        vigilance: [
            "Lack of territorial jurisdiction: the defendant lives in Grenoble (art. 42 CPC) and the works were performed in Grenoble (art. 46 CPC). Nothing connects the dispute to Lyon. Raise the objection with reasons and name the competent court, the Judicial Court of Grenoble (art. 75 CPC).",
            "No place, day and time of the hearing: the writ only states that the date \"will be communicated later\". These particulars are required on pain of nullity (art. 56 1° CPC).",
            "No list of exhibits attached: \"exhibits will be disclosed during the proceedings\" breaches art. 56 3° CPC, which requires a list annexed to the writ.",
            "Legal representation is mandatory (claim above €10,000), yet the writ does not state the time limit within which the defendant must appoint counsel (art. 752 2° CPC).",
            "Strategy: raise all these procedural objections together and before any defence on the merits or plea of inadmissibility (art. 74 CPC). Nullity for a formal defect requires proof of prejudice (art. 114 CPC), e.g. the impossibility of organising the defence without knowing the hearing date.",
        ],
        dates: [
            { libelle: 'Service of the writ of summons', date: '2026-11-12' },
            { libelle: "Time limit to appoint counsel (15 days from the writ, art. 763 CPC)", date: '2026-11-27' },
            { libelle: 'Formal notice by registered letter', date: '2026-07-01' },
            { libelle: 'Acceptance and handover of the works without reservation', date: '2026-05-15' },
        ],
        pretentions: [
            { libelle: 'Unpaid invoice no. 2026-118 (principal), with statutory interest from 1 July 2026', montant: '€18,400' },
            { libelle: 'Article 700 CPC (legal costs)', montant: '€2,000' },
            { libelle: 'Costs of the proceedings (dépens)', montant: '' },
        ],
        fondements: ['Art. 1103 C. civ. (binding force of contracts)', 'Art. 1231-6 C. civ. (statutory interest)', 'Art. 700 CPC'],
    },
    fr: {
        resume: "Assignation devant le tribunal judiciaire de Lyon (procédure écrite ordinaire), signifiée le 12 novembre 2026 à la requête de la SAS Ateliers Rhodaniens à M. Paul Lambert, domicilié à Grenoble. Demande en paiement d'une facture de menuiseries de 18 400 € pour des travaux réalisés au domicile du défendeur, à Grenoble.",
        vigilance: [
            "Incompétence territoriale : le défendeur demeure à Grenoble (art. 42 CPC) et les travaux y ont été exécutés (art. 46 CPC). Aucun rattachement à Lyon : exception à motiver en désignant la juridiction compétente, le tribunal judiciaire de Grenoble (art. 75 CPC).",
            "Absence des lieu, jour et heure de l'audience : l'acte renvoie à une date « ultérieurement communiquée ». Mention prescrite à peine de nullité (art. 56 1° CPC).",
            "Absence de bordereau de pièces annexé : « les pièces seront communiquées en cours de procédure » méconnaît l'art. 56 3° CPC.",
            "Représentation obligatoire (demande supérieure à 10 000 €) : l'acte n'indique pas le délai dans lequel le défendeur est tenu de constituer avocat (art. 752 2° CPC).",
            "Stratégie : soulever ces exceptions simultanément et avant toute défense au fond ou fin de non-recevoir (art. 74 CPC) ; la nullité pour vice de forme suppose la preuve d'un grief (art. 114 CPC), par exemple l'impossibilité d'organiser la défense sans date d'audience.",
        ],
        dates: [
            { libelle: "Signification de l'assignation", date: '2026-11-12' },
            { libelle: "Délai de constitution d'avocat (15 jours à compter de l'assignation, art. 763 CPC)", date: '2026-11-27' },
            { libelle: 'Mise en demeure par lettre recommandée', date: '2026-07-01' },
            { libelle: 'Réception des travaux sans réserve', date: '2026-05-15' },
        ],
        pretentions: [
            { libelle: 'Facture impayée n° 2026-118 (principal), intérêts au taux légal à compter du 1er juillet 2026', montant: '18 400 €' },
            { libelle: 'Article 700 CPC', montant: '2 000 €' },
            { libelle: 'Dépens', montant: '' },
        ],
        fondements: ['Art. 1103 C. civ. (force obligatoire du contrat)', 'Art. 1231-6 C. civ. (intérêts moratoires)', 'Art. 700 CPC'],
    },
};

function remplirDossiersAnalyse() {
    const sel = $('analyse-dossier');
    const actuel = sel.value || (affaireActive() ? affaireActive().id : '');
    sel.innerHTML = `<option value="">${esc(t('Aucun dossier'))}</option>` + (etat.donnees.affaires || [])
        .filter(a => a.statut !== 'ARCHIVEE')
        .map(a => `<option value="${esc(a.id)}">${esc(a.nom_dossier)}</option>`).join('');
    sel.value = (etat.donnees.affaires || []).some(a => a.id === actuel) ? actuel : '';
}

function afficherPiece(nom, infos) {
    $('piece-chargee').classList.toggle('hidden', !nom);
    $('zone-depot').classList.toggle('hidden', !!nom);
    $('piece-nom').textContent = nom || '';
    $('piece-infos').textContent = infos || '';
}

function lireFichierBase64(fichier) {
    return new Promise((ok, ko) => {
        const r = new FileReader();
        r.onload = () => ok(String(r.result).split(',')[1] || '');
        r.onerror = () => ko(new Error(t('Fichier illisible.')));
        r.readAsDataURL(fichier);
    });
}

async function deposerPiece(fichier) {
    if (!fichier) return;
    etat.casTest = false;
    afficherPiece(fichier.name, t('Extraction du texte...'));
    try {
        const contenu = await lireFichierBase64(fichier);
        const r = await appelRust('extraire_texte_document', { nom: fichier.name, contenuBase64: contenu });
        $('adverse-act-raw-text').value = r.texte;
        afficherPiece(fichier.name, `${r.pages} ${t('page(s)')} · ${r.caracteres.toLocaleString(locale())} ${t('caractères extraits')}`);
    } catch (e) {
        afficherPiece('', '');
        toast(e.message, 'erreur', 9000);
    }
}

function chargerCasTest() {
    etat.casTest = true;
    $('adverse-act-raw-text').value = CAS_TEST_TEXTE;
    afficherPiece(CAS_TEST_NOM, t('Cas pratique de démonstration (fictif)'));
    toast(t('Cas test chargé. Cliquez sur « Analyser ».'), 'info');
}

function initAnalyse() {
    const zone = $('zone-depot');
    $('input-piece').onchange = (e) => { deposerPiece(e.target.files[0]); e.target.value = ''; };
    ['dragenter', 'dragover'].forEach(ev => zone.addEventListener(ev, (e) => { e.preventDefault(); zone.classList.add('survol'); }));
    ['dragleave', 'drop'].forEach(ev => zone.addEventListener(ev, (e) => { e.preventDefault(); zone.classList.remove('survol'); }));
    zone.addEventListener('drop', (e) => { const f = e.dataTransfer && e.dataTransfer.files[0]; if (f) deposerPiece(f); });
    // Un fichier lâché ailleurs dans la fenêtre ne doit pas remplacer l'application.
    window.addEventListener('dragover', (e) => e.preventDefault());
    window.addEventListener('drop', (e) => e.preventDefault());
    $('btn-retirer-piece').onclick = () => { etat.casTest = false; afficherPiece('', ''); $('adverse-act-raw-text').value = ''; };
    $('adverse-act-raw-text').addEventListener('input', () => { if ($('adverse-act-raw-text').value !== CAS_TEST_TEXTE) etat.casTest = false; });
    $('btn-cas-test').onclick = chargerCasTest;
    $('btn-extract-adverse-analysis').onclick = (e) => avecBouton(e.currentTarget, analyserActeAdverse);
    $('btn-copy-analyse').onclick = async () => { try { await navigator.clipboard.writeText(texteAnalyse()); toast(t('Analyse copiée.'), 'ok'); } catch (err) { erreur(err); } };
    $('btn-word-analyse').onclick = () => exporterWord(`${t('Analyse_piece_adverse')}_${aujourdHui()}`, t('Analyse de la pièce adverse'), enParagraphes(texteAnalyse())).catch(erreur);
}

function texteAnalyse() {
    const r = etat.derniereAnalyse;
    if (!r) return '';
    const ligne = (titre, liste) => liste.length ? `\n${titre}\n- ${liste.join('\n- ')}` : '';
    return [r.resume,
        ligne(t('Irrégularités et moyens de défense'), r.vigilance),
        ligne(t('Dates et délais'), r.dates.map(d => `${d.libelle} : ${parseIso(d.date) ? fmt(d.date) : d.date}`)),
        ligne(t('Demandes'), r.pretentions.map(p => p.libelle + (p.montant ? ` : ${p.montant}` : ''))),
        ligne(t('Fondements invoqués'), r.fondements)].join('\n').trim();
}

async function analyserActeAdverse() {
    const texte = $('adverse-act-raw-text').value.trim();
    if (!texte) return toast(t("Déposez une pièce ou collez son texte."), 'alerte');
    const idAffaire = $('analyse-dossier').value || null;
    const demo = etat.casTest && texte === CAS_TEST_TEXTE.trim();
    if (!demo && !copiloteConfigure()) return toast(`${t('Clé API')} ${nomFournisseur()} ${t('non configurée (Paramètres).')}`, 'alerte');
    $('analyse-vide').classList.add('hidden');
    $('adverse-analysis-results').classList.remove('hidden');
    $('analyse-actions').classList.add('hidden');
    $('analyse-resume').textContent = t('Analyse en cours...');
    ['adverse-dates-container', 'adverse-claims-container', 'adverse-grounds-container', 'adverse-flaws-container'].forEach(id => { $(id).innerHTML = ''; });
    $('analyse-meta').textContent = '';
    let r;
    try {
        if (demo) {
            await new Promise(ok => setTimeout(ok, 900));
            r = { ...CAS_TEST_ANALYSE[langue()], modele: t('Cas test : analyse préparée, sans connexion') };
        } else {
            r = await appelRust('analyser_acte_adverse', { texte, idAffaire });
        }
    } catch (e) {
        $('analyse-resume').textContent = '';
        $('adverse-flaws-container').innerHTML = `<p class="note-alerte" style="list-style:none;padding-left:1rem">${esc(e.message)}</p>`;
        return;
    }
    etat.derniereAnalyse = r;
    $('analyse-resume').textContent = r.resume || t('Synthèse non fournie.');
    $('adverse-flaws-container').innerHTML = r.vigilance.length
        ? r.vigilance.map((v, i) => `<div class="irregularite"><span class="irregularite-num">${i + 1}</span><p>${lierArticles(esc(v), 'CPC')}</p></div>`).join('')
        : `<p class="text-sm texte-3">${esc(t('Aucune irrégularité relevée.'))}</p>`;
    $('adverse-flaws-container').querySelectorAll('.btn-legal-article').forEach(b => b.classList.add('lien-article'));
    $('adverse-dates-container').innerHTML = r.dates.length ? '' : `<p class="text-sm texte-3">${esc(t('Aucune date détectée.'))}</p>`;
    r.dates.forEach(d => {
        const ligne = document.createElement('div');
        ligne.className = 'ligne-date';
        const valide = !!parseIso(d.date);
        ligne.innerHTML = `<span class="ligne-date-quand">${esc(valide ? fmt(d.date) : d.date || '?')}</span><span class="flex-1 min-w-0 text-sm texte-2">${lierArticles(esc(d.libelle), 'CPC')}</span>`;
        if (valide) {
            const b = document.createElement('button');
            b.type = 'button';
            b.className = 'btn btn-fantome btn-mini shrink-0';
            b.innerHTML = `<i data-lucide="calendar-plus" class="w-3.5 h-3.5"></i> ${esc(t('Échéance'))}`;
            b.title = t('Créer une échéance dans le dossier à partir de cette date');
            b.onclick = () => {
                if (idAffaire && idAffaire !== etat.selectedId) etat.selectedId = idAffaire;
                ouvrirModalActe({ nature: d.libelle, date: d.date });
            };
            ligne.appendChild(b);
        }
        $('adverse-dates-container').appendChild(ligne);
    });
    $('adverse-claims-container').innerHTML = r.pretentions.map(p => `<p>${esc(p.libelle)}${p.montant ? ` : <strong class="texte-1">${esc(p.montant)}</strong>` : ''}</p>`).join('') || `<p class="texte-3">${esc(t('Aucune demande chiffrée détectée.'))}</p>`;
    $('adverse-grounds-container').innerHTML = r.fondements.map(f => `<p>${lierArticles(esc(f), 'CCIV')}</p>`).join('') || `<p class="texte-3">${esc(t('Aucun fondement détecté.'))}</p>`;
    $('adverse-grounds-container').querySelectorAll('.btn-legal-article').forEach(b => b.classList.add('lien-article'));
    $('analyse-meta').textContent = `${r.modele ? r.modele + ' · ' : ''}${t("Analyse à contrôler sur l'acte original.")}`;
    $('analyse-actions').classList.remove('hidden');
    icones();
}

// -----------------------------------------------------------------------------
// 15. PARAMÈTRES, CONNECTEURS, SAUVEGARDE
// -----------------------------------------------------------------------------
function remplirParametres() {
    const p = P();
    $('param-nom-cabinet').value = p.nom_cabinet || '';
    $('param-seuil-rouge').value = p.seuil_rouge_jours ?? 3;
    $('param-seuil-orange').value = p.seuil_orange_jours ?? 7;
    $('param-gemini-key').value = p.gemini_api_key || '';
    $('param-gemini-model').value = p.gemini_model || 'gemini-2.5-flash';
    $('param-gemini-anonymize').checked = p.gemini_anonymize !== false;
    $('param-claude-key').value = p.claude_api_key || '';
    $('param-claude-model').value = p.claude_model || 'claude-opus-5-5';
    choisirFournisseurIa(p.ia_fournisseur === 'CLAUDE' ? 'CLAUDE' : 'GEMINI');
    majChoixLangue();
    $('param-openlegi-url').value = p.openlegi_url || '';
    $('param-openlegi-jeton').value = p.openlegi_jeton || '';
    $('param-resend-key').value = p.resend_api_key || '';
    $('param-resend-from').value = p.resend_expediteur || '';
    $('param-resend-emails').value = (p.emails_destinataires || []).join(', ');
    $('param-calendrier-defaut').value = p.calendrier_defaut || 'METROPOLE';
    $('param-alertes-auto').checked = p.alertes_auto !== false;
    $('param-alertes-jours').value = (p.alertes_jours || [7, 2]).join(', ');
    $('param-demarrage-windows').checked = !!p.demarrage_windows;
}

function lireParametres() {
    return {
        ...P(),
        nom_cabinet: $('param-nom-cabinet').value.trim(),
        seuil_rouge_jours: parseInt($('param-seuil-rouge').value, 10),
        seuil_orange_jours: parseInt($('param-seuil-orange').value, 10),
        gemini_api_key: $('param-gemini-key').value.trim(),
        gemini_model: $('param-gemini-model').value.trim() || 'gemini-2.5-flash',
        gemini_anonymize: $('param-gemini-anonymize').checked,
        ia_fournisseur: $('param-ia-fournisseur').value,
        claude_api_key: $('param-claude-key').value.trim(),
        claude_model: $('param-claude-model').value.trim() || 'claude-opus-5-5',
        openlegi_url: $('param-openlegi-url').value.trim(),
        openlegi_jeton: $('param-openlegi-jeton').value.trim(),
        resend_api_key: $('param-resend-key').value.trim(),
        resend_expediteur: $('param-resend-from').value.trim(),
        emails_destinataires: $('param-resend-emails').value.split(/[,;]/).map(s => s.trim()).filter(Boolean),
        calendrier_defaut: $('param-calendrier-defaut').value,
        alertes_auto: $('param-alertes-auto').checked,
        alertes_jours: $('param-alertes-jours').value.split(/[,;\s]+/).map(x => parseInt(x, 10)).filter(Number.isFinite),
    };
}

function initLangue() {
    document.querySelectorAll('#choix-langue [data-langue]').forEach(b => { b.onclick = () => changerLangue(b.dataset.langue).catch(erreur); });
}

function majChoixLangue() {
    document.querySelectorAll('#choix-langue [data-langue]').forEach(b => b.classList.toggle('actif', b.dataset.langue === langue()));
}

/** Bascule immédiate : le moteur renvoie ses textes dans la nouvelle langue, l'écran est retraduit. */
async function changerLangue(code) {
    if (code === langue()) return;
    const r = await appelRust('definir_langue', { code });
    const [regles, ref] = await Promise.all([appelRust('lister_regles_delais'), appelRust('lister_referentiels')]);
    const choixExpress = $('exp-delai-select').value, choixActe = $('modal-act-delai').value, choixRegime = $('sim-regime').value, choixMatiere = $('tree-nat').value;
    etat.regles = regles;
    etat.ref = ref;
    I18N.appliquer(code);
    ecrirePreference('lexprocedura_langue', code);
    remplirSelectRegles($('exp-delai-select'), false);
    remplirSelectRegles($('modal-act-delai'), true);
    remplirReferentiels();
    $('exp-delai-select').value = choixExpress; $('modal-act-delai').value = choixActe; $('sim-regime').value = choixRegime; $('tree-nat').value = choixMatiere;
    appliquer(r);
    majChoixLangue();
    majBoutonTheme();
    renderAiWelcomeMessage();
    planifierExpress();
    calculerRetro().catch(() => {});
    computePrescription();
    etat.cleCompetence = null;
    recomputeCompetence();
    recomputeDispositif();
    renderInfosStockage();
    toast(code === 'en' ? 'Interface in English.' : 'Interface en français.', 'ok');
}

function choisirFournisseurIa(code) {
    $('param-ia-fournisseur').value = code;
    document.querySelectorAll('#choix-fournisseur-ia [data-fournisseur]').forEach(b => b.classList.toggle('actif', b.dataset.fournisseur === code));
    $('ia-bloc-gemini').classList.toggle('hidden', code !== 'GEMINI');
    $('ia-bloc-claude').classList.toggle('hidden', code !== 'CLAUDE');
}

function renderInfosStockage() {
    const i = etat.infos;
    if (!i) return;
    $('storage-info').innerHTML = `<div><span class="texte-3">${esc(t('Base'))}</span> ${esc(i.chemin_donnees)}</div><div><span class="texte-3">${esc(t('Sauvegardes'))}</span> ${esc(i.dossier_sauvegardes)}</div><div><span class="texte-3">${esc(t('Exports'))}</span> ${esc(i.dossier_exports)}</div><div><span class="texte-3">${esc(t('Clés API'))}</span> ${esc(i.cles_dans_coffre ? (SUR_MAC ? t('Trousseau macOS') : t("Gestionnaire d'identification Windows")) : (SUR_MAC ? t('fichier de données (Trousseau macOS indisponible)') : t('fichier de données (coffre Windows indisponible)')))}</div>`;
}

function statut(id, texte, type) {
    const el = $(id);
    el.textContent = texte;
    el.className = `text-xs font-bold ${type === 'ok' ? 'text-emerald-600' : type === 'erreur' ? 'text-rose-600' : 'text-sky-600'}`;
}

function initParametres() {
    $('btn-recalc-global').onclick = (e) => avecBouton(e.currentTarget, () => proposerRecalcul(null, true));
    $('btn-save-settings').onclick = (e) => avecBouton(e.currentTarget, async () => {
        const p = lireParametres();
        if (!Number.isFinite(p.seuil_rouge_jours) || !Number.isFinite(p.seuil_orange_jours)) return toast('Seuils invalides.', 'alerte');
        const calendrierAvant = P().calendrier_defaut;
        const r = await appelRust('enregistrer_parametres', { nouveauxParametres: p });
        appliquer(r);
        remplirParametres();
        renderAiWelcomeMessage();
        $('btn-refresh-legi').classList.toggle('hidden', !openlegiConfigure());
        $('exp-calendrier-select').value = P().calendrier_defaut || 'METROPOLE';
        toast('Paramètres enregistrés.', 'ok');
        if (calendrierAvant !== P().calendrier_defaut) await proposerRecalcul(null);
    });

    $('btn-test-openlegi').onclick = (e) => avecBouton(e.currentTarget, async () => {
        statut('openlegi-test-status', "Lecture de l'art. 641 CPC via OpenLegi...", 'info');
        try {
            const msg = await appelRust('tester_openlegi', { url: $('param-openlegi-url').value.trim() || null, jeton: $('param-openlegi-jeton').value.trim() || null });
            statut('openlegi-test-status', msg + ' Pensez à enregistrer.', 'ok');
        } catch (err) { statut('openlegi-test-status', err.message, 'erreur'); }
    });

    document.querySelectorAll('#choix-fournisseur-ia [data-fournisseur]').forEach(b => {
        b.onclick = () => { choisirFournisseurIa(b.dataset.fournisseur); statut('ia-test-status', '', 'info'); };
    });
    $('btn-test-ia').onclick = (e) => avecBouton(e.currentTarget, async () => {
        const claude = $('param-ia-fournisseur').value === 'CLAUDE';
        const cle = $(claude ? 'param-claude-key' : 'param-gemini-key').value.trim();
        const modele = $(claude ? 'param-claude-model' : 'param-gemini-model').value.trim() || (claude ? 'claude-opus-5-5' : 'gemini-2.5-flash');
        if (!cle) return statut('ia-test-status', 'Saisissez une clé API.', 'erreur');
        statut('ia-test-status', 'Test en cours...', 'info');
        try { statut('ia-test-status', (await appelRust(claude ? 'tester_claude' : 'tester_gemini', { cle, modele })) + ' Pensez à enregistrer.', 'ok'); }
        catch (err) { statut('ia-test-status', err.message, 'erreur'); }
    });

    $('btn-verifier-alertes').onclick = (e) => avecBouton(e.currentTarget, async () => {
        statut('alertes-status', 'Vérification en cours...', 'info');
        try {
            const r = await appelRust('verifier_alertes');
            if (r.donnees) appliquer({ donnees: r.donnees });
            statut('alertes-status', r.message, 'ok');
        } catch (err) { statut('alertes-status', err.message, 'erreur'); }
    });
    $('param-demarrage-windows').onchange = async (e) => {
        const actif = e.currentTarget.checked;
        statut('demarrage-status', '', 'info');
        try {
            const r = await appelRust('definir_demarrage_windows', { actif });
            appliquer(r);
            statut('demarrage-status', r.message || '', 'ok');
        } catch (err) {
            $('param-demarrage-windows').checked = !actif;
            statut('demarrage-status', err.message, 'erreur');
        }
    };
    $('btn-test-email').onclick = (e) => avecBouton(e.currentTarget, async () => {
        statut('email-test-status', 'Envoi en cours...', 'info');
        try { statut('email-test-status', await appelRust('declencher_test_email'), 'ok'); }
        catch (err) { statut('email-test-status', err.message, 'erreur'); }
        etat.donnees = await appelRust('charger_donnees');
    });

    $('btn-export-backup-json').onclick = (e) => avecBouton(e.currentTarget, async () => {
        const chemin = await appelRust('exporter_sauvegarde');
        toast(`Sauvegarde créée : ${chemin}`, 'ok', 8000);
    });

    $('input-import-backup-json').onchange = (e) => {
        const f = e.target.files[0];
        e.target.value = '';
        if (!f) return;
        const lecteur = new FileReader();
        lecteur.onload = async () => {
            const conserver = $('chk-import-keep-params').checked;
            const ok = await uiConfirm('Restaurer une sauvegarde', `Remplacer les dossiers actuels par le contenu de « ${f.name} » ?\n\nUne sauvegarde horodatée de la base actuelle est créée au préalable. Le journal d'audit est conservé.${conserver ? '\nVos paramètres et clés actuels sont conservés.' : '\nLes paramètres et clés de la sauvegarde remplaceront les vôtres.'}`, 'Restaurer');
            if (!ok) return;
            try {
                const r = await appelRust('importer_sauvegarde', { contenu: String(lecteur.result), conserverParametres: conserver });
                etat.selectedId = null;
                appliquer(r); remplirParametres();
                toast(r.message || 'Sauvegarde restaurée.', 'ok');
            } catch (err) { erreur(err); }
        };
        lecteur.onerror = () => toast('Lecture du fichier impossible.', 'erreur');
        lecteur.readAsText(f, 'utf-8');
    };

    $('btn-reset-base').onclick = async () => {
        if (!await uiConfirm('Réinitialiser la base', 'Effacer tous les dossiers, paramètres et le journal de ce poste ?\n\nUne sauvegarde horodatée complète est créée au préalable dans le dossier Sauvegardes.', 'Continuer')) return;
        if (!await uiConfirm('Confirmation finale', 'Cette opération vide la base du cabinet. Confirmez-vous la réinitialisation ?', 'Réinitialiser')) return;
        try { const r = await appelRust('reinitialiser_base'); etat.selectedId = null; appliquer(r); remplirParametres(); toast(r.message, 'ok', 9000); } catch (err) { erreur(err); }
    };
}

// -----------------------------------------------------------------------------
// 16. JOURNAL D'AUDIT
// -----------------------------------------------------------------------------
function initJournal() {
    $('journal-filter-cat').onchange = renderJournal;
    $('journal-filter-text').oninput = renderJournal;
    $('btn-export-journal-pdf').onclick = (e) => avecBouton(e.currentTarget, async () => {
        const chemin = await appelRust('exporter_journal_pdf', { categorie: $('journal-filter-cat').value || null, recherche: $('journal-filter-text').value.trim() || null, idAffaire: null, ouvrir: true });
        toast(`Journal exporté en PDF : ${chemin}`, 'ok', 8000);
    });
    $('btn-journal-dossier-pdf').onclick = (e) => avecBouton(e.currentTarget, async () => {
        const a = affaireActive();
        if (!a) return;
        const chemin = await appelRust('exporter_journal_pdf', { categorie: null, recherche: null, idAffaire: a.id, ouvrir: true });
        toast(`Journal du dossier exporté en PDF : ${chemin}`, 'ok', 8000);
    });
    $('btn-export-journal').onclick = (e) => avecBouton(e.currentTarget, async () => {
        const chemin = await appelRust('exporter_journal');
        toast(`Journal exporté : ${chemin}`, 'ok', 8000);
    });
}

function renderJournal() {
    const cat = $('journal-filter-cat').value;
    const q = $('journal-filter-text').value.toLowerCase().trim();
    const noms = Object.fromEntries((etat.donnees.affaires || []).map(a => [a.id, a.nom_dossier]));
    const entrees = (etat.donnees.historique || []).filter(e => (!cat || e.categorie === cat) && (!q || [e.titre, e.details, e.fondement, noms[e.id_affaire]].join(' ').toLowerCase().includes(q)));
    $('journal-list').innerHTML = entrees.length ? entrees.slice(0, 500).map(e => ligneJournal(e, e.id_affaire ? noms[e.id_affaire] : null)).join('')
        + (entrees.length > 500 ? `<p class="px-6 py-3 text-xs texte-3">${entrees.length - 500} entrée(s) plus ancienne(s) : utilisez l'export.</p>` : '')
        : '<p class="etat-vide" style="min-height:10rem">Aucune entrée.</p>';
}

// -----------------------------------------------------------------------------
// 17. PALETTE DE RECHERCHE (CTRL + F / CTRL + K)
// -----------------------------------------------------------------------------
function initPalette() {
    const ouvrir = () => {
        ouvrirModal('palette-modal');
        $('palette-search-input').value = '';
        rechercherPalette('');
        $('palette-search-input').focus();
    };
    window.addEventListener('keydown', (e) => {
        if ((e.ctrlKey || e.metaKey) && ['f', 'k'].includes(e.key.toLowerCase())) { e.preventDefault(); ouvrir(); }
        if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'n' && !document.querySelector('.fixed.flex[id^="modal-"]')) { e.preventDefault(); ouvrirModalAffaire(null); }
    });
    $('btn-quick-search-trigger').onclick = ouvrir;
    $('palette-modal').onclick = (e) => { if (e.target.id === 'palette-modal') fermerModal('palette-modal'); };
    $('palette-search-input').oninput = (e) => rechercherPalette(e.target.value);
    $('palette-search-input').onkeydown = (e) => {
        const items = $('palette-items-list').querySelectorAll('[data-idx]');
        if (!items.length) return;
        if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
            e.preventDefault();
            etat.paletteIndex = (etat.paletteIndex + (e.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length;
            items.forEach((it, i) => it.classList.toggle('bg-slate-100', i === etat.paletteIndex));
        } else if (e.key === 'Enter') { e.preventDefault(); items[etat.paletteIndex]?.click(); }
    };
}

function rechercherPalette(q) {
    q = q.toLowerCase().trim();
    const list = $('palette-items-list');
    list.innerHTML = '';
    etat.paletteIndex = 0;
    const resultats = [];
    (etat.donnees.affaires || []).forEach(a => {
        const texte = [a.nom_dossier, a.reference_cabinet, a.numero_rg, a.juridiction, a.client, a.partie_adverse].join(' ').toLowerCase();
        if (!q || texte.includes(q)) resultats.push({ titre: a.nom_dossier, sous: [a.reference_cabinet, a.numero_rg].filter(Boolean).join(' - '), id: a.id });
        (a.actes || []).forEach(x => { if (q && String(x.nature).toLowerCase().includes(q)) resultats.push({ titre: `${x.nature} (${fmt(x.date_echeance_calculee)})`, sous: a.nom_dossier, id: a.id }); });
    });
    if (!resultats.length) { list.innerHTML = `<p class="p-2 text-slate-400">${(etat.donnees.affaires || []).length ? 'Aucun résultat.' : 'Aucun dossier dans la base.'}</p>`; return; }
    resultats.slice(0, 40).forEach((r, i) => {
        const d = document.createElement('div');
        d.dataset.idx = i;
        d.className = `p-2 hover:bg-slate-100 rounded cursor-pointer flex justify-between gap-2 ${i === 0 ? 'bg-slate-100' : ''}`;
        d.innerHTML = `<span class="truncate">${esc(r.titre)}</span><span class="text-slate-400 font-mono truncate">${esc(r.sous)}</span>`;
        d.onclick = () => { fermerModal('palette-modal'); ouvrirDossier(r.id); };
        list.appendChild(d);
    });
}

// -----------------------------------------------------------------------------
// 18. AGENDA DES ÉCHÉANCES (tous dossiers) : vue mensuelle et liste
// -----------------------------------------------------------------------------
const MOIS_FR = ['janvier', 'février', 'mars', 'avril', 'mai', 'juin', 'juillet', 'août', 'septembre', 'octobre', 'novembre', 'décembre'];

function initAgenda() {
    const auj = parseIso(aujourdHui());
    etat.agenda = { annee: auj.getFullYear(), mois: auj.getMonth(), jour: aujourdHui(), mode: lirePreference('lexprocedura_agenda_mode', 'mois') };
    const changerMois = (delta) => {
        const d = new Date(etat.agenda.annee, etat.agenda.mois + delta, 1);
        etat.agenda.annee = d.getFullYear(); etat.agenda.mois = d.getMonth();
        renderAgenda();
    };
    $('btn-agenda-prec').onclick = () => changerMois(-1);
    $('btn-agenda-suiv').onclick = () => changerMois(1);
    $('btn-agenda-aujourdhui').onclick = () => {
        const a = parseIso(aujourdHui());
        Object.assign(etat.agenda, { annee: a.getFullYear(), mois: a.getMonth(), jour: aujourdHui() });
        renderAgenda();
    };
    const choisirMode = (mode) => { etat.agenda.mode = mode; ecrirePreference('lexprocedura_agenda_mode', mode); renderAgenda(); };
    $('btn-agenda-mois').onclick = () => choisirMode('mois');
    $('btn-agenda-liste').onclick = () => choisirMode('liste');
    $('agenda-afficher-accomplies').onchange = renderAgenda;
    $('agenda-afficher-archives').onchange = renderAgenda;
    $('agenda-grille').addEventListener('click', (e) => {
        const puce = e.target.closest('[data-acte]');
        const jour = e.target.closest('[data-jour]');
        if (!jour) return;
        etat.agenda.jour = jour.dataset.jour;
        const d = parseIso(jour.dataset.jour);
        if (d.getMonth() !== etat.agenda.mois) { etat.agenda.annee = d.getFullYear(); etat.agenda.mois = d.getMonth(); }
        renderAgenda();
        if (puce) ouvrirEcheanceAgenda(puce.dataset.affaire);
    });
    $('agenda-detail-jour').addEventListener('click', (e) => {
        const b = e.target.closest('[data-affaire]');
        if (b) ouvrirEcheanceAgenda(b.dataset.affaire);
    });
    $('btn-export-agenda-ics').onclick = () => {
        const items = elementsAgenda().filter(x => x.acte.statut !== 'ACCOMPLI' && x.j >= 0);
        if (!items.length) return toast('Aucune échéance à venir à exporter.', 'alerte');
        exporterIcs(`Agenda_DeadlineBreaker_${aujourdHui()}`, items.map(x => evenementIcs(x.aff, x.acte))).catch(erreur);
    };
}

function ouvrirEcheanceAgenda(idAffaire) { if (idAffaire) ouvrirDossier(idAffaire, 'echeances'); }

function elementsAgenda() {
    const accomplies = $('agenda-afficher-accomplies').checked;
    const archives = $('agenda-afficher-archives').checked;
    const items = [];
    (etat.donnees.affaires || []).forEach(aff => {
        if (aff.statut === 'ARCHIVEE' && !archives) return;
        (aff.actes || []).forEach(acte => {
            if (!acte.date_echeance_calculee) return;
            if (acte.statut === 'ACCOMPLI' && !accomplies) return;
            items.push({ aff, acte, j: joursJusqua(acte.date_echeance_calculee) });
        });
    });
    return items.sort((a, b) => a.acte.date_echeance_calculee.localeCompare(b.acte.date_echeance_calculee));
}

function classePuce(x) {
    if (x.acte.statut === 'ACCOMPLI') return 'puce-fait';
    if (x.j <= (P().seuil_rouge_jours ?? 3)) return 'puce-danger';
    if (x.j <= (P().seuil_orange_jours ?? 7)) return 'puce-alerte';
    return 'puce-normal';
}

function renderAgenda() {
    if (!etat.agenda) return;
    const mode = etat.agenda.mode;
    $('btn-agenda-mois').classList.toggle('actif', mode === 'mois');
    $('btn-agenda-liste').classList.toggle('actif', mode === 'liste');
    $('agenda-vue-mois').classList.toggle('hidden', mode !== 'mois');
    $('agenda-list').classList.toggle('hidden', mode !== 'liste');
    ['btn-agenda-prec', 'btn-agenda-suiv', 'btn-agenda-aujourdhui'].forEach(id => $(id).classList.toggle('invisible', mode !== 'mois'));
    $('agenda-titre-mois').textContent = mode === 'mois' ? new Date(etat.agenda.annee, etat.agenda.mois, 1).toLocaleDateString(locale(), { month: 'long', year: 'numeric' }) : t('Toutes les échéances');
    const items = elementsAgenda();
    if (mode === 'mois') renderAgendaMois(items); else renderAgendaListe(items);
    icones();
}

function renderAgendaMois(items) {
    const { annee, mois } = etat.agenda;
    const parJour = {};
    items.forEach(x => { (parJour[x.acte.date_echeance_calculee] = parJour[x.acte.date_echeance_calculee] || []).push(x); });
    const premier = new Date(annee, mois, 1);
    const decalage = (premier.getDay() + 6) % 7; // lundi en premier
    const debut = new Date(annee, mois, 1 - decalage);
    const nbSemaines = Math.ceil((decalage + dernierJourDuMois(annee, mois)) / 7);
    const auj = aujourdHui();
    let html = '';
    for (let i = 0; i < nbSemaines * 7; i++) {
        const d = new Date(debut.getFullYear(), debut.getMonth(), debut.getDate() + i);
        const iso = isoLocal(d);
        const liste = parJour[iso] || [];
        const classes = ['agenda-jour'];
        if (d.getMonth() !== mois) classes.push('hors-mois');
        if (d.getDay() === 0 || d.getDay() === 6) classes.push('week-end');
        if (iso === auj) classes.push('aujourdhui');
        if (iso === etat.agenda.jour) classes.push('selection');
        const visibles = liste.slice(0, 3);
        html += `<div role="button" tabindex="0" class="${classes.join(' ')}" data-jour="${iso}" title="${liste.length ? liste.length + ' échéance(s)' : ''}">
            <span class="agenda-num">${d.getDate()}</span>
            ${visibles.map(x => `<span class="agenda-puce ${classePuce(x)}" data-acte="${esc(x.acte.id)}" data-affaire="${esc(x.aff.id)}">${esc(x.acte.nature)}</span>`).join('')}
            ${liste.length > 3 ? `<span class="agenda-plus">+ ${liste.length - 3} autre${liste.length - 3 > 1 ? 's' : ''}</span>` : ''}
        </div>`;
    }
    $('agenda-grille').innerHTML = html;
    renderDetailJour(parJour[etat.agenda.jour] || []);
}

function renderDetailJour(liste) {
    const jour = etat.agenda.jour;
    const j = joursJusqua(jour);
    const relatif = j === 0 ? "Aujourd'hui" : j === 1 ? 'Demain' : j === -1 ? 'Hier' : j > 0 ? `Dans ${j} jours` : `Il y a ${-j} jours`;
    $('agenda-detail-jour').innerHTML = `
        <p class="text-xs font-medium texte-3">${esc(relatif)}</p>
        <h2 class="text-base font-semibold texte-1 mt-0.5 first-letter:uppercase">${esc(dateEnLettres(jour))}</h2>
        <div class="mt-4 space-y-2">${liste.length ? liste.map(x => `
            <button type="button" class="detail-echeance" data-affaire="${esc(x.aff.id)}">
                <span class="agenda-puce ${classePuce(x)} mb-2" style="display:inline-block;width:auto">${x.acte.statut === 'ACCOMPLI' ? 'Accomplie' : esc(libelleJours(x.j))}</span>
                <span class="block text-sm font-semibold texte-1">${esc(x.acte.nature)}</span>
                <span class="block text-xs texte-2 mt-0.5">${esc(x.aff.nom_dossier)}${x.aff.juridiction ? ' · ' + esc(x.aff.juridiction) : ''}</span>
                ${x.acte.sanction ? `<span class="block text-xs texte-3 mt-1">Sanction : ${esc(x.acte.sanction)}</span>` : ''}
            </button>`).join('') : '<p class="text-sm texte-3">Aucune échéance ce jour.</p>'}</div>`;
}

function renderAgendaListe(items) {
    const groupes = [
        { titre: 'En retard', filtre: x => x.j < 0 && x.acte.statut !== 'ACCOMPLI' },
        { titre: "Aujourd'hui", filtre: x => x.j === 0 && x.acte.statut !== 'ACCOMPLI' },
        { titre: '7 prochains jours', filtre: x => x.j >= 1 && x.j <= 7 && x.acte.statut !== 'ACCOMPLI' },
        { titre: '30 prochains jours', filtre: x => x.j >= 8 && x.j <= 30 && x.acte.statut !== 'ACCOMPLI' },
        { titre: 'Plus tard', filtre: x => x.j > 30 && x.acte.statut !== 'ACCOMPLI' },
        { titre: 'Accomplies', filtre: x => x.acte.statut === 'ACCOMPLI' },
    ];
    const box = $('agenda-list');
    if (!items.length) {
        box.innerHTML = `<div class="carte etat-vide"><i data-lucide="calendar-check" class="w-8 h-8"></i><p>Aucune échéance enregistrée. Ajoutez des actes depuis la fiche d'un dossier.</p></div>`;
        return;
    }
    box.innerHTML = groupes.map(g => {
        const lignes = items.filter(g.filtre);
        if (!lignes.length) return '';
        return `<section class="carte overflow-hidden">
            <h3 class="px-5 py-3 text-sm font-semibold texte-1" style="border-bottom:1px solid var(--bord)">${esc(g.titre)} <span class="texte-3 font-normal">${lignes.length}</span></h3>
            <div>${lignes.map(x => `
                <button type="button" data-affaire="${esc(x.aff.id)}" class="w-full text-left px-5 py-3 flex items-center gap-4 hover:bg-[var(--survol)]" style="border-top:1px solid var(--bord)">
                    <span class="w-28 shrink-0 text-sm font-medium texte-1">${fmt(x.acte.date_echeance_calculee)}</span>
                    <span class="agenda-puce ${classePuce(x)} shrink-0" style="width:7.5rem">${x.acte.statut === 'ACCOMPLI' ? 'Accomplie' : esc(libelleJours(x.j))}</span>
                    <span class="min-w-0 flex-1">
                        <span class="block text-sm font-medium texte-1 truncate">${esc(x.acte.nature)}</span>
                        <span class="block text-xs texte-2 truncate">${esc(x.aff.nom_dossier)}${x.aff.juridiction ? ' · ' + esc(x.aff.juridiction) : ''}</span>
                    </span>
                    <span class="hidden lg:block w-64 shrink-0 text-xs texte-3 truncate" title="${esc(x.acte.sanction || '')}">${esc(x.acte.sanction || '')}</span>
                </button>`).join('')}</div></section>`;
    }).join('');
    box.querySelectorAll('[data-affaire]').forEach(b => { b.onclick = () => ouvrirEcheanceAgenda(b.dataset.affaire); });
}

})(); // Portée isolée : aucun nom global (Tauri/wry réserve notamment window.ipc).
