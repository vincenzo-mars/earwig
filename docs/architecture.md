# Architettura

Documento unico sul backend: struttura, flusso dati, pipeline audio e integrazione whisper.
Per la superficie API (comandi Tauri, tipi, componenti Svelte) vedi [`reference.md`](reference.md).

## Struttura repo

`src/` (frontend Svelte) + `src-tauri/` (backend Rust) a top-level: layout standard dei template Tauri+Vite. `src-tauri/` deve restare a root perché `Cargo.toml` e la config del bundler usano path relativi fissi rispetto alla root del progetto.

## Flusso dati end-to-end

```
[Microfono]                    [Audio sistema]
    │                               │
    │ cpal, rate e canali           │ ScreenCaptureKit (macOS), già a 16kHz
    │ nativi del device             │ PipeWire/PulseAudio monitor (Linux)
    ▼                               ▼
[mic_samples: Arc<Mutex<Vec<f32>>>]  [sys_samples: Arc<Mutex<Vec<f32>>>]
    │                               │
    ▼                               │
[audio::to_mono()]                  │  downmix esplicito: resample_linear su
    │                               │  buffer interleaved mescolerebbe i canali
    ▼                               │
[audio::resample_linear()]          │  solo il mic: cpal non forza un rate,
mic_native_rate → 16kHz             │  sys arriva già a TARGET_SAMPLE_RATE
    │                               │
    └──────────┬────────────────────┘
               ▼
        [aec::cancel_echo()]           solo se sys è attivo e non vuoto
        toglie dal mic l'audio delle
        casse rientrato dal microfono
        (WebRTC AEC3 + NS + HPF + AGC2)
               │
               ▼
        [audio::balance_speech_levels()] pareggia il volume attivo del mic
               │                       all'audio di sistema (o target speech)
               ▼                       evitando squilibri e allucinazioni
        [audio::interleave_stereo()]   mic → canale L, sistema → canale R
               │                       (senza audio di sistema resta mono)
               ▼
        [audio::write_wav()]           WAV Int16 PCM, 16kHz
               │
               ▼
        [<timestamp>/recording.wav]
               │
               ▼
   [transcribe_recording()]            In-process Parakeet TDT 0.6B v3 (ONNX)
   de-interleave stereo:               Nessun server esterno né porta 8080.
   canale L (mic) → YOU ("0")          Segmentazione in frasi naturali.
   canale R (sys) → THEM ("1")
               │
               ▼
   [TranscriptResult]                  segments[].speaker = "0" | "1" | None
               │                       ("?" ambiguo → normalizzato a None)
               ▼
   [<timestamp>/transcript.json]
               │
               ▼
        [Frontend Svelte]              "0" → YOU, "1" → THEM
```

### Entry point alternativo: import di un file esterno

`import_audio_file` è una seconda sorgente per lo stesso pipeline: decodifica un file scelto dall'utente (symphonia: wav/mp3/m4a/mp4/flac/ogg/aac), lo porta a **mono** 16kHz e scrive `recording.wav` in una nuova cartella Records. Da lì il flusso è identico a una registrazione.

Il downmix a mono è voluto e va tenuto: i canali L/R di un file esterno (un export Zoom, un mp3) non sono "io" e "gli altri", sono solo stereo. Lasciarli separati farebbe produrre a whisper etichette speaker casuali. Essendo mono, `diarize` viene ignorato dal server e `speaker` resta `None`.

### Guardia STT: `ensure_stt_ready`

`start_recording` e `import_audio_file` condividono `ensure_stt_ready(&AppHandle)` (`commands/recording.rs`): entrambe finiscono in `transcribe_recording`, quindi entrambe rifiutano di partire (`Err` in italiano) se il modello non esiste sul path atteso (`local_model_path`) o se `check_stt_server` non ritorna `"running"`. È lo stesso controllo già applicato lato UI (`App.svelte`, `canRecord`), ripetuto qui per il caso in cui il frontend sia disallineato: una chiamata `invoke` diretta o una race sullo stato non bypassano il gate. Non tenta di avviare il server al posto dell'utente: se è stato fermato di proposito dalle impostazioni, resta fermo finché non lo riavvia lui.

## Registrazione

### Stato (`recorder/mod.rs`)

`RecorderInner` è protetto da `tokio::sync::Mutex` e vive per tutta la sessione app.

| Campo | Tipo | Ruolo |
|---|---|---|
| `is_recording` | `bool` | Guardia contro doppio start/stop |
| `mic_samples` / `sys_samples` | `Arc<Mutex<Vec<f32>>>` | Buffer di cattura, scritti dai callback audio |
| `mic_stream` | `Option<cpal::Stream>` | Va tenuto vivo: dropparlo ferma la cattura |
| `sys_capture` | `Option<Box<dyn SysAudioStop>>` | Handle della cattura di sistema |
| `sample_rate` | `u32` | Rate di **output**, fisso a `TARGET_SAMPLE_RATE` |
| `mic_native_rate` | `u32` | Rate nativo del device, dipende dall'hardware |
| `channels` | `u16` | Canali del microfono; serve solo a `to_mono` |

In `stop_recording` il lock serve solo a estrarre buffer e parametri: downmix, resample, AEC e scrittura WAV (CPU-bound su minuti di audio) girano in `spawn_blocking`, mai dentro il lock né sul runtime async — altrimenti bloccherebbero ogni altro command per la durata del calcolo. Il cronometro in UI è un timer locale del frontend (che è l'unico a poter avviare la registrazione): nessun polling IPC verso il backend.

### Microfono (`recorder/mic.rs`)

`cpal` sul default input device, con la sua `default_input_config()`: rate e canali non sono forzati, si prende quello che il device espone. Supporta i formati `F32`, `I16` e `U16` normalizzando tutto a `f32` in `[-1.0, 1.0]`.

### Audio di sistema (`recorder/system_audio/`)

- **macOS** (`macos.rs`): `SCStream` di ScreenCaptureKit sul display primario. La configurazione chiede direttamente `TARGET_SAMPLE_RATE`, che il framework converte nativamente: nessun resampling lato nostro. Richiede il permesso Screen Recording (`NSScreenCaptureUsageDescription` in `Info.plist`).
- **Linux** (`linux.rs`): `cpal` sul primo input device il cui nome contiene "monitor" (PulseAudio/PipeWire).
- Altre piattaforme: `system_audio::start` ritorna un errore esplicito. Windows non è supportato.

Se la cattura di sistema fallisce la registrazione **non** si interrompe: l'errore viene loggato e si registra il solo microfono.

### Primitive condivise (`recorder/audio.rs`)

`TARGET_SAMPLE_RATE` (16000, il rate che `read_wav` di whisper.cpp richiede in `common.cpp`) più le funzioni usate da più moduli:

| Funzione | Note |
|---|---|
| `rms(&[f32]) -> f32` | Energia globale |
| `active_speech_rms(&[f32], frame_size) -> f32` | Calcola l'RMS del parlato attivo (top 30% frame energetici), ignorando silenzi e rumore di fondo |
| `normalize_speech(Vec<f32>, target_rms) -> Vec<f32>` | Normalizza il livello del parlato a un target RMS con limiter di picco (max 0.95) e cap al guadagno (max 10x) |
| `balance_speech_levels(mic, sys) -> Vec<f32>` | Allinea l'RMS attivo del microfono a quello del sistema per evitare squilibri stereo e allucinazioni Whisper |
| `to_mono(Vec<f32>, channels) -> Vec<f32>` | Prende ownership per restituire il buffer intatto quando è già mono. Va sempre chiamata **prima** di `resample_linear` |
| `resample_linear(&[f32], from, to)` | Interpolazione lineare pura-Rust, nessuna dipendenza esterna. Assume input **mono** |
| `interleave_stereo(left, right)` | Intreccia mic e sistema zero-paddando il più corto |
| `write_wav(&[f32], path, rate, channels)` | `hound`, Int16 PCM |

### Echo cancellation e controllo guadagno (`recorder/aec.rs`)

Serve a registrare con gli altoparlanti accesi invece che in cuffia (es. chiamate Teams/Meet). Utilizza **WebRTC AEC3** (Echo Canceller 3) e **AGC2** (Automatic Gain Control 2) tramite il crate puro-Rust `sonora`, con elaborazione a frame da 10ms (160 campioni a 16kHz).

La pipeline applica:
- **AEC3**: modella la risposta all'impulso della stanza, eliminando il percorso diretto e il riverbero multi-path
- **Noise Suppression (NS)**: riduce i rumori di fondo continui (ventole, fruscio)
- **High-Pass Filter (HPF)**: taglia sub-bassi e DC offset
- **GainController2 (AGC2)**: adatta il livello del microfono tramite Voice Activity Detection a rete neurale RNN e limiter anti-clipping
- **Livellamento RMS (`audio::balance_speech_levels`)**: allinea l'energia del microfono a quella dell'audio digitale di sistema (spesso 15-20 dB più forte), impedendo che la voce dell'utente anneghi nel downmix mono di Whisper o venga penalizzata nel confronto energetico di diarizzazione L/R.

## STT (In-process Parakeet TDT 0.6B v3)

### Architettura in-process e modello

A differenza del server LLM (che resta un processo separato su porta 8081), il motore STT è interamente **in-process** all'interno dell'app Tauri, eseguito tramite `parakeet-rs` e ONNX Runtime (`ort`).
Non ci sono binari esterni da compilare o gestire per STT, né porte locali aperte (nessuna porta 8080), né lockfile di processo o rischi di processi orfani.

Il modello di default è **NVIDIA Parakeet TDT 0.6B v3** quantizzato INT8 (`istupakov/parakeet-tdt-0.6b-v3-onnx` su Hugging Face). È un modello FastConformer-TDT all'avanguardia con punteggiatura e capitalizzazione integrate, eccellente per l'italiano e significativamente superiore a Whisper Large-v3-Turbo per precisione e naturalezza delle frasi.

I file del modello risiedono in `<app_data_dir>/models/parakeet-tdt-0.6b-v3/`:
- `vocab.txt` (~94 KB): vocabolario tokenizzatore
- `decoder_joint-model.int8.onnx` (~18 MB): decoder e joint network TDT
- `encoder-model.int8.onnx` (~652 MB): encoder FastConformer INT8

### Download multi-file e stato del modello

Il modello **non** è bundled nel binario dell'app per contenere le dimensioni del pacchetto di installazione:
- `download_local_model` scarica in sequenza i 3 file necessari da Hugging Face tramite streaming HTTP (`reqwest`).
- Ogni file viene scaricato su un file temporaneo `.part` e rinominato atomicamente al completamento.
- La barra di avanzamento emette eventi `download-progress` con `{ step: "model", pct }` calcolato sui byte complessivi (~670 MB totali), e termina con `{ step: "done", pct: 100 }`.
- `is_stt_model_ready` (e `get_stt_settings`) verifica direttamente la presenza di tutti e 3 i file sul filesystem prima di dichiarare il modello pronto (`local_ready: true`).

### Gestione dello stato in-process (`SttEngineState`)

`SttEngineState` (gestito come state in Tauri `lib.rs`) incapsula un `Arc<Mutex<Option<Arc<ParakeetTDT>>>>`:
- L'engine viene inizializzato in modo lazy alla prima trascrizione oppure esplicitamente via `start_stt_server` (preriscaldamento).
- `check_stt_server` ritorna `"running"` se tutti i file del modello sono presenti su disco, oppure `"stopped"` se il modello non è ancora stato scaricato.
- `stop_stt_server` rilascia la sessione ONNX in memoria impostando lo slot a `None`, liberando istantaneamente la RAM.
- `delete_local_model` rilascia l'engine in memoria e rimuove la cartella del modello dal disco.

### Trascrizione e chunking audio

`transcribe_recording` legge il file WAV prodotto dal registratore:
1. Converte i campioni PCM in `f32` a 16 kHz.
2. Per gestire registrazioni arbitrarie e di lunga durata senza picchi eccessivi di memoria o latenze di inferenza dell'encoder, l'audio viene suddiviso in chunk temporali da **120 secondi** (1.920.000 campioni).
3. Ogni chunk viene elaborato da `model.transcribe_samples(chunk, TimestampMode::Sentences)`: Parakeet TDT restituisce direttamente i token temporizzati e raggruppati in frasi naturali con maiuscole e punteggiatura.
4. I timestamp di ogni chunk vengono traslati dell'offset temporale e uniti nella lista dei segmenti.
5. Il risultato viene salvato come `transcript.json` accanto al file WAV (con `transcription_ms` misurato accuratamente) e viene generato un file Markdown strutturato `transcript.md` per Obsidian con frontmatter YAML (e duplicato come `<timestamp>.md` nella cartella `transcripts_dir` se configurata).

### Diarizzazione deterministica a canali fisici

La diarizzazione non si affida a euristiche di clustering spettrale o a confronti di energia fallibili. Sfrutta invece l'isolamento fisico dei canali stereo garantito dalla pipeline audio (AEC3 + AGC2 + bilanciamento RMS):
- **Canale 0 (Sinistro)**: microfono dell'utente isolato dal rientro delle casse → **YOU (`"0"`)**
- **Canale 1 (Destro)**: audio digitale delle altre persone nella chiamata → **THEM (`"1"`)**

Nei file stereo, `transcribe_recording` de-interleave i due canali ed esegue la trascrizione indipendente di ciascun canale con Parakeet TDT:
- I segmenti del canale sinistro ricevono determinisicamente `speaker: Some("0".to_string())`.
- I segmenti del canale destro ricevono determinisicamente `speaker: Some("1".to_string())`.
- Tutti i segmenti dei due interlocutori vengono poi ordinati cronologicamente per `start` e uniti.

Nei file mono (es. audio importati dall'esterno via `import_audio_file`), il canale singolo viene trascritto con `speaker: None`.

### Stato "fallito": `transcript_error.json`

Se `transcribe_recording` incontra un errore (file non valido, memoria insufficiente), scrive un file sidecar `transcript_error.json` con `{ "message": <errore> }` nella cartella della registrazione, popolando il campo `RecordingEntry.error` letto da `list_recordings`. Quando una trascrizione successiva ha successo, il file di errore viene rimosso.

## LLM locale (riassunto e chat)


Per ogni registrazione trascritta, un riassunto/appunti generato una volta e persistito, più una chat di follow-up sulla trascrizione, entrambi tramite un LLM **locale** (nessun cloud): stesso principio di whisper, un secondo binario bundlato invece di una dipendenza esterna (Ollama & co).

### Perché non `@ai-sdk/svelte`

La classe `Chat` del binding Svelte di Vercel AI SDK richiede un backend HTTP proprio che esegua `streamText()` e restituisca il protocollo "UI Message Stream" — non può puntare direttamente a un provider. Earwig non ha un server JS a runtime (frontend Vite/Svelte puro in webview + backend Rust), quindi non c'è dove far girare quel backend. Si usano invece le funzioni **core** di `ai` (`streamText`) direttamente nel frontend (`src/lib/llm.ts`), col provider `@ai-sdk/openai-compatible` puntato su `http://127.0.0.1:8081/v1`, l'endpoint OpenAI-compatible nativo di `llama-server`.

### Fetch via `@tauri-apps/plugin-http`, non fetch nativa del webview

`createOpenAICompatible` riceve un `fetch` custom da `@tauri-apps/plugin-http` invece della fetch nativa del browser. Motivo: l'App Transport Security di macOS può bloccare richieste HTTP semplici da un'app pacchettizzata anche verso `127.0.0.1` ([tauri-apps/tauri#4722](https://github.com/tauri-apps/tauri/issues/4722)), e finora earwig non aveva mai fatto una fetch diretta dal webview verso un server locale (tutto l'HTTP esistente, whisper compreso, passa da Rust via `reqwest`) — nessuna eccezione ATS era mai stata necessaria in `Info.plist`. Il plugin esegue la richiesta lato Rust (nessun caricamento di risorsa nel webview, quindi ATS/CORS non si applicano). Capability scoped a `http://127.0.0.1:8081/*` in `capabilities/default.json`, principio del minimo privilegio.

### Binario e modello: `llama-server` con `--model`, download fatto da earwig

`llama-server` (llama.cpp, stessa org `ggml-org` di whisper.cpp) è compilato da `scripts/build-llama-server.sh`, calco di `build-whisper-server.sh` (binario universale macOS, statico, Metal attivo), con `-DLLAMA_BUILD_UI=OFF`/`-DLLAMA_USE_PREBUILT_UI=OFF` per non bundlare né costruire la web UI (earwig non la usa).

**`-DLLAMA_BUILD_LIBRESSL=ON` è obbligatorio**, non opzionale: senza un backend TLS il binario compila comunque (nessun errore in fase di build) ma fallisce a runtime con `"HTTPS is not supported"` su qualunque richiesta HTTPS — verificato empiricamente in fase di implementazione, non assunto dalla documentazione. `llama.cpp` non usa più `libcurl`: linka OpenSSL/BoringSSL/LibreSSL direttamente. LibreSSL vendorizzata dal build stesso (nessuna libreria di sistema da installare sulla macchina di sviluppo) è l'unica delle tre opzioni coerente col resto dello script: self-contained, nessuna dylib esterna.

A differenza di quanto ci si aspetterebbe, **il modello non viene scaricato da `llama-server`**: pur supportando nativamente `--hf-repo <owner/repo> --hf-file <filename>`, la sua barra di progresso nativa (`Downloading <file> ─────╴ NN%`) è emessa da `common/download.cpp` dietro un check `!is_output_a_tty()` — sotto `Stdio::piped()` (necessario per catturare l'output da un processo Tauri) non produce assolutamente nulla, quindi non è osservabile per costruire una progress bar (vedi `DEVLOG.md`). Duplicare il layout della cache nativa di llama-server per pre-scaricare il file altrove è stato scartato a sua volta: usa uno schema hash-based (`models--org--repo/blobs/<hash>`, stile huggingface_hub) con file serviti spesso via redirect a storage "Xet", non banalmente riproducibile lato nostro.

`download_llm_model` (`commands/llm.rs`, via l'helper condiviso `commands/download.rs`) scarica quindi il GGUF da sé via `reqwest`, stesso identico pattern di `download_local_model` per whisper: stream diretto da `https://huggingface.co/<repo>/resolve/main/<file>`, scrittura su `.part`, evento di progresso (`llm-download-progress`) a ogni cambio di percentuale intera (non per chunk: sarebbero decine di migliaia di eventi IPC su un file da GB), rename atomico a fine stream. Il file finisce in `<model_dir>/llm-models/<org>--<repo>/<file>.gguf` (slash sanitizzato in `--`, nessun hash: percorso ispezionabile a mano). `start_llm_server` passa quel percorso a `llama-server` con `--model <path>`, non più `--hf-repo`/`--hf-file`, e fallisce subito se il file non esiste ancora sul disco. Il repo/file/size scelti dall'utente sono persistiti in `SttSettings.llm_hf_repo`/`llm_hf_file`/`llm_size_bytes`; `llm_ready` è ricalcolato da `get_stt_settings` verificando l'esistenza del file, esattamente come `local_ready` per whisper — mai la fonte di verità è il valore persistito.

Chi ha scaricato un modello con una versione precedente di earwig (quella con `--hf-repo`/`--hf-file`) ha la vecchia cache in `<model_dir>/llm-cache/` (layout hash-based sopra): non viene migrata automaticamente, resta finché l'utente preme "Elimina modelli scaricati" in Settings (`clear_llm_cache`, cancella entrambe le cartelle).

### Ricerca modelli: proxy Rust verso l'API di Hugging Face

`search_hf_models`/`get_hf_model_files` (`commands/llm.rs`) interrogano `huggingface.co/api/models` via `reqwest` (lato Rust, non fetch diretta dal webview, per coerenza con l'unico altro precedente HTTP del progetto). `search_hf_models` filtra a `filter=gguf&pipeline_tag=text-generation`; `get_hf_model_files` chiama il dettaglio repo con `?blobs=true` (l'unico modo per ottenere `siblings[].size` reale) e segnala `gated` così la UI può disabilitare la selezione per i repo che richiederebbero autenticazione HF (non supportata). `get_system_memory_gb` (sysctl `hw.memsize` su macOS, `/proc/meminfo` su Linux: nessuna dipendenza) alimenta un badge euristico "consigliato per la tua RAM" sui file di dimensione diversa, mai bloccante.

### Ciclo di vita: `/health`, non solo la porta

A differenza di whisper-server, `llama-server` apre la porta **prima** di aver caricato il modello: `/health` risponde `503` mentre carica, `200` quando è pronto. `check_llm_server` sonda `/health` (non solo `port_is_open`) per distinguere `"loading"` da `"running"`. `start_llm_server` spawna e ritorna subito, **senza** attendere `/health` come fa whisper con la sua attesa-porta-con-timeout: il modello è già sul disco (il download è un passo separato, vedi sopra), ma caricare qualche GB in RAM/VRAM può comunque richiedere parecchi secondi, e bloccare il comando Tauri per quel tempo sarebbe sbagliato. Il frontend (`App.svelte`, `refreshLlmState`) fa polling periodico di `check_llm_server` finché lo stato non è `"running"`.

`whisper-server` e `llama-server` sono processi indipendenti (porte 8080/8081, `WhisperServerState`/`LlamaServerState` separati): avviare/fermare l'uno non tocca l'altro. Chi ha poca RAM viene guidato verso un modello LLM più piccolo dal badge RAM in ricerca, non forzato a spegnere whisper.

`refreshLlmState` in `App.svelte` inverte il default di `attemptStart` rispetto a `refreshSttState` (`false` invece di `true`): al mount si osserva soltanto, non si carica un modello da 1-5GB per una feature che molte sessioni non toccano mai. Il download/avvio è sempre un'azione esplicita dell'utente (Settings o CTA nel pannello note), mai automatico — stessa filosofia di `ensure_stt_ready`, che non avvia mai whisper al posto dell'utente.

### Note per registrazione: `notes.json`

Un solo sidecar accanto al WAV (`read_recording_notes`/`write_recording_notes`/`delete_recording_notes`, scrittura atomica via `.part` + rename come il download del modello whisper), con `summary` (quattro sezioni: testo, punti chiave, azioni, domande aperte) e `messages` (cronologia chat completa). Whole-document replace: un solo scrittore (il componente `TranscriptNotes.svelte`), niente da riconciliare fra due file separati.

Il riassunto è generato con un prompt a sezioni marcate (`RIASSUNTO:`/`PUNTI CHIAVE:`/`AZIONI:`/`DOMANDE APERTE:`, testo semplice non JSON: lo structured output non fa streaming), parsate da `parseSummary` in `llm.ts`. Se il modello non rispetta il formato (possibile coi modelli non curati, scelti dalla ricerca libera), il testo grezzo finisce tutto in `text` senza mai bloccare con un errore.

Il contesto della trascrizione (`buildTranscriptContext`) usa `groupSegments` già esistente per righe `[m:ss] IO: ...`/`[m:ss] INTERLOCUTORE: ...` quando la diarizzazione è disponibile, altrimenti il testo grezzo (mono/import). Va nelle `instructions` (system prompt), non in un primo messaggio: tenere il prefisso del prompt identico turno dopo turno lascia a llama.cpp la possibilità di riusare la KV cache dello slot invece di riprocessare l'intera trascrizione a ogni domanda. Cap a `MAX_TRANSCRIPT_CHARS` (~30000): `--context-shift` è disattivato lato server, quindi un prompt troppo lungo è un errore secco, non un troncamento morbido.

Generazione automatica del riassunto solo se `llmStatus === "running"` all'apertura del dettaglio; altrimenti un CTA esplicito, mai un avvio/download a sorpresa.

### Onboarding: nessun gate

Il download/scelta del modello LLM è opzionale e lazy: `Onboarding.svelte` resta invariato e riguarda solo whisper (nulla funziona senza), la chat è puramente additiva su un'app che già registra e trascrive.

### Modalità ridotta (Edge drawer / Mini bar)

La modalità ridotta (`set_compact_mode`, `set_compact_hover`, `set_compact_y`, `get_compact_position` in `commands/window.rs`, con UI in `CompactStrip.svelte`) permette di tenere Earwig sempre attivo ancorato al bordo destro dello schermo in background con una linguetta minimal in stile macOS premium (colori coerenti con l'app, senza animazioni fancy):
- **Trasparenza nativa**: finestra configurata con `transparent: true` e `macOSPrivateApi: true` in `tauri.conf.json`, feature `macos-private-api` abilitata per `tauri` in `Cargo.toml`, e classe `compact-mode` su `html`, `body` e `#app` con `background: transparent !important`.
- **I 2 stati della linguetta**:
  1. **Idle (non hover)**: sporge solo una linguetta discreta aumentata a `w-[12px]` dal bordo destro dello schermo con un sobrio micro-dot; l'ingombro della finestra è di soli 18px per non intralciare i click su altre app.
  2. **Hover (sopra la linguetta)**: la linguetta si allarga a `w-[82px]` con layout ultra-compatto senza timer: a sinistra il pulsante REC/STOP circolare (con icona Play centrata poco a destra), affiancato all'icona nuda cliccabile `Maximize2` (senza round button) e ai 6 puntini verticali `GripVertical` posizionati a ridosso del bordo schermo. Zero polling in background, transizione fluida e reattiva guidata dagli eventi del puntatore.
- **Spostamento verticale (Draggable)**: tramite l'icona grip `GripVertical` l'utente può trascinare liberamente la barra su e giù lungo il bordo destro dello schermo (`set_compact_y`), con clamping automatico per evitare la barra dei menu di macOS in alto e il Dock in basso. La Y persiste tra un avvio e l'altro: a fine drag il frontend chiama `save_compact_y` che la scrive in `settings.json` (campo `compactY`, `#[serde(default)]`), e `set_compact_mode` all'attivazione la ricarica se in sessione non c'è già stato un drag.
- **Animazione di hover a padrone unico**: la larghezza è animata dal CSS del webview (`transition-[width]` 200ms), mentre il resize nativo della finestra è istantaneo ma governato dal frontend: cresce a hover-start (verso sinistra, invisibile su sfondo trasparente) e si restringe solo ~220ms dopo il mouseleave, quando la transizione è finita e il contenuto è già larghissimo 12px. Un rientro del cursore nel frattempo annulla il restringimento. Prima di questo accordo le due animazioni correvano in parallelo e il contenuto veniva clippato dalla finestra già ristretta a metà transizione.
- **Transizione ridotta → normale**: ripristina decorazioni native, disattiva `always_on_top`, riapplica i vincoli di dimensione minima e riposiziona la finestra alle coordinate precedenti.

### Tray Icon (Menu Bar) e Gestione Dinamica del Dock

Earwig integra un'icona nella barra dei menu macOS in alto a destra (`TrayIconBuilder` con `icon_as_template(true)`) che consente all'app di vivere silenziosamente come utility daemon:
- **Interazione Tray**:
  - *Clic sinistro*: toggle rapido della dashboard principale (`show_dashboard` / `hide_dashboard`). Se la finestra è aperta, la nasconde e rimuove l'app dal Dock; se è chiusa o minimizzata, la ripristina e focalizza.
  - *Clic destro*: menu contestuale minimale con voci native per accedere direttamente alle "Impostazioni..." (emette l'evento frontend `open-settings`) ed "Esci da Earwig" (chiusura sicura con terminazione dei server locali tracciati).
- **Visibilità Dinamica nel Dock (`set_dock_visible`)**:
  - Quando la dashboard è aperta e visibile, l'app passa a `NSApplicationActivationPolicy::Regular` (visibile nel Dock di macOS con puntino indicatore e presente in `Cmd+Tab`).
  - Quando la dashboard viene chiusa (pulsante rosso "X" o `Cmd+W`), minimizzata (pulsante giallo o `Cmd+M`) o nascosta via tray, l'evento viene intercettato da `on_window_event`: la finestra viene nascosta (`window.hide()`) e la policy passa a `NSApplicationActivationPolicy::Accessory` (scompare dal Dock a meno che non sia stata fissata manualmente dall'utente con "Mantieni nel Dock", e scompare da `Cmd+Tab`).
  - Se l'utente clicca l'app nel Finder o Spotlight mentre è già in esecuzione in background, l'evento `RunEvent::Reopen` riapre la dashboard e ripristina la policy su `Regular`.
- **Avvio Automatico al Boot (Autostart)**:
  - Gestito dal plugin `tauri-plugin-autostart` (e `@tauri-apps/plugin-autostart` lato frontend) tramite un `LaunchAgent` nativo macOS (`~/Library/LaunchAgents/com.vincenzomars.earwig.plist`).
  - All'avvio del sistema, Earwig viene lanciato con il flag `--background`: parte direttamente in policy `Accessory` con finestra nascosta, pronto all'uso istantaneo nella barra dei menu.
  - Lo switch è configurabile sia nell'ultimo step dell'Onboarding (attivo per default) sia nella prima sezione delle Impostazioni (`SettingsPanel.svelte`).

## Stato persistente

- `settings.json` — `SttSettings` serializzato in `app_data_dir/` (`local_ready`/`llm_ready` sono solo l'ultimo valore noto, vedi sopra; `llm_hf_repo`/`llm_hf_file`/`llm_size_bytes` sono il modello LLM scelto, tutti `#[serde(default)]` per non far fallire il parsing di un `settings.json` scritto prima che questi campi esistessero)
- `app_data_dir/models/ggml-large-v3-turbo.bin` — il modello whisper; `ggml-large-v3-turbo.bin.part` può comparire durante un download, sempre transitorio
- `app_data_dir/models/llm-models/<org>--<repo>/<file>.gguf` — i modelli LLM scaricati da earwig stesso (`download_llm_model`); `.gguf.part` durante un download, sempre transitorio
- `app_data_dir/models/llm-cache/` — vecchia cache nativa di `llama-server` (`LLAMA_CACHE`, layout hash-based), non più scritta ma non migrata automaticamente: resta finché l'utente non preme "Elimina modelli scaricati"
- `~/Documents/Earwig/Records/<timestamp>/` — `recording.wav` + `transcript.json` + `transcript.md` (successo, note formattate per Obsidian) oppure `transcript_error.json` (ultimo tentativo fallito, rimosso al primo rilancio riuscito) + `notes.json` (riassunto/chat, opzionale). Se configurata, le note `.md` vengono salvate anche nel Vault Obsidian / cartella `transcripts_dir` dell'utente come `<timestamp>.md`

## Dipendenze chiave

| Crate/lib | Versione | Uso |
|---|---|---|
| tauri | 2.x | Framework desktop |
| cpal | 0.17 | Audio input cross-platform |
| screencapturekit | 7.x | System audio macOS |
| hound | 3.x | Encoding WAV |
| symphonia | 0.5 | Decodifica dei file importati, Rust puro, niente ffmpeg |
| reqwest | 0.12 | HTTP verso whisper-server, Hugging Face (modello whisper e ricerca modelli LLM) |
| tokio | 1.x | Runtime async |
| tauri-plugin-http | 2.x | Fetch lato Rust per le chiamate AI SDK verso `llama-server` (evita ATS/CORS nel webview) |
| tauri-plugin-autostart | 2.x | Gestione avvio all'accensione del Mac tramite LaunchAgent |
| sonora | 0.2 | Port puro-Rust di WebRTC AEC3 (Acoustic Echo Cancellation) + NS + HPF |
| objc2 / objc2-app-kit | 0.6 / 0.3 | Binding macOS nativi per commutare la NSApplicationActivationPolicy a runtime |

Frontend: `ai` (core, `streamText`) + `@ai-sdk/openai-compatible` per il provider locale, `@tauri-apps/plugin-http` per il fetch, `@tauri-apps/plugin-autostart` per il controllo dell'avvio automatico, tutti in `src/lib/`.

## Pipeline CI/CD e Release

Earwig include due workflow GitHub Actions in `.github/workflows/`:

### 1. Continuous Integration (`ci.yml`)
- **Trigger**: Pull request su qualunque branch e push su `develop`, `feat/**`, `fix/**`.
- **Jobs**:
  - `frontend-check`: esegue `npm run lint` (Biome) e `npm run typecheck` (Svelte).
  - `rust-check`: runner `macos-14` (Apple Silicon), installa toolchain Rust `stable` (target `aarch64-apple-darwin`), applica caching di Cargo via `Swatinem/rust-cache` ed esegue `cargo check --manifest-path src-tauri/Cargo.toml`.

### 2. Continuous Delivery & Release (`release.yml`)
- **Trigger**: Push/merge su `main` e `workflow_dispatch` (avvio manuale).
- **Runner**: `macos-14` (Apple Silicon M1 nativo).
- **Flusso**:
  1. `actions/checkout@v4` con `fetch-depth: 0` (necessario per tag e changelog).
  2. Setup Node.js 20 con cache npm + `npm ci`.
  3. Setup Rust `stable` per target `aarch64-apple-darwin` con cache Cargo.
  4. Caching server STT/LLM: `actions/cache@v4` cacha `src-tauri/binaries/` con chiave calcolata sull'hash di `scripts/build-whisper-server.sh` e `scripts/build-llama-server.sh`. Se cache miss, esegue gli script di compilazione.
  5. Build Tauri bundle release: `npm run tauri build -- --target aarch64-apple-darwin`.
  6. Calcolo tag e versione: legge la versione da `package.json`. Se `v<version>` è già presente su Git (merge multipli prima di un version bump), genera automaticamente `v<version>-b<run_number>`; se è inedito, usa `v<version>`.
  7. Raccolta asset: estrae il `.dmg`, crea l'archivio `.zip` del bundle `Earwig.app` usando `ditto` (per preservare symlink e permessi macOS) e calcola i checksum SHA-256 (`checksums-sha256.txt`).
  8. Creazione GitHub Release: pubblica la release via `softprops/action-gh-release@v2` con note di rilascio generate automaticamente da GitHub e gli asset allegati.

