<p align="center">
  <img src="public/icon.png" width="128" alt="Earwig icon" />
</p>

<h1 align="center">Earwig</h1>

<p align="center">
  App desktop macOS per registrare audio (microfono + sistema) e trascriverlo <b>localmente</b> con whisper.cpp — nessun cloud, nessun account, nessun Docker.
</p>

---

## Cosa fa

- Registra **microfono** e **audio di sistema** (es. una call) contemporaneamente, mixandoli in un unico WAV
- Stima — best effort — **chi sta parlando** ("tu" / "sistema" / sovrapposti) confrontando le due tracce prima del mix
- Trascrive in italiano tramite **whisper-server** (whisper.cpp) eseguito in locale, porta `127.0.0.1:8080`
- Mostra la trascrizione raggruppata per speaker

## Requisiti

- macOS (Apple Silicon o Intel) — supporto Linux/Windows non ancora completo
- Permessi di sistema: **Microfono** e **Registrazione schermo** (necessario a ScreenCaptureKit per catturare l'audio di sistema — l'audio dello schermo non viene salvato, solo l'audio)
- ~1.5 GB liberi per il modello Whisper (`ggml-large-v3-turbo`, scaricato al primo avvio)

## Come si usa

### 1. Primo avvio — setup

All'apertura, Earwig chiede i permessi di **microfono** e **registrazione schermo**. Se non li hai ancora concessi, apri ⚙️ **Impostazioni → Permessi** e segui le CTA per raggiungere il pannello di sistema giusto.

Sempre da ⚙️ **Impostazioni**, scarica il modello Whisper (pulsante di download, mostra progresso). Il binario `whisper-server` è già incluso nell'app — non serve installare nulla a parte il modello.

Puoi anche scegliere dove salvare **modello** e **registrazioni** (di default: cartella dati dell'app / `~/Movies/Earwig` rispettivamente).

### 2. Registrare

- Premi **Avvia registrazione** per catturare mic + audio di sistema insieme (es. durante una videochiamata)
- Premi **Ferma** per salvare il WAV nella cartella registrazioni

### 3. Trascrivere

- Apri una registrazione dalla lista e premi **Trascrivi**
- Earwig avvia (se non già attivo) il server whisper-server locale e invia l'audio per la trascrizione
- Il risultato appare raggruppato per chi parla: **Tu** (mic), **Sistema**, **Sovrapposti** — quando è stato possibile distinguerli

> La diarizzazione è "a 2 vie": distingue mic da audio di sistema, non le singole persone all'interno di ciascuna sorgente (es. una call multi-partecipante lato sistema risulta un unico "Sistema").

## Sviluppo

```bash
npm install
npm run tauri dev
```

Richiede il binario `whisper-server` compilato in `src-tauri/binaries/` — vedi `scripts/build-whisper-server.sh` e [`docs/architecture.md`](docs/architecture.md) per i dettagli di build.

## Documentazione

- [`docs/onboarding.md`](docs/onboarding.md) — percorso di lettura del codice, per entrare nel repo
- [`docs/architecture.md`](docs/architecture.md) — flusso dati end-to-end, pipeline audio, integrazione whisper
- [`docs/reference.md`](docs/reference.md) — comandi Tauri, tipi condivisi, componenti Svelte
