# Earwig — istruzioni di progetto

## Progetto

**Earwig** è un'app desktop macOS per registrare audio (microfono + sistema) e trascriverlo localmente con NVIDIA Parakeet TDT 0.6B v3 (in-process via ONNX Runtime).

- Stack: Tauri v2 + Svelte 5 + TypeScript + Vite
- Backend: Rust (src-tauri/)
- Frontend: Svelte (src/)
- Target: macOS (arm64 + x64), con supporto futuro Linux e Windows
- STT: locale in-process via parakeet-rs (Parakeet TDT 0.6B v3), nessun server esterno né porta
- Riassunto/chat sulle trascrizioni: LLM locale tramite llama-server (llama.cpp), porta 8081, via Vercel AI SDK (core, non `@ai-sdk/svelte`)
- Nessun Docker, nessuna dipendenza cloud

## Architettura — mappa moduli

```
src-tauri/src/
  main.rs                        # entry point Rust (boilerplate Tauri)
  lib.rs                         # setup app, registra tutti i commands
  commands/
    mod.rs                       # settings, percorsi, permessi OS
    server.rs                    # primitive condivise processo/porta (llm.rs)
    download.rs                  # streaming HTTP condiviso (stt.rs + llm.rs) per il download dei modelli
    stt.rs                       # download modello, motore in-process Parakeet TDT, trascrizione e diarizzazione
    llm.rs                       # ricerca modelli HF, download modello, ciclo di vita llama-server, note (riassunto/chat)
    recording.rs                 # start/stop/status/list registrazione, import file
  permissions.rs                 # permessi OS (Screen Recording via CoreGraphics, pannelli System Settings)
  recorder/
    mod.rs                       # structs condivise (RecorderState, SysAudioStop)
    audio.rs                     # primitive condivise: rms, to_mono, resample, mix, WAV
    mic.rs                       # cattura microfono via cpal
    aec.rs                       # echo cancellation: toglie il system audio rientrato nel mic
    system_audio/
      mod.rs                     # routing platform-specific
      macos.rs                   # SCStream (ScreenCaptureKit)
      linux.rs                   # PulseAudio/PipeWire monitor source

src/
  main.ts                        # entry point Svelte
  App.svelte                     # componente root
  App.css                        # stili
  lib/
    types.ts                     # tipi e helper condivisi
    utils.ts                     # cn() (clsx + tailwind-merge)
    routes.ts                    # mappa rotte (svelte-spa-router, hash routing)
    llm.ts                       # provider AI SDK, prompt, streaming (unico file che conosce l'AI SDK)
    stores/
      servers.svelte.ts          # stato stt/llama + refresh e poll /health
      session.svelte.ts          # registrazione, trascrizione, lock e cronometri
      recordings.svelte.ts       # cache list_recordings, fonte unica di lista e dettaglio
    Button.svelte                # bottone dell'app (varianti ghost/icon/solid/danger/primary)
    Modal.svelte                 # finestra modale generica con backdrop, transizioni e tasto chiusura
    PageHeader.svelte            # header delle pagine interne (back a icona, titolo, azioni)
    AccordionSection.svelte      # sezione collassabile con animazione slide (usato in SettingsPanel)
    Recorder.svelte              # schermata base: hero REC + lista registrazioni
    Onboarding.svelte            # primo avvio: download modello STT a schermo intero
    SttIndicator.svelte          # indicatore stato motore STT
    SettingsPanel.svelte         # modale "/settings": impostazioni/download modello/server STT+LLM
    ServerControls.svelte        # stato + bottoni di un server locale (usato 2x da SettingsPanel)
    DownloadProgressBar.svelte   # barra download modello (Onboarding + SettingsPanel)
    RecordingsList.svelte        # lista registrazioni, montata in home sotto il REC
    RecordingDetail.svelte       # modale "/detail/:id": dettaglio singola registrazione
    TranscriptView.svelte        # rendering trascrizione con speaker
    TranscriptNotes.svelte       # riassunto + chat locale sulla trascrizione (orchestrazione)
    TranscriptChat.svelte        # chat locale: UI presentazionale (log, streaming, input)

docs/
  architecture.md                # backend: flusso dati, pipeline audio, integrazione whisper
  reference.md                   # superficie API: comandi Tauri, tipi, componenti Svelte
```

## Comandi

- Dev: `source ~/.cargo/env && npm run tauri dev` — richiede `src-tauri/binaries/whisper-server` (se manca: `bash scripts/build-whisper-server.sh`) e `src-tauri/binaries/llama-server` (se manca: `bash scripts/build-llama-server.sh`)
- Lint/format: `npm run lint:fix` (biome)
- Typecheck FE: `npm run typecheck`
- Test Rust: `source ~/.cargo/env && cargo test --manifest-path src-tauri/Cargo.toml` — oggi non linka su questo setup (vedi Gotchas macOS): la verifica reale è `cargo check`
- Avvio app, screenshot, whisper-server, smoke test STT: skill `run-earwig`
- Test da utente nuovo (wipe totale + build release + install in /Applications): skill `fresh-install` (`bash scripts/fresh-install.sh`). DISTRUTTIVO: cancella modello, impostazioni, permessi OS e registrazioni

## Commit

Estende le regole globali (skill `/commit`), non le ripete. Specifico di earwig:

- Il pre-commit hook esegue `biome check --staged`: lancia `npm run lint:fix` PRIMA di `git commit`, non dopo il fallimento
- Un commit che tocca `src/` o `src-tauri/` deve includere il doc corrispondente (vedi Regola documentazione): il pre-commit hook git `githooks/pre-commit` lo impone, bypass `SKIP_DOCS=1` (per clone, una volta: `git config core.hooksPath githooks`)

## Verifica (OBBLIGATORIA come i doc)

- Una feature che tocca recording/STT non è finita finché lo smoke test record→transcribe non passa (skill `run-earwig`)
- Una modifica a onboarding, permessi OS o download del modello va verificata da stato vergine (skill `fresh-install`): i permessi concessi in `tauri dev` appartengono a un'identità diversa da quella dell'app installata, quindi in dev quel flusso non lo vedi
- Stesso motivo per qualsiasi fetch diretta dal webview verso un server locale (es. AI SDK → llama-server via `@tauri-apps/plugin-http`): l'App Transport Security di macOS può comportarsi diversamente in un'app firmata/notarizzata installata rispetto a `tauri dev` — verificare con `fresh-install`, non assumere che funzioni perché va in dev
- Modifiche UI: verifica con screenshot reale (skill `run-earwig`), non dichiarare "fatto" senza aver visto il render

## Gotchas macOS

- Alias `$lib` via `paths` nel tsconfig, MAI `baseUrl` (deprecato, rimosso in TS 7)
- Screenshot/cattura da terminale richiedono permesso Screen Recording al terminale; UI scripting via osascript è negato su questa macchina — non tentarlo
- Swift compat libs (setup solo-CLT): `.cargo/config.toml` con `-L /Library/Developer/CommandLineTools/usr/lib/swift/macosx`
- `cargo test` fallisce in fase di link (`__swift_FORCE_LOAD_$_swiftCompatibility56` undefined, via `apple_metal` di screencapturekit): il `-L` sopra basta a `cargo check`/`build` ma non al binario di test. Problema di ambiente, non di codice

## Convenzioni

- Lingua: italiano per testo UI e messaggi utente; inglese per codice Rust/TS, nomi variabili, commenti tecnici
- Nessun `unwrap()` senza gestione errore nel path principale
- Errori: propagati come `Result<_, String>` verso il frontend

## Regola documentazione (OBBLIGATORIA)

Ogni volta che modifichi o aggiungi codice, aggiorna il doc corrispondente:

| Modifica a... | Aggiorna... |
|---|---|
| `recorder/` (qualsiasi file) | `docs/architecture.md` |
| `commands/` (qualsiasi file), `permissions.rs` | `docs/architecture.md` (come funziona) + `docs/reference.md` (firma del comando) |
| `src/` (Svelte) | `docs/reference.md` |
| flusso dati o dipendenze | `docs/architecture.md` |
| decisione non ovvia o cambio architettura | aggiungi entry in `DEVLOG.md` |

Non aspettare che l'utente lo chieda. Aggiorna sempre contestualmente alla modifica del codice.
