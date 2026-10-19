---
name: fresh-install
description: >
  Test heedm as a brand-new user would see it: wipe every trace of the app
  (settings, Whisper model, caches, webview data, macOS permissions,
  recordings), build the release bundle, install it into /Applications and
  launch it, then walk the first-run checklist. Use when asked to test the
  app from scratch, from zero config, as a fresh install, "come un utente
  nuovo", or to verify the onboarding/permissions/model-download flow.
  For day-to-day dev runs and screenshots use the run-heedm skill instead.
---

## Cosa fa e quando NON usarla

Questa skill risponde a una domanda sola: **cosa vede chi installa Heedm per la
prima volta?** Copre il pannello di setup, i due permessi macOS, il download del
modello e il primo giro registrazione → trascrizione.

Non è la skill per lo sviluppo quotidiano. Per lanciare in dev, fare screenshot
o debuggare whisper-server usa `run-heedm`: questa ricompila in release e
azzera dati, quindi costa minuti e distrugge stato.

## Prerequisiti

- `src-tauri/binaries/whisper-server` deve esistere, **prima** del build.
  Senza, `tauri build` fallisce con `resource path doesn't exist` solo dopo aver
  compilato tutto il Rust. Se manca: `bash scripts/build-whisper-server.sh`
  (~10 minuti). Il binario finisce dentro il bundle, quindi la versione di
  whisper che testi è quella presente al momento del build, non quella nel repo.
- Rust in `~/.cargo` e Node disponibili.
- L'utente deve poter scrivere in `/Applications`.

## Esecuzione

```sh
bash scripts/fresh-install.sh
```

Lo script è la fonte di verità della procedura: preflight, chiusura di app e
orfani, wipe, reset permessi, build, install, lancio. Non replicare qui i suoi
comandi a memoria, leggilo se serve sapere cosa fa esattamente.

## Cosa distrugge

| Cosa | Dove | Recuperabile? |
|---|---|---|
| Impostazioni | `~/Library/Application Support/com.vincenzomars.heedm/settings.json` | Si rigenerano |
| Modello Whisper | stessa cartella, `models/` (~1.5 GB) | Solo ri-scaricandolo dall'app |
| Cache e dati webview | `~/Library/Caches/`, `~/Library/WebKit/` | Si rigenerano |
| **Registrazioni** | `~/Documents/Heedm/Records` | **No, cancellate** |
| Permessi microfono e schermo | database TCC | Vanno riconcessi a mano |

La cancellazione delle registrazioni è una scelta esplicita dell'utente
(2026-07-31), non un effetto collaterale. Il `rm -rf` è protetto da una guardia
sul path: se `RECORDS_DIR` non corrisponde a `~/Documents/*/Records` lo script
si ferma invece di cancellare.

## Checklist del primo avvio

Lo script la stampa a fine esecuzione. I punti che si sbagliano più facilmente:

- **Il permesso di registrazione schermo richiede il riavvio dell'app.** macOS
  non lo applica a un processo già vivo: dopo averlo concesso, quit e riapri.
  Se non lo fai sembra che la cattura dell'audio di sistema sia rotta.
- **I permessi concessi in `tauri dev` non valgono qui.** Il binario di dev è
  un'identità diversa da `/Applications/Heedm.app`: l'app installata li chiede
  comunque, anche senza `tccutil`.
- **Il WAV deve essere stereo** quando c'era audio di sistema (mic a sinistra,
  sistema a destra: è così che whisper diarizza). Verifica:
  `afinfo ~/Documents/Heedm/Records/*/recording.wav | grep "Data format"`.
  Un file mono significa che la cattura di sistema non è partita, non che la
  diarizzazione è rotta.
- **Dopo il quit non deve restare nessun whisper-server**:
  `lsof -nP -iTCP:8080 -sTCP:LISTEN` deve essere vuoto.

## Firma del codice

`tauri.conf.json` non definisce `signingIdentity`, quindi il bundle è firmato
ad-hoc. Lo script stampa l'output di `codesign -dv` dopo l'installazione. Se la
firma cambia a ogni rebuild, macOS può trattare ogni versione come un'app nuova
e accumulare voci duplicate in Privacy & Security: se ne vedi più di una per
Heedm, è questo il motivo, non un bug dell'app.
