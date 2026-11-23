#!/bin/bash
# =============================================================================
#  Deadline Breaker - fabrication de l'installateur Mac (.dmg)
#  À lancer SUR UN MAC, dans le Terminal, depuis le dossier du projet :
#      bash construire_mac.sh
#  Le script installe au besoin les outils (outils de développement Apple, Rust,
#  outil Tauri), puis fabrique un .dmg unique pour les Mac Intel et Apple Silicon.
#  Résultat : dossier « Installateur_Mac » (le .dmg et la notice pour vos collègues).
#  Première fois : 20 à 40 minutes. Les fois suivantes : 5 à 10 minutes.
# =============================================================================
set -u

ROUGE=$'\033[31m'; VERT=$'\033[32m'; BLEU=$'\033[36m'; JAUNE=$'\033[33m'; FIN=$'\033[0m'
etape()  { echo ""; echo "${BLEU}== $1${FIN}"; }
ok()     { echo "${VERT}   $1${FIN}"; }
info()   { echo "   $1"; }
erreur() { echo ""; echo "${ROUGE}ERREUR : $1${FIN}"; [ -n "${2:-}" ] && echo "${ROUGE}$2${FIN}"; exit 1; }

# --- 0. Contrôles préalables ---------------------------------------------------
[ "$(uname -s)" = "Darwin" ] || erreur "ce script se lance sur un Mac (macOS)." "Sous Windows, utilisez le script PowerShell du lot avec l'option -Construire."

RACINE="$(cd "$(dirname "$0")" && pwd)"
cd "$RACINE" || erreur "dossier du projet inaccessible : $RACINE"
[ -f "src-tauri/Cargo.toml" ] && [ -f "src-tauri/tauri.macos.conf.json" ] && [ -d "ui" ] \
  || erreur "lancez ce script depuis le dossier du projet (celui qui contient src-tauri et ui)." "Dossier actuel : $RACINE"

echo "${BLEU}=== Deadline Breaker - installateur Mac ===${FIN}"
info "Projet : $RACINE"
info "macOS $(sw_vers -productVersion 2>/dev/null) - processeur $(uname -m)"

# --- 1. Outils de développement Apple (compilateur, codesign) -------------------
etape "1/4 Outils de développement Apple (Command Line Tools)"
if xcode-select -p >/dev/null 2>&1 && xcrun --find clang >/dev/null 2>&1; then
  ok "déjà installés ($(xcode-select -p))"
else
  info "Installation demandée à macOS : une fenêtre va s'ouvrir."
  xcode-select --install >/dev/null 2>&1 || true
  echo ""
  echo "${JAUNE}   Cliquez sur « Installer » dans la fenêtre, acceptez la licence et attendez la fin"
  echo "   (10 à 20 minutes). Relancez ensuite :  bash construire_mac.sh${FIN}"
  exit 0
fi

# --- 2. Rust (compilateur du moteur) --------------------------------------------
etape "2/4 Rust"
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
if ! command -v rustup >/dev/null 2>&1; then
  info "Installation de Rust (quelques minutes, connexion Internet nécessaire)..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal \
    || erreur "installation de Rust impossible." "Vérifiez la connexion Internet puis relancez le script."
  . "$HOME/.cargo/env"
fi
command -v cargo >/dev/null 2>&1 || erreur "Rust est installé mais « cargo » reste introuvable." "Fermez le Terminal, rouvrez-le, puis relancez le script."
ok "$(rustc --version)"
info "Ajout des cibles Intel et Apple Silicon..."
rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null \
  || erreur "ajout des cibles Mac impossible." "Vérifiez la connexion Internet puis relancez le script."
ok "cibles aarch64-apple-darwin et x86_64-apple-darwin prêtes"

# --- 3. Outil Tauri (fabrique l'application et le .dmg) ---------------------------
etape "3/4 Outil Tauri"
if cargo tauri --version 2>/dev/null | grep -q "tauri-cli 2\."; then
  ok "$(cargo tauri --version)"
else
  info "Installation de l'outil Tauri (une seule fois, 5 à 15 minutes)..."
  cargo install tauri-cli --version '^2' --locked \
    || erreur "installation de l'outil Tauri impossible." "Vérifiez la connexion Internet puis relancez le script."
  ok "$(cargo tauri --version)"
fi

# --- 4. Fabrication ----------------------------------------------------------------
etape "4/4 Compilation optimisée et création du .dmg (Intel + Apple Silicon)"
info "Patientez : la première compilation est longue ; les avertissements jaunes sont normaux."
cd "$RACINE/src-tauri" || exit 1
# CI=true : .dmg créé sans piloter le Finder (aucune autorisation « Terminal souhaite
# contrôler Finder » à accorder ; seule la disposition des icônes est celle par défaut).
CI=true cargo tauri build --target universal-apple-darwin
code=$?
cd "$RACINE" || exit 1
[ $code -eq 0 ] || erreur "la fabrication a échoué (code $code)." "Envoyez les dernières lignes rouges affichées ci-dessus (sans aucune clé API)."

DOSSIER_DMG="src-tauri/target/universal-apple-darwin/release/bundle/dmg"
DMG="$(ls -t "$DOSSIER_DMG"/*.dmg 2>/dev/null | head -n 1)"
[ -n "$DMG" ] || erreur "fabrication terminée, mais aucun .dmg trouvé dans $DOSSIER_DMG."

SORTIE="$RACINE/Installateur_Mac"
mkdir -p "$SORTIE"
cp -f "$DMG" "$SORTIE/" || erreur "copie du .dmg impossible vers $SORTIE."
[ -f "outils/mac/Notice_installation_Mac.html" ] && cp -f "outils/mac/Notice_installation_Mac.html" "$SORTIE/"

echo ""
ok "Installateur prêt : $SORTIE/$(basename "$DMG")"
info "À transmettre à vos collègues : le .dmg ET « Notice_installation_Mac.html »."
info "Pour l'installer sur ce Mac : double-cliquez sur le .dmg et glissez Deadline Breaker dans Applications."
open "$SORTIE" >/dev/null 2>&1 || true
