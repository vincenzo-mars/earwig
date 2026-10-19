#!/usr/bin/env bash
# Porta il Mac allo stato di un utente che installa Earwig per la prima volta:
# azzera ogni traccia dell'app (impostazioni, modello, cache, webview, permessi
# OS, registrazioni), compila il bundle release, lo installa in /Applications e
# lo lancia.
#
# Uso: bash scripts/fresh-install.sh
#
# DISTRUTTIVO: cancella il modello Whisper (~1.5 GB, va riscaricato dall'app) e
# TUTTE le registrazioni in ~/Documents/Earwig/Records.
#
# Richiede: Rust (~/.cargo), Node, e src-tauri/binaries/whisper-server già
# compilato (se manca: bash scripts/build-whisper-server.sh).

set -euo pipefail

IDENTIFIER="com.vincenzomars.earwig"
APP_NAME="Earwig"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RECORDS_DIR="$HOME/Documents/Earwig/Records"
INSTALLED_APP="/Applications/$APP_NAME.app"
BUILT_APP="$ROOT_DIR/src-tauri/target/release/bundle/macos/$APP_NAME.app"

step() { printf '\n\033[1m==> %s\033[0m\n' "$1"; }
warn() { printf '\033[33m    %s\033[0m\n' "$1"; }

# ── 1. Preflight ──────────────────────────────────────────────────────────────

step "Preflight"

if [ ! -x "$ROOT_DIR/src-tauri/binaries/whisper-server" ]; then
  echo "ERRORE: manca src-tauri/binaries/whisper-server." >&2
  echo "Il bundle fallirebbe con 'resource path doesn't exist' dopo aver compilato tutto." >&2
  echo "Compilalo prima: bash scripts/build-whisper-server.sh" >&2
  exit 1
fi
echo "    whisper-server presente"

# shellcheck disable=SC1090
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
command -v cargo >/dev/null || { echo "ERRORE: cargo non nel PATH." >&2; exit 1; }
echo "    cargo $(cargo --version | awk '{print $2}')"

# ── 2. Chiusura di quello che è vivo ──────────────────────────────────────────

step "Chiudo l'app e gli eventuali orfani"

pkill -x "$APP_NAME" 2>/dev/null && echo "    app terminata" || echo "    app non era in esecuzione"
pkill -f "whisper-server --model" 2>/dev/null && echo "    whisper-server terminato" || echo "    nessun whisper-server vivo"
sleep 1

if lsof -nP -iTCP:8080 -sTCP:LISTEN >/dev/null 2>&1; then
  warn "porta 8080 ancora occupata da un processo che non ho ucciso:"
  lsof -nP -iTCP:8080 -sTCP:LISTEN | tail -n +2 | sed 's/^/    /'
fi

# ── 3. Wipe dello stato ───────────────────────────────────────────────────────

step "Azzero lo stato dell'app"

for d in \
  "$HOME/Library/Application Support/$IDENTIFIER" \
  "$HOME/Library/Caches/$IDENTIFIER" \
  "$HOME/Library/WebKit/$IDENTIFIER" \
  "$HOME/Library/HTTPStorages/$IDENTIFIER" \
  "$HOME/Library/Saved Application State/$IDENTIFIER.savedState" \
  "$HOME/Library/Preferences/$IDENTIFIER.plist"
do
  if [ -e "$d" ]; then
    rm -rf "$d"
    echo "    rimosso: ${d/#$HOME/~}"
  fi
done

# Guardia sul path prima dell'unico rm -rf che tocca dati dell'utente: deve
# stare sotto ~/Documents, chiamarsi Records ed esistere. Un'espansione di
# variabile andata storta non deve poter cancellare altro.
step "Cancello le registrazioni"

case "$RECORDS_DIR" in
  "$HOME/Documents/"*/Records) ;;
  *) echo "ERRORE: RECORDS_DIR inatteso ($RECORDS_DIR), non lo tocco." >&2; exit 1 ;;
esac

if [ -d "$RECORDS_DIR" ]; then
  count=$(find "$RECORDS_DIR" -mindepth 1 -maxdepth 1 -type d | wc -l | tr -d ' ')
  rm -rf "${RECORDS_DIR:?}"
  echo "    cancellate $count registrazioni da ${RECORDS_DIR/#$HOME/~}"
else
  echo "    nessuna cartella Records da cancellare"
fi

# ── 4. Permessi OS ────────────────────────────────────────────────────────────

step "Resetto i permessi macOS"

for service in Microphone ScreenCapture; do
  if tccutil reset "$service" "$IDENTIFIER" >/dev/null 2>&1; then
    echo "    $service resettato"
  else
    echo "    $service: nessuna voce da resettare (l'app non l'aveva ancora chiesto)"
  fi
done

# ── 5. Build ──────────────────────────────────────────────────────────────────

step "Compilo il bundle release (diversi minuti)"

cd "$ROOT_DIR"
# Solo il .app: il .dmg allungherebbe il build senza servire a un test locale.
npm run tauri build -- --bundles app

[ -d "$BUILT_APP" ] || { echo "ERRORE: bundle non trovato in $BUILT_APP" >&2; exit 1; }

# ── 6. Install ────────────────────────────────────────────────────────────────

step "Installo in /Applications"

if [ -e "$INSTALLED_APP" ]; then
  rm -rf "$INSTALLED_APP" || {
    echo "ERRORE: non riesco a rimuovere $INSTALLED_APP (serve un utente admin)." >&2
    exit 1
  }
  echo "    versione precedente rimossa"
fi

cp -R "$BUILT_APP" "$INSTALLED_APP"
echo "    installata: $INSTALLED_APP"

echo "    firma:"
codesign -dv "$INSTALLED_APP" 2>&1 | grep -E "^(Identifier|Signature|Authority)" | sed 's/^/      /' || true

# ── 7. Lancio ─────────────────────────────────────────────────────────────────

step "Avvio l'app"

open -a "$INSTALLED_APP"

cat <<'CHECKLIST'

    ── Checklist primo avvio ────────────────────────────────────────────────

    1. Si apre il pannello Impostazioni da solo (settings.configured = false)
    2. Sezione Permessi in cima:
       - click su Microfono apre Privacy & Security → Microfono
       - click su Cattura audio sistema apre il pannello Registrazione schermo
       - concedi entrambi, poi CHIUDI E RIAPRI L'APP: macOS non applica il
         permesso di registrazione schermo a un processo già vivo
    3. Sezione Modello locale: deve dire che va scaricato, non "installato e
       pronto". Premi Scarica e guarda la progress bar (~1.5 GB)
    4. Salva → l'indicatore in basso a sinistra passa a "Server attivo"
    5. Lista registrazioni (icona in alto a destra): "Nessuna registrazione
       trovata"
    6. Registra qualcosa con l'audio di sistema attivo, poi verifica che la
       trascrizione mostri sia YOU sia THEM
    7. Controlla che il WAV prodotto sia stereo:
       afinfo ~/Documents/Earwig/Records/*/recording.wav | grep "Data format"
       deve dire 2 ch quando c'era audio di sistema
    8. Quit dell'app → nessun whisper-server deve restare vivo:
       lsof -nP -iTCP:8080 -sTCP:LISTEN

CHECKLIST
