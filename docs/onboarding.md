# Onboarding — percorso di lettura del codice

Ordine consigliato per entrare nelle logiche del repo. Non segue le cartelle ma i
**flussi verticali**: il dato dal microfono fino alla chat, perché la difficoltà
qui non è la sintassi ma il ciclo di vita (chi è la fonte di verità, chi avvia
cosa, cosa resta sul disco se qualcosa fallisce a metà).

Per il funzionamento interno vedi [`architecture.md`](architecture.md), per le
firme [`reference.md`](reference.md), per il perché storico di ogni scelta
[`../DEVLOG.md`](../DEVLOG.md).

Ogni tappa chiude con una domanda di controllo: se sai rispondere senza riaprire
i file, puoi passare alla successiva.

## 0 — Orientamento

| File | Perché |
|---|---|
| `src-tauri/src/lib.rs` | Indice di tutta l'app: i 3 state manageati, tutti i comandi esposti al frontend, e il kill dei processi figli su `ExitRequested` |
| `docs/architecture.md` | Il diagramma del flusso dati end-to-end in cima basta come mappa mentale |
| `DEVLOG.md` | Solo l'elenco dei titoli, dal basso verso l'alto: è la storia dell'app in 29 tappe |
| `src-tauri/tauri.conf.json` + `capabilities/default.json` | Cosa viene bundlato (i due binari) e l'unica capability di rete concessa al webview |

> Domanda: quanti processi separati esistono a runtime, e chi li termina?

## 1 — Registrazione: dal microfono al WAV

| File | Perché |
|---|---|
| `recorder/mod.rs` | `RecorderInner`: cosa serve tenere vivo durante una registrazione, e perché `mic_stream` non può essere droppato |
| `recorder/audio.rs` | Le primitive pure (rms, downmix, resample, interleave, WAV). `TARGET_SAMPLE_RATE` è il vincolo che detta tutto il resto |
| `recorder/mic.rs` | cpal sul default device: rate e canali non sono forzati, si prende quello che l'hardware espone |
| `recorder/system_audio/mod.rs` → `macos.rs` | Routing per piattaforma e SCStream: l'audio di sistema arriva già a 16kHz, il mic no |
| `recorder/aec.rs` | Cross-correlazione e sottrazione dell'eco. Non è qualità audio: senza, la diarizzazione salta |
| `commands/recording.rs` (`start_recording`, `stop_recording`) | Dove le parti si compongono, nell'ordine obbligato: to_mono → resample → AEC → interleave → WAV |
| `commands/recording.rs` (`decode_audio`, `import_audio_file`) | La seconda sorgente per lo stesso pipeline, e perché qui il downmix a mono è voluto |

> Domanda: perché un file registrato è stereo e un file importato è mono, e cosa
> cambia a valle?

## 2 — STT: modello, server, trascrizione

| File | Perché |
|---|---|
| `commands/mod.rs` | Da leggere per primo: `SttSettings`, gli helper di path, e soprattutto `get_stt_settings` che ricalcola i flag dal disco ignorando il JSON |
| `commands/download.rs` | Streaming su `.part` + rename atomico: l'invariante che impedisce a un file troncato di sembrare un modello valido |
| `commands/server.rs` | Le primitive di processo/porta condivise. I due commenti in cima valgono più del codice |
| `commands/stt.rs` | Download modello, lifecycle di whisper-server, `transcribe_recording`, `normalize_speaker` e il sidecar `transcript_error.json` |
| `commands/recording.rs` (`ensure_stt_ready`, `list_recordings`) | La guardia backend e come lo stato di ogni registrazione viene ricostruito leggendo la cartella |

> Domanda: se cancelli il `.bin` del modello a mano mentre l'app è aperta, cosa
> se ne accorge e quando?

## 3 — Stato in UI e gating

| File | Perché |
|---|---|
| `src/lib/types.ts` | Il contratto FE↔BE (snake_case dai comandi, camelCase per `SttSettings`) più tutti gli helper di presentazione |
| `src/App.svelte` | Unico owner dello stato globale: `refreshSttState`/`refreshLlmState`, `canRecord`, `retryTranscription`. Nessuno store, tutto props e callback |
| `src/lib/Onboarding.svelte` | Il gate a schermo intero quando manca il modello: nessun bottone "salta" |
| `src/lib/SettingsPanel.svelte` | Il file più lungo del repo, ma è ripetitivo: leggi la sezione Server STT e le altre si deducono |
| `RecordingsList` / `RecordingDetail` / `TranscriptView` | Presentazionali, 200 righe in tutto: leggili di corsa |

> Domanda: perché `refreshLlmState` ha `attemptStart` a `false` e
> `refreshSttState` a `true`?

## 4 — LLM: riassunto e chat

| File | Perché |
|---|---|
| `commands/llm.rs` (lifecycle) | Perché qui serve `/health` e non basta la porta aperta, e perché `start_llm_server` non aspetta |
| `commands/llm.rs` (ricerca HF, download) | Proxy verso Hugging Face lato Rust, e perché il download lo fa earwig invece di llama-server |
| `commands/llm.rs` (note) | Il sidecar `notes.json`: whole-document replace, un solo scrittore |
| `src/lib/llm.ts` | L'unico file che conosce l'AI SDK: provider locale, fetch via plugin-http, prompt, `parseSummary` tollerante |
| `src/lib/TranscriptNotes.svelte` | L'orchestrazione: streaming, `AbortController`, quando si persiste e quando no |
| `src/lib/TranscriptChat.svelte` | Puramente presentazionale, chiude il giro |

> Domanda: perché la trascrizione va nelle `instructions` e non in un primo
> messaggio della conversazione?

## 5 — I pattern trasversali

Cinque regole che si ripetono in tutto il repo. Capite queste, il resto è meccanico.

1. **La verità è il disco, mai il JSON persistito**: `local_ready`/`llm_ready` sono ricalcolati a ogni lettura.
2. **Sidecar per lo stato**: `transcript.json`, `transcript_error.json`, `notes.json` accanto al WAV. Nessun database.
3. **`.part` + rename per ogni scrittura importante**: download modelli e note.
4. **Guardia duplicata UI + backend**: `canRecord` in `App.svelte` e `ensure_stt_ready` in Rust dicono la stessa cosa, di proposito.
5. **Mai un'azione costosa a sorpresa**: nessun download né avvio server implicito, sempre un'azione esplicita dell'utente.

### Cosa saltare

`main.rs` (boilerplate), `Cargo.lock`/`package-lock.json`, le icone, `Button.svelte`,
`utils.ts`. `system_audio/linux.rs` solo se ti serve il supporto Linux.

## Disallineamenti noti (agosto 2026)

Da tenere presente mentre leggi, perché la documentazione più vecchia mente in
qualche punto:

- La mappa moduli in `AGENTS.md` cita `recorder/diarization.rs`: non esiste più, la diarizzazione è delegata a whisper (vedi `DEVLOG.md`, 2026-07-31). Non elenca nemmeno `Onboarding.svelte` e `lib/utils.ts`.
- `README.md` è fermo a prima dell'onboarding e dell'LLM: indica `~/Movies/Earwig` come cartella registrazioni (oggi è `~/Documents/Earwig/Records`) e descrive una trascrizione manuale che oggi parte da sola dopo lo stop.
- `delete_recording_notes` è registrato in `lib.rs` ma non è invocato da nessun punto del frontend.
