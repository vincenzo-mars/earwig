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
               │
               ▼
        [audio::interleave_stereo()]   mic → canale L, sistema → canale R
               │                       (senza audio di sistema resta mono)
               ▼
        [audio::write_wav()]           WAV Int16 PCM, 16kHz
               │
               ▼
        [<timestamp>/recording.wav]
               │
               ▼
   [transcribe_recording()]            multipart POST /inference
   language=it, response_format=       i byte del file vanno così come sono:
   verbose_json, diarize=true          sono già nel formato di whisper.cpp
               │
               ▼
   [whisper-server locale]             127.0.0.1:8080
   ggml-large-v3-turbo                 diarizza confrontando L vs R
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
| `rms(&[f32]) -> f32` | Energia; usata da AEC |
| `to_mono(Vec<f32>, channels) -> Vec<f32>` | Prende ownership per restituire il buffer intatto quando è già mono. Va sempre chiamata **prima** di `resample_linear` |
| `resample_linear(&[f32], from, to)` | Interpolazione lineare pura-Rust, nessuna dipendenza esterna. Assume input **mono** |
| `interleave_stereo(left, right)` | Intreccia mic e sistema zero-paddando il più corto |
| `write_wav(&[f32], path, rate, channels)` | `hound`, Int16 PCM |

### Echo cancellation (`recorder/aec.rs`)

Serve a registrare con gli altoparlanti accesi invece che in cuffia. Stima il ritardo fra `sys` e la sua eco nel microfono via cross-correlazione (finestra di 2s, ritardo massimo 200ms), poi sottrae la componente stimata con un coefficiente ai minimi quadrati. Se la correlazione massima è sotto `MIN_CORR` (0.10) l'eco è trascurabile e il mic torna invariato.

Non è solo qualità audio: **la diarizzazione dipende da questo**. L'audio di sistema rientrato nel canale mic falserebbe il confronto di energia con cui whisper attribuisce gli speaker.

Limiti: copre il percorso di eco diretto dominante, non il riverbero multi-path, e assume un ritardo stabile per tutta la registrazione.

## STT

### Binario e modello

`whisper-server` (whisper.cpp) è compilato da `scripts/build-whisper-server.sh` come binario universale macOS (arm64 + x86_64), statico e con Metal attivo, e finisce in `src-tauri/binaries/`. Tauri lo bundla come risorsa (`tauri.conf.json` → `bundle.resources`) e a runtime `bundled_bin_path` lo risolve con `app.path().resolve(..., BaseDirectory::Resource)`.

Il modello **non** è bundled: `download_local_model` scarica `ggml-large-v3-turbo.bin` (~1.5 GB) da Hugging Face in streaming, emettendo eventi `download-progress` con payload `{ step: "model" | "done", pct }`.

Il download scrive su un file temporaneo `<model>.bin.part` nella stessa cartella del modello finale, e fa `rename` atomico al path definitivo solo a stream completato con successo. Se lo stream fallisce (rete, chiusura app a metà) il `.part` viene ripulito con un `remove_file` best-effort prima di propagare l'errore; se l'app crasha invece di uscire in modo pulito, il `.part` può restare orfano ma non interferisce mai con `model.exists()` sul path finale, quindi non appare mai come modello valido.

### Stato del modello: riconciliato col disco, non col flag salvato

`SttSettings.local_ready` è persistito in `settings.json`, ma non è la fonte di verità: `get_stt_settings` lo ricalcola a ogni chiamata verificando se il file del modello esiste realmente sul path atteso (`local_model_path`), ignorando il valore letto dal JSON. Questo copre due casi che il flag salvato da solo non gestirebbe: l'utente che cancella il `.bin` a mano, e un download interrotto che grazie al file temporaneo non lascia comunque un `.bin` valido. Il valore scritto da `download_local_model` a fine download resta nel file per compatibilità ma è puramente informativo: chiunque legga lo stato via `get_stt_settings` vede sempre la realtà del filesystem.

### Ciclo di vita del server

Le primitive di processo/porta (`port_is_open`, `wait_for_port`, `spawn_tracked`, `stop_tracked_server`, `kill_tracked`, `default_threads`) vivono in `commands/server.rs`, condivise con il server LLM (vedi sotto): la logica di stop porta due invarianti non ovvi (guard rilasciato prima di ogni `.await`, mai kill-by-porta) che una copia-incolla fra due domini avrebbe finito per rompere in uno dei due punti. `spawn_tracked` (spawn + registrazione del child nello slot) e `default_threads` (core meno due, mai sotto 1) sono l'equivalente per l'avvio; `kill_tracked` è il kill best-effort usato alla chiusura dell'app.

Due dettagli servono a non lasciare server orfani, cioè processi vivi che tengono la porta occupata senza che nessuno li tracci più (al riavvio successivo l'app li trova e rifiuta di fermarli, vedi il caso orfano più sotto):

- `spawn_tracked` imposta `kill_on_drop(true)`: se il `Child` viene droppato senza un kill esplicito (unwind da panic, o un errore a metà di `stop_tracked_server`), tokio termina il processo invece di abbandonarlo.
- `stop_tracked_server` toglie il child dallo slot con `take()` **prima** di killarlo: se `start_kill` fallisce il segnale non è mai partito, quindi il child va rimesso nello slot prima di propagare l'errore. Un `wait` fallito invece non si recupera, perché a quel punto il SIGKILL è già stato inviato.

Nessuno dei due copre la morte violenta dell'app (SIGKILL sul processo padre, Force Quit, logout): lì `RunEvent::ExitRequested` non viene mai emesso e nessun codice del padre gira più. È il motivo per cui un `whisper-server` può sopravvivere a una sessione terminata da un segnale esterno, ad esempio un `tauri dev` ucciso dal terminale invece che chiuso dalla finestra. Quel caso non si previene: si rimedia alla sessione successiva, con il lockfile.

### Lockfile del processo

Un server sopravvissuto a una morte violenta tiene la porta, e senza altre informazioni è indistinguibile da un processo di terzi: l'app non può né riusarlo con cognizione né fermarlo, perché l'invariante "mai kill-by-porta" (sopra) le impedisce di terminare qualcosa che non ha avviato lei.

`spawn_tracked` scrive quindi `<app_data_dir>/run/<nome>.json` (`whisper.json`, `llm.json`) con quattro campi:

| Campo | A cosa serve |
|---|---|
| `pid` | Chi terminare, quando il processo non è più un `Child` |
| `bin` | Riconoscere il processo: deve essere il nostro binario |
| `started_at` | Difesa contro il riciclo dei pid, nel formato grezzo di `ps -o lstart=` |
| `model` | Sapere cosa ha in RAM: un `llama-server` orfano può avere un GGUF diverso da quello ora selezionato |

`read_valid_lock` restituisce il lock **solo** se `ps -p <pid> -o lstart=,comm=` conferma sia il binario sia l'istante di avvio; altrimenti il lock è spazzatura di una sessione passata e viene cancellato senza mandare nessun segnale. È l'unica cosa che autorizza `kill_pid`: un pid non verificato può essere stato riassegnato dall'OS a un processo di qualcun altro.

Sui tre punti in cui la porta è occupata ma lo slot `Child` è vuoto:

- **avvio** (`try_adopt_running_server`): lock valido e modello atteso → si **adotta** il processo così com'è, senza ricaricare il modello; lock valido ma modello diverso → si termina e si rispawna; nessun lock valido → si usa comunque quello che risponde sulla porta, come l'app ha sempre fatto (è il flusso di chi avvia un server a mano per debug, vedi la skill `run-earwig`), e sarà lo stop a rifiutarsi di terminarlo
- **stop** (`stop_tracked_server`): lock valido → SIGKILL per pid e attesa del rilascio della porta; altrimenti l'errore di sempre, che chiede di chiudere il processo a mano
- **chiusura dell'app** (`kill_tracked`): stesso trattamento. Senza questo ramo un processo adottato non verrebbe mai terminato e verrebbe riadottato a ogni sessione, all'infinito

Due limiti accettati: `ps -o lstart=` formatta la data secondo il locale, quindi un cambio di locale fra due sessioni fa fallire il confronto e l'app non adotta (degrada verso la prudenza, mai verso il kill sbagliato); e due istanze di earwig in parallelo condividono il lockfile, con la seconda che adotterebbe il server della prima.

`WhisperServerState` tiene l'unico handle al processo. `start_local_server`:

1. Se la porta 8080 è già in ascolto, esce subito
2. Verifica che il modello esista su disco
3. Spawna il binario con `--model/--host/--port`, più `--flash-attn` e `--threads`
4. Polla la porta ogni 250ms fino a 60s

Flash attention e numero di thread non hanno UI: si deducono dalla macchina (`available_parallelism` meno due, per lasciare respiro al resto del sistema). `--flash-attn` esiste solo dalle build recenti di whisper.cpp, quindi il tag pinnato in `scripts/build-whisper-server.sh` e i flag passati qui devono restare allineati: passare `-fa` a un binario vecchio lo fa uscire con un errore di parsing.

`stop_local_server` (condivisa da `stop_stt_server`, `restart_stt_server` e `delete_local_model`) estrae il `Child` da `WhisperServerState` tenendo il `MutexGuard` solo per il `take()`, mai attraverso un `.await`: poi killa e attende il processo, e infine polla la porta 8080 ogni secondo (timeout 5s) finché non risulta libera, perché il socket può restare occupato un istante dopo che `wait()` è tornato. Se `WhisperServerState` è già vuoto ma la porta risulta comunque aperta (un whisper-server orfano da una sessione precedente, non tracciato da questo processo), la funzione non tenta di killare nulla per porta: ritorna un errore esplicito che chiede di chiudere il processo a mano. `restart_stt_server` è `stop_local_server` + `start_local_server`: se lo stop fallisce (incluso il caso orfano), il restart fallisce con lo stesso errore. `delete_local_model` ferma il server prima di cancellare il file (il processo lo tiene aperto) e propaga lo stesso errore se lo stop fallisce.

In `lib.rs` l'app è costruita con `.build()` invece di `.run()` diretto, così il loop eventi può intercettare `RunEvent::ExitRequested` e terminare il figlio, altrimenti `whisper-server` resterebbe in background dopo il quit.

### Trascrizione

`transcribe_recording` invia il WAV in multipart a `POST /inference` con `language=it`, `response_format=verbose_json` e `diarize=true`, in streaming dal file (`Part::stream`, mai caricato per intero in RAM). Il file **non** viene convertito: quello che Earwig produce è già mono/stereo 16kHz 16-bit PCM, cioè esattamente ciò che `read_wav` accetta.

Il risultato viene arricchito con `transcription_ms` (misurato lato client, non presente nella risposta del server) e scritto come `transcript.json` accanto al WAV.

### Stato "fallito": `transcript_error.json`

`transcribe_recording` (il comando Tauri, non `run_transcription` che ne contiene la logica) avvolge l'intera chiamata: se il risultato è `Err`, scrive `{ "message": <errore> }` in `transcript_error.json` nella stessa cartella del WAV; se è `Ok`, rimuove quel file se presente. Questo copre sia il primo tentativo (es. whisper-server giù, JSON malformato) sia un rilancio: un rilancio riuscito su un record già segnato come fallito cancella la traccia d'errore, altrimenti resterebbe segnato "fallito" nonostante un `transcript.json` valido.

`list_recordings` legge `transcript_error.json` solo quando `transcript.json` non è presente (i due non dovrebbero mai coesistere, ma se coesistessero il transcript vince). Il messaggio letto popola `RecordingEntry.error`, da cui il frontend deriva un terzo stato (`TranscriptStatus`, vedi [`reference.md`](reference.md)) oltre a "trascritto"/"in attesa".

Il rilancio non ha un comando dedicato: è lo stesso `transcribe_recording(path)`, richiamato con `<folder_path>/recording.wav` dal frontend (`App.svelte`, `retryTranscription`).

### Diarizzazione: la fa il server

Con `diarize=true` e un file **stereo**, whisper.cpp confronta l'energia dei due canali su ogni segmento (`estimate_diarization_speaker` in `server.cpp`, soglia 1.1) e mette il risultato nel campo `speaker` di ogni segmento del `verbose_json`:

- `"0"` → canale sinistro → microfono → **YOU**
- `"1"` → canale destro → audio di sistema → **THEM**
- `"?"` → energie a meno del 10% l'una dall'altra

`normalize_speaker` riduce l'ambiguità a `None`, lo stesso stato dei file mono: le etichette sono due, non tre. Sui file mono il server ignora `diarize` da solo (`pcmf32s.size() == 2` è falso), quindi non serve condizionare il campo lato client.

Verificato empiricamente: due voci distinte su canali separati escono correttamente come `speaker` `"0"` e `"1"`; senza il campo `diarize` il campo `speaker` non compare affatto.

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

## Stato persistente

- `settings.json` — `SttSettings` serializzato in `app_data_dir/` (`local_ready`/`llm_ready` sono solo l'ultimo valore noto, vedi sopra; `llm_hf_repo`/`llm_hf_file`/`llm_size_bytes` sono il modello LLM scelto, tutti `#[serde(default)]` per non far fallire il parsing di un `settings.json` scritto prima che questi campi esistessero)
- `app_data_dir/models/ggml-large-v3-turbo.bin` — il modello whisper; `ggml-large-v3-turbo.bin.part` può comparire durante un download, sempre transitorio
- `app_data_dir/models/llm-models/<org>--<repo>/<file>.gguf` — i modelli LLM scaricati da earwig stesso (`download_llm_model`); `.gguf.part` durante un download, sempre transitorio
- `app_data_dir/models/llm-cache/` — vecchia cache nativa di `llama-server` (`LLAMA_CACHE`, layout hash-based), non più scritta ma non migrata automaticamente: resta finché l'utente non preme "Elimina modelli scaricati"
- `~/Documents/Earwig/Records/<timestamp>/` — `recording.wav` + `transcript.json` (successo) oppure `transcript_error.json` (ultimo tentativo fallito, rimosso al primo rilancio riuscito) + `notes.json` (riassunto/chat, opzionale)

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

Frontend: `ai` (core, `streamText`) + `@ai-sdk/openai-compatible` per il provider locale, `@tauri-apps/plugin-http` per il fetch, tutti in `src/lib/llm.ts`. Deliberatamente **non** `@ai-sdk/svelte` (vedi sopra).

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

