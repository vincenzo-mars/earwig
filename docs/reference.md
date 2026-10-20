# Reference

Superficie API dell'app: comandi Tauri, tipi condivisi e componenti Svelte.
Per come funzionano dentro, vedi [`architecture.md`](architecture.md).

## Comandi Tauri

Registrati in `lib.rs` con il percorso completo del modulo: la macro `generate_handler!` genera anche un `__cmd__<nome>` e non segue i re-export, quindi `pub use` nel `mod.rs` non basterebbe.

### Impostazioni e percorsi (`commands/mod.rs`)

| Command | Input | Output | Effetti |
|---|---|---|---|
| `get_stt_settings` | — | `SttSettings` | Legge `settings.json` (default se assente), poi ricalcola `local_ready` verificando `local_model_path` sul disco: il flag persistito non è mai la fonte di verità |
| `save_stt_settings` | `settings: SttSettings` | `Result<(), String>` | Scrive `settings.json` |
| `get_local_model_path` | — | `String` | Path assoluto del modello; crea la cartella padre così "Mostra nel Finder" funziona anche prima del download |
| `get_recordings_dir` | — | `String` | Path assoluto della cartella registrazioni, creata se assente |

Gli helper di percorso (`model_dir`, `local_model_path`, `recordings_dir`, `bundled_bin_path`, `llm_model_path`, `llm_models_dir`, `llm_cache_dir`) prendono un `&SttSettings` già caricato, per non rileggere `settings.json` a ogni chiamata. `llm_model_path` ritorna `None` se non è stato scelto un modello; `llm_cache_dir` resta solo per pulire la vecchia cache nativa di llama-server (vedi `clear_llm_cache` sotto).

### STT (`commands/stt.rs`)

| Command | Input | Output | Effetti |
|---|---|---|---|
| `get_stt_presets` | — | `Vec<SttModelPreset>` | Preset del modello STT scaricabile (`parakeet-v3-int8` ~671MB, `parakeet-v3-fp32` ~2.55GB) con etichette per la UI |
| `download_local_model` | — | `Result<(), String>` | Scarica in streaming da Hugging Face i file del preset selezionato (`sttModelId` in `settings.json`: int8 = `vocab.txt` + `encoder-model.int8.onnx` + `decoder_joint-model.int8.onnx`; fp32 = versioni non quantizzate + `encoder-model.onnx.data`) su file `.part` temporanei, emette `download-progress`, fa rename atomico, rimuove la cartella del preset precedente (una sola installazione) e azzera la cache del motore |
| `start_stt_server` | — | `Result<(), String>` | Preriscalda in-process la sessione ONNX `ParakeetTDT` nello stato gestito `SttEngineState` se il modello è presente su disco |
| `stop_stt_server` | — | `Result<(), String>` | Rilascia la sessione del motore in-process impostando `SttEngineState` a `None`, liberando istantaneamente la RAM |
| `restart_stt_server` | — | `Result<(), String>` | `stop_stt_server` + `start_stt_server` (scarica e ricarica la sessione ONNX in memoria) |
| `delete_local_model` | — | `Result<(), String>` | Rilascia il motore in-process e cancella dal disco la cartella del modello selezionato |
| `check_stt_server` | — | `String` | `"running"` se i file del modello selezionato sono presenti su disco, oppure `"stopped"` |
| `transcribe_recording` | `path: String` | `Result<TranscriptResult, String>` | Trascrizione in-process con Parakeet TDT 0.6B v3: se accanto al WAV c'è `mic_asr.wav` (mono 16k senza gain, scritto da `stop_recording` con strategia mic nudo/AEC decisa dal rilevamento leakage) il canale Tu trascrive da lì e l'Altri dal destro del WAV stereo, altrimenti catena storica. Blocchi ~120s tagliati sui silenzi (frame più silenzioso in una finestra ±5s). Scrive `transcript.json` con `transcription_ms`, genera `transcript.md` con frontmatter YAML per Obsidian (e lo copia in `transcripts_dir` se configurata), e rimuove l'eventuale `transcript_error.json` (o lo genera in caso di fallimento) |

### LLM locale (`commands/llm.rs`)

| Command | Input | Output | Effetti |
|---|---|---|---|
| `get_system_memory_gb` | — | `f64` | RAM totale in GB, per il badge "consigliato" nella ricerca modelli |
| `search_hf_models` | `query: String` | `Vec<HfModelSummary>` | Proxy verso l'API di ricerca Hugging Face (`filter=gguf&pipeline_tag=text-generation`) |
| `get_hf_model_files` | `repoId: String` | `HfModelDetail` | Proxy verso il dettaglio repo (`?blobs=true`, unico modo per avere `size` reale); filtra ai soli file `.gguf` |
| `set_llm_model` | `repo: String, file: String, sizeBytes: u64` | `Result<(), String>` | Persiste `llm_hf_repo`/`llm_hf_file`/`llm_size_bytes` in `settings.json`; non avvia/riavvia/scarica da solo |
| `download_llm_model` | — | `Result<(), String>` | Scarica il GGUF scelto direttamente da Hugging Face (stesso pattern di `download_local_model`, `commands/download.rs`), emette `llm-download-progress`. Non usa il downloader nativo di llama-server (`--hf-repo`/`--hf-file`): la sua barra di progresso è gated su `isatty(stdout)` nel sorgente di llama.cpp e non produce output sotto `Stdio::piped()` (vedi `DEVLOG.md`) |
| `check_llm_server` | — | `String` | `"stopped"` (porta chiusa) / `"loading"` (porta aperta, `/health` non ancora 200) / `"running"` (`/health` 200) |
| `start_llm_server` | — | `Result<(), String>` | Errore se il modello non è ancora scaricato (`llm_model_path` non esiste su disco); spawna `llama-server` con `--model <path locale>`; ritorna subito senza attendere `/health` (il caricamento in RAM/VRAM può richiedere parecchi secondi) |
| `stop_llm_server` | — | `Result<(), String>` | Come `stop_stt_server`, ma sul processo/porta LLM (`commands/server.rs`, condiviso) |
| `restart_llm_server` | — | `Result<(), String>` | `stop_llm_server` + `start_llm_server` |
| `clear_llm_cache` | — | `Result<(), String>` | Ferma il server, poi cancella sia `llm-models/` (i modelli scaricati da earwig) sia la vecchia `llm-cache/` (cache nativa di llama-server, chi aveva già scaricato un modello con una versione precedente dell'app) |
| `read_recording_notes` | `folderPath: String` | `RecordingNotes` | File mancante/non parsabile → documento vuoto, mai un errore |
| `write_recording_notes` | `folderPath: String, notes: RecordingNotes` | `Result<(), String>` | Whole-document replace, scrittura atomica (`.part` + rename) |
| `delete_recording_notes` | `folderPath: String` | `Result<(), String>` | `NotFound` trattato come successo |

### Registrazione (`commands/recording.rs`)

| Command | Input | Output | Effetti |
|---|---|---|---|
| `start_recording` | — | `Result<(), String>` | Errore se già in corso, oppure se il modello non è scaricato (vedi `ensure_stt_ready` in [`architecture.md`](architecture.md)); altrimenti avvia mic e audio di sistema |
| `stop_recording` | `timestamp: Option<String>` | `Result<String, String>` | Ferma, applica AEC + AGC2 e bilanciamento RMS, scrive il WAV (usando il timestamp opzionale passato dal frontend o quello calcolato da chrono), ritorna il path |
| `list_recordings` | — | `Result<Vec<RecordingEntry>, String>` | Scansiona la cartella Records, ordina per nome decrescente. `error` (messaggio da `transcript_error.json`) è letto solo quando `transcript` è assente |
| `import_audio_file` | — | `Result<Option<String>, String>` | Stessa guardia `ensure_stt_ready` di `start_recording`; poi picker, decodifica, salva come registrazione. `None` se annullato |

### Permessi OS (`commands/mod.rs` + `permissions.rs`)

| Command | Input | Output | Effetti |
|---|---|---|---|
| `check_microphone_permission` | — | `bool` | `[AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio]` via FFI nativo (CoreMedia/AVFoundation + libobjc), sola lettura, nessun prompt |
| `check_screen_recording_permission` | — | `bool` | `CGPreflightScreenCaptureAccess`, sola lettura, nessun prompt |
| `open_permission_settings` | `pane: "microphone" \| "screen-recording"` | `Result<(), String>` | Apre il pannello Privacy & Security via `open x-apple.systempreferences:...` |
| `get_transcripts_dir` | — | `String` | Ritorna il percorso effettivo per le note Markdown (vault personalizzato o fallback su `recordings_dir`) |
| `set_transcripts_dir` | `path: Option<String>` | `Result<String, String>` | Aggiorna `transcripts_dir` in `settings.json` (`None` per ripristinare il default); crea la directory se mancante e ne ritorna il path |

### Gestione finestra e Dock (`commands/window.rs`)

| Command | Input | Output | Effetti |
|---|---|---|---|
| `show_dashboard` | — | `Result<(), String>` | Mostra, rimuove dalla minimizzazione e porta in primo piano la finestra principale, impostando la policy del Dock a `Regular` |
| `hide_dashboard` | — | `Result<(), String>` | Nasconde la finestra e imposta la policy del Dock a `Accessory` (rimossa dal Dock a meno che non sia fissata) |

`pane` è convertito in un enum chiuso prima di toccare il comando di sistema: un valore non riconosciuto ritorna `Err` e non arriva mai a `open`.

## Eventi

| Evento | Payload | Emesso da |
|---|---|---|
| `download-progress` | `{ step: "model" \| "done", pct: number }` | `download_local_model` |
| `llm-download-progress` | `{ step: "llm" \| "done", pct: number }` | `download_llm_model` |
| `open-settings` | `()` | Menu contestuale della tray icon ("Impostazioni...") |

Evento separato dal precedente invece di un nuovo `step` su `download-progress`: quell'evento ha già due listener indipendenti (`SettingsPanel` e `Onboarding`), e uno `step: "done"` condiviso farebbe scattare per errore, nel listener STT di `SettingsPanel`, il codice che marca `localReady = true`.

## Tipi condivisi (`src/lib/types.ts`)

`SttSettings`, `RecordingEntry`, `TranscriptResult`, `TranscriptSegment`, `DownloadProgress`, `LlmDownloadProgress`, `ServerStatus`, `SttStatus`, `TranscriptStatus`, `NotesSummary`, `ChatRole`, `ChatMessage`, `RecordingNotes`, `HfModelSummary`, `HfGgufFile`, `HfModelDetail`.

`ServerStatus` = `"checking" | "starting" | "running" | "error" | "stopped" | "loading"`, stato di un motore/server locale gestito dall'app (motore in-process STT o processo LLM). `"stopped"` è distinto da `"error"`: è l'esito di uno stop esplicito (impostazioni o modello non scaricato), non di un fallimento. `"loading"` è specifico dell'LLM (`llama-server` apre la porta prima di aver caricato il modello; il motore STT in-process non lo usa mai). `SttStatus` è un alias di `ServerStatus` per non far churnare `SttIndicator`/`SettingsPanel`.

`LlmDownloadProgress { step: "llm" | "done", pct: number }`: stessa forma di `DownloadProgress`, ma su un evento dedicato (vedi sopra).

`SttSettings.transcriptsDir: string | null` è il percorso opzionale della cartella per le note Markdown (es. Vault Obsidian); se `null`, le note vengono salvate nella cartella predefinita di ciascuna registrazione. `SttSettings.llmHfRepo`/`llmHfFile: string` (`""` = nessun modello scelto) sono il modello LLM selezionato dall'utente in Settings. `llmSizeBytes: number` è la size del file scelto (nota dal frontend al momento della selezione, da `HfGgufFile.size_bytes`), usata solo per etichettare il bottone di download. `llmReady: boolean` rispecchia `localReady`: ricalcolato da `get_stt_settings` verificando `llm_model_path` sul disco, mai la fonte di verità è il valore persistito.

`NotesSummary { text, key_points: string[], actions: string[], open_questions: string[], model, generated_at }`, `ChatMessage { id, role: ChatRole, content, at }` (`ChatRole` = `"user" | "assistant"`), `RecordingNotes { version, summary: NotesSummary | null, messages: ChatMessage[] }`: rispecchiano 1:1 le struct Rust in `commands/llm.rs` (snake_case, come `RecordingEntry`/`TranscriptResult`, non camelCase come `SttSettings`).

`HfModelSummary { id, downloads, license }` (risultato di ricerca), `HfGgufFile { filename, size_bytes }`, `HfModelDetail { gated, files: HfGgufFile[] }` (dettaglio repo).

`RecordingEntry.error: string | null` è il messaggio dell'ultimo fallimento di trascrizione (da `transcript_error.json`), sempre `null` quando `transcript` è presente.

`TranscriptStatus` = `"transcribed" | "pending" | "failed"`, stato derivato (non un campo persistito) di una `RecordingEntry`.

Helper di presentazione nello stesso file:

| Helper | Ruolo |
|---|---|
| `speakerInfo(speaker)` | Mappa la chiave grezza a `{ label, color, badgeClass }`: `"0"` → "Tu" (rosso), `"1"` → "Altri" (viola), tutto il resto → "Voce" (grigio) |
| `groupSegments(segments)` | Accorpa segmenti consecutivi con lo stesso speaker |
| `formatDuration(ms)` | `hh:mm:ss`, per il cronometro di registrazione |
| `formatSeconds(s)` | `m:ss`, per i timestamp dei segmenti |
| `formatElapsed(ms)` | `12.4s` sotto il minuto, poi `m:ss` |
| `formatRecordingDate(name)` | Converte il nome cartella `YYYY-MM-DD HH.mm.ss` in data naturale (`"Oggi, 14:23"`, `"Ieri, 09:12"`, `"18 Ott, 16:45"`) |
| `transcriptStatus(entry)` | `RecordingEntry` → `TranscriptStatus` (`transcript` presente → `"transcribed"`, altrimenti `error` presente → `"failed"`, altrimenti `"pending"`) |
| `transcriptStatusInfo(entry)` | `transcriptStatus(entry)` → `{ label, className }` per il badge a 3 stati di `RecordingsList` |

## Store (`src/lib/stores/`)

Moduli `.svelte.ts` singleton: le rune funzionano anche fuori dai componenti, quindi lo stato condiviso non deve più attraversare una catena di props. Ogni store espone un oggetto con getter (`get sttStatus()`, `get all()`, ...) invece delle variabili `$state` nude, perché una variabile esportata direttamente perderebbe la reattività all'import.

Le dipendenze vanno in una sola direzione, `recordings` → `session` → `servers`: mai al contrario, o si formano import circolari.

### `servers.svelte.ts`

Stato dei due motori locali: `sttStatus`, `llmStatus` (`ServerStatus`), `modelReady`, `llmReady`.

`refreshStt(opts?: { attemptStart?: boolean }): Promise<SttSettings | null>` è il punto unico di riallineamento fra stato reale (motore Parakeet TDT + modello sul disco) e stato mostrato in UI: legge `get_stt_settings` (aggiorna `modelReady` da `localReady`) e `check_stt_server`, poi imposta `sttStatus` di conseguenza (`"checking"` mentre gira, poi `"running"` / `"stopped"` / `"starting"`→`"running"` / `"error"`). Con `attemptStart: true` (default) un motore pronto viene confermato, come al mount; con `attemptStart: false` un motore fermo resta `"stopped"`. Ritorna le `SttSettings` lette (o `null` in errore). Chiamata al mount, da `SettingsPanel` dopo ogni azione sul server o sulle impostazioni, e dalla `onContinue` di `Onboarding`; chiunque cambi lo stato del server o del modello la richiama invece di duplicare la logica di check/avvio.

`refreshLlm(opts?)` è il mirror per il server LLM: stessa forma, stessa fonte di verità (`get_stt_settings` + `check_llm_server`), ma **default invertito** — `attemptStart` è `false` qui contro il `true` di STT. Al mount si osserva soltanto: caricare un modello LLM da 1-5GB per una feature che molte sessioni non toccano mai non ha senso, a differenza di STT che serve per le trascrizioni. Aggiorna anche `llmReady` (modello LLM presente sul disco, ricalcolato a ogni chiamata): è il gate del bottone "Avvia il server LLM" in `TranscriptNotes`, e resta allineato da sé perché `SettingsPanel` richiama `refreshLlm` dopo download, cambio modello ed eliminazione della cache.

`llmStatus` non si assegna mai direttamente: passa da `setLlmStatus`, che accende un poll di `check_llm_server` ogni 2s finché lo stato è `"loading"`/`"starting"` e lo spegne appena esce. A differenza del motore STT in-process il tempo di avvio dell'LLM è variabile (caricamento modello GGUF in memoria) e nessun secondo click dell'utente aggiornerebbe altrimenti la UI. Il poll vive nel modulo (`setInterval`), non in un `$effect` di un componente, così non dipende da chi è montato in quel momento.

### `session.svelte.ts`

Sessione di lavoro: `isRecording`, `isStarting`, `isStopping`, `isTranscribing`, `processingItem` (`ProcessingItem`), `durationMs`, `transcribeMs`, `error`, più i derivati `locked`, `canRecord`, `recordGateReason`.

`canRecord` (`sttStatus === "running" && modelReady`, letti da `servers`) è il gate lato UI per registrazione e import, e rispecchia `ensure_stt_ready` lato backend (vedi [`architecture.md`](architecture.md)); `recordGateReason` è il messaggio in italiano quando `!canRecord`, che distingue "modello non scaricato" da "server non attivo". Quando registrare non è disponibile (`!canRecord`, `isTranscribing`, `isStarting`, `isStopping`): il bottone REC in `Recorder.svelte` viene disattivato in modo opaco (bordo e sfondo attenuati, ombra rimossa, opacità 40%, `cursor-not-allowed`) senza spinner concorrenti all'interno né pillole separate nel dock. Le fasi di elaborazione non vengono scritte "a parte": appena la registrazione viene interrotta, l'entry appare direttamente nell'elenco registrazioni in stato di caricamento inline.

`processingItem` (`{ name, folderPath?, phase: "noise_cancellation" | "transcribing" }`) traccia l'elemento attualmente in elaborazione (AEC/cancellazione rumore o trascrizione STT), sia per una nuova registrazione sia per un rilancio o import.

`locked` (`isRecording || isStarting || isStopping || isTranscribing`) è il lock condiviso: un solo flusso alla volta, e mentre è attivo lista e dettaglio disabilitano **ogni** riga, non solo quella interessata.

`runTranscription(path)` è la primitiva che tiene insieme lock e cronometro; la usano lo stop della registrazione, l'import e il rilancio in `recordings`. I cronometri (`durationMs`, `transcribeMs`) sono timer locali del frontend: il frontend è l'unico a poter avviare i due flussi, quindi conosce già l'istante di inizio e non serve interrogare il backend via IPC.

`importFile(): Promise<boolean>` ritorna `true` solo se un file è stato davvero importato e trascritto, così il chiamante sa se navigare alla lista; `false` copre anche l'annullamento del dialog, che non è un errore.

### `recordings.svelte.ts`

Cache di `list_recordings` (`all`, `loading`) con `load(force = false)` e `byName(name)`. `all` deriva la lista aggiungendo in cima l'eventuale `session.processingItem` non ancora salvato su disco: questo fa comparire subito la nuova entry in `RecordingsList` come riga non cliccabile in elaborazione con il badge di stato dedicato (`cancellazione rumore...` e poi `trascrizione in corso...`), fino al completamento e ricaricamento automatico dal filesystem.

`retryTranscription(folderPath: string): Promise<RecordingEntry[]>` imposta `session.processingItem` su `phase: "transcribing"` per mostrare lo stato di caricamento inline sulla riga corrispondente, esegue `session.runTranscription` e ricarica i risultati dal disco.

## Routing (`src/lib/routes.ts`)

[`svelte-spa-router`](https://github.com/ItalyPaleAle/svelte-spa-router) con hash routing. Sotto Tauri il documento è servito da un protocollo custom e non c'è nessun server che possa riscrivere i path: `#/...` è l'unica forma di URL che regge un reload.

L'applicazione adotta un'architettura a **schermata base permanente con modali sovrapposte**:

| Rotta | Componente | Note |
|---|---|---|
| `/` | `Recorder.svelte` | schermata base con hero REC e lista registrazioni |
| `/detail/:id` | `RecordingDetail.svelte` | modale overlay sopra la home; `id` = `entry.name`, cioè il nome della cartella. Passa da `encodeURIComponent` in scrittura e `decodeURIComponent` in lettura |
| `/settings` | `SettingsPanel.svelte` | modale overlay sopra la home per impostazioni e server |
| `*` | `Recorder.svelte` | un hash sconosciuto torna alla home senza modali |

Non esiste una rotta a pagina intera per la lista né per il dettaglio o le impostazioni: la home con REC e lista (`Recorder.svelte`) resta sempre montata nello sfondo. Quando l'hash corrisponde a `/settings` o `/detail/:id`, la vista corrispondente viene montata come modale nativa (`Modal.svelte`) sopra la home, con backdrop scuro semitrasparente e sfocato (`bg-black/60 backdrop-blur-sm`).

Navigazione con `push` (`svelte-spa-router`) per aprire le modali (`push("/settings")`, `push("/detail/<name>")`). La chiusura avviene tramite il tasto `X` nell'header della modale, la pressione del tasto `Escape`, il click sul backdrop esterno o il gesto/tasto Indietro dell'OS: tutte invocano `replace("/")` o `closeModal()`, riallineando l'hash senza lasciare rotte morte nella history.

### Layout e modali (`Modal.svelte`)

Il guscio in `App.svelte` è `relative flex h-screen flex-col overflow-hidden`: la schermata base `Recorder.svelte` occupa la finestra e gestisce il proprio scroll per la lista registrazioni.

Le due schermate secondarie (`SettingsPanel` e `RecordingDetail`) sono racchiuse dentro il componente riutilizzabile `Modal.svelte`:
- **Comportamento a tutta finestra**: quando la finestra è troppo piccola (`innerWidth < 640` o `innerHeight < 600`, monitorati tramite `bind:innerWidth` e `bind:innerHeight` su `<svelte:window>`), la modale si espande a **tutta finestra** (`h-full w-full rounded-none border-0 shadow-none p-0`), azzerando i margini e sfruttando ogni pixel dello schermo. Quando la finestra è sufficientemente capiente, torna automaticamente a presentarsi come modale flottante centrata con angoli arrotondati e ombra: con le dimensioni di default 640x720 dell'app la modale apre già così, e torna a tutta finestra solo se l'utente restringe la finestra sotto la soglia.
- **Backdrop**: `fixed inset-0 z-50 bg-black/60 backdrop-blur-sm` (in modalità compatta a tutta finestra diventa `bg-brand-dark p-0`), con transizione `fade` (150ms). Un controllo su mousedown + click assicura che selezioni di testo o trascinamenti all'interno non chiudano accidentalmente la modale al rilascio del cursore.
- **Finestra modale**: contenitore centrato con bordo `border-brand-cream/10`, sfondo `bg-brand-dark`, ombra elevata `shadow-2xl`, angoli arrotondati `rounded-2xl` e transizione `scale` (start 0.96, 150ms) per dare l'effetto pop-in tipico delle finestre di dialogo native macOS.
- **Header modale**: barra superiore fissa (`px-6 py-4 border-b border-brand-cream/8 bg-brand-dark/95`, compatta a `px-4 py-3` su finestre piccole) con titolo uppercase discreto a sinistra, eventuali azioni e pulsante di chiusura circolare `Button variant="icon"` con icona `X` a destra.
- **Accessibilità**: attributi `role="dialog"`, `aria-modal="true"`, gestione evento `Escape` su window con stopPropagation.
- **Dimensioni**:
  - `SettingsPanel`: `maxWidth="max-w-xl"`, altezza dinamica fino a `max-h-[88vh]` con scroll verticale interno `overflow-y-auto` (a tutta finestra su schermi compatti).
  - `RecordingDetail`: `maxWidth="max-w-5xl"`, altezza bloccata `h-[85vh]` su schermi ampi. Su finestre compatte (`< md`) il layout si riadatta fluidamente in verticale: la trascrizione occupa l'area superiore a scorrimento e il dock con durata, percorso Finder e azioni rapide siede compatto in fondo.

### `PageHeader.svelte`

Header comune alle rotte interne: bottone back solo-icona (`ArrowLeft`, `variant="icon"`) sempre nella stessa posizione in alto a sinistra, titolo, e uno snippet `actions` opzionale allineato a destra. `sticky top-0` con `backdrop-blur`, così resta visibile mentre il contenuto scorre sotto.

Prop: `title: string`, `onBack: () => void`, `actions?: Snippet`.

Il bottone impostazioni **non** sta qui: vive in `Recorder.svelte`, `fixed` in alto a destra. Nelle pagine interne si sovrapporrebbe alle azioni dell'header.

## Componenti Svelte

Svelte 5 con le rune (`$state`, `$derived`, `$effect`, `$props`), un componente per file sotto `src/lib/`. Lo stato condiviso sta nelle store qui sopra; le props restano per ciò che è locale a un singolo albero. I componenti-route ricevono dal router solo `params`: tutto il resto lo leggono dalle store.

### `App.svelte` (root)

Guscio dell'app, senza stato proprio: monta `<Recorder />` come schermata base permanente e `SttIndicator` (pillola di stato, `fixed` in alto a sinistra, visibile in home). Reagisce a `router.location` per montare come modale `SettingsPanel` (se `/settings`) o `RecordingDetail` (se `/detail/:id`). Ascolta l'evento `open-settings` inviato dalla tray icon di sistema per aprire direttamente la modale delle impostazioni.

Rendering condizionato da `servers.modelReady`, valutato allo stesso modo indipendentemente dal motivo per cui il modello manca (primo avvio mai configurato, o modello eliminato in un secondo momento dalle impostazioni): se `!modelReady` l'intera UI normale è sostituita da `Onboarding` a schermo intero, nessun ramo separato per i due casi.

Flusso:
1. Mount → `servers.refreshStt()` + `servers.refreshLlm()`; se `!modelReady` si mostra `Onboarding` al posto della UI normale
2. `Onboarding` scarica il modello e a fine download invoca `onContinue` → `refreshStt()` → `modelReady` torna `true` → si torna automaticamente alla UI normale
3. REC → `session.startRecording()` → `start_recording`
4. STOP → `session.stopRecording()` → `stop_recording` → `transcribe_recording`
5. "o carica un file" (solo a riposo) → `session.importFile()` → se ritorna `true`, `recordings.load(true)`
6. Click su una riga della lista in home → `push("/detail/<name>")` apre la modale di dettaglio sopra la home
7. Click sull'icona impostazioni in home → `push("/settings")` apre la modale delle impostazioni sopra la home
8. Rilancio trascrizione (da `RecordingsList` o `RecordingDetail`) → `recordings.retryTranscription(folderPath)`

Il riallineamento manuale dell'entry aperta dopo un rilancio non serve più: lista e dettaglio derivano entrambi da `recordings`, quindi la `load(true)` finale li aggiorna insieme.

### `Recorder.svelte`

La home: hero REC in alto e `RecordingsList` sotto. Bottone REC/STOP con timer integrato su tasto stop opaco, stato intermedio `isStopping` ("Salvataggio e cancellazione eco..." con spinner), pulsante d'importazione file circolare allineato a destra, dock di stato inferiore ad altezza bloccata (badge salvataggio/trascrizione con `LoaderCircle` ed eventuale errore): tutto letto da `session`, nessuna prop. Ospita anche la toolbar in alto a destra (`fixed right-6 top-6 z-10`) con il pulsante delle impostazioni (`Settings`).

### `RecordingsList.svelte`

Lista delle registrazioni, puramente presentazionale: nessuna prop, nessun header, nessuna larghezza propria (la decide chi la monta). Fa `recordings.load()` in un `$effect` al mount. Era la rotta `/list`, prima che la lista si trasferisse in home.

Stato vuoto: card con bordo tratteggiato, icona `AudioLines`, titolo e hint ("Premi REC...") — non una riga ridotta a placeholder, perché è il primo volto della home per un utente nuovo.

Le registrazioni sono organizzate in un elenco lineare a schede distanziate (`flex flex-col gap-2` con card `rounded-lg border border-brand-cream/8 bg-brand-dark/40 px-3.5 py-2.5`):
- **Titolo e metadati (a sinistra)**: nessun'icona ripetuta a sinistra; mostra direttamente il nome convertito in data naturale leggibile (`formatRecordingDate`, es. "Oggi, 14:23", "Ieri, 09:12") con durata dell'audio affiancata in font mono (`formatSeconds`).
- **A destra**: badge compatto di stato con dot semantico (Trascritto / Elaborazione... / Fallito / In attesa) con eventuale tempo di trascrizione o cronometro elapsed; se la trascrizione è fallita o in attesa, pulsante icona compatto `RotateCcw` per riprovare senza occupare spazio; freccia `ChevronRight` a destra con micro-transizione orizzontale all'hover per le righe trascritte.
- **Interattività**: per le registrazioni trascritte l'intera riga è un `<button type="button">` nativo a tutta larghezza con feedback visivo (`cursor-pointer`, hover border/bg e active scale), eliminando zone morte sui padding e aprendo affidabilmente la modale di dettaglio (`push("/detail/<name>")`); disabilitata solo durante la registrazione attiva (`session.isRecording`) o l'elaborazione. Per registrazioni non trascritte o in errore la riga resta un contenitore informativo con il solo pulsante di retry, senza annidamento di bottoni.

### `SttIndicator.svelte`

Indicatore di stato flottante in alto a sinistra, presente sulla home (nelle pagine interne non compare per non sovrapporsi al tasto Indietro dell'header; su `/settings` lo stato è già mostrato dentro le sezioni `ServerControls`: duplicarlo a schermo è rumore). Unica prop: `status: SttStatus`. È interattivo come link per aprire direttamente la schermata delle impostazioni posizionandosi sulla sezione del server STT (`push("/settings?section=stt-server")`). Il refresh dello stato del server viene comunque eseguito in automatico al focus della finestra dell'app.

| Stato | Dot | Label |
|---|---|---|
| `checking`, `starting` | `warn` lampeggiante | "Avvio..." |
| `running` | `ok` pieno | "STT pronto" |
| `error` | `danger` | "STT non disponibile" |
| `stopped` | `brand-light` (idle) | "STT non scaricato" |

Nessuno stato ha sfondo o ombra: a riposo in alto a sinistra si presenta con il dot semantico affiancato dall'icona del server sempre visibile (`Server`, `size={14}`, altezza `h-8`, posizionato a `left-6 top-6` coerentemente con il padding di pagina `px-6` e allineato al pulsante impostazioni a `right-6 top-6`), mantenendo l'intera area cliccabile per aprire le impostazioni; al passaggio del mouse (`:hover`) o al focus da tastiera si espande con transizione orizzontale fluida (`transition-all duration-200 ease-out`) la sola label testuale (`text-xs text-ink-muted` → `text-ink-primary`). `SttStatus` esclude `loading` (esiste solo per llama-server, che apre la porta a modello da caricare), quindi l'indicatore STT non lo conosce e mostra solo lo stato STT, non quello LLM (vedi `architecture.md`).

### `Onboarding.svelte`

View a schermo intero (non un modal), mostrata da `App.svelte` al posto della UI normale quando `!modelReady`. Logo (`public/icon.png`), titolo e sottotitolo fissi, poi tre stati in sequenza: CTA "Scarica il modello" preceduta dalla scelta del preset del modello STT (`get_stt_presets`, Consigliato pre-selezionato: la scelta viene salvata in `sttModelId` prima del download) → progress bar sull'evento `download-progress` (stesso comando `download_local_model` e stesso evento di `SettingsPanel`) con il messaggio "prenditi un caffè" visibile solo mentre `downloading` è vero → configurazione opzionale della cartella per le note Markdown, toggle per l'avvio automatico con il Mac in background (abilitato di default via `@tauri-apps/plugin-autostart`) e bottone "Inizia a usare Earwig". Nessun bottone "salta": l'unica uscita è un download riuscito. Sotto il sottotitolo una card fissa spiega il dialogo "Earwig è danneggiata" (app non notarizzata: da Sequoia+ macOS blocca i download senza firma Developer ID) con il comando `xattr -cr /Applications/Earwig.app`, unica documentazione del workaround visibile dall'interno dell'app.

Prop: `onContinue: () => void`, chiamata dal bottone di continuazione; `App.svelte` la collega a `servers.refreshStt()` così `modelReady` si aggiorna e il rendering torna da solo alla UI normale, senza che `Onboarding` gestisca la propria visibilità.

### `AccordionSection.svelte`

Componente riutilizzabile per le sezioni collassabili (`id?`, `title`, `icon`, `badge?`, `isOpen = $bindable(true)`, `class`, `children`). Il contenitore adotta lo stesso linguaggio visivo delle righe della lista (`rounded-lg border border-brand-cream/8 bg-brand-dark/40`), con hover curato: da chiuso risalta l'intera card (`hover:border-brand-cream/25 hover:bg-brand-cream/[0.04] active:scale-[0.995]`), mentre da aperto l'header presenta un hover dedicato (`rounded-t-lg hover:bg-brand-cream/[0.04]`) separato dai figli da `border-t border-brand-cream/6`. Il trigger è una riga cliccabile accessibile (`px-3.5 py-2.5`) con icona a sinistra (`size={17}`), titolo (`text-sm font-semibold tracking-wide`), eventuale pillola `badge` e chevron a destra (`size={15}`) con rotazione a 200ms. Il corpo si apre e si chiude con `transition:slide={{ duration: 200 }}` di Svelte.

### `SettingsPanel.svelte`

Modale `/settings` (incapsulata in `Modal.svelte`, `maxWidth="max-w-xl"`) con permessi OS, scelta del preset del modello STT (radio su `get_stt_presets`: il cambio salva `sttModelId` e lancia subito il download della nuova variante, sostituendo quella precedente), cartella registrazioni, cartella note Markdown, download del modello vocale STT, controlli del server STT e percorsi. L'interfaccia è organizzata in sezioni collassabili animate con `AccordionSection.svelte`, tutte aperte di default senza alcuna animazione al landing.

Supporta il deep-linking con auto-scroll tramite querystring (es. `/settings?section=stt-server`): un `$effect` reattivo su `router.querystring` individua l'elemento target, ne assicura l'apertura se collassato ed esegue lo scroll fluido automatico (`scrollIntoView({ behavior: "smooth", block: "start" })`).

Corpo a colonna singola centrata (`max-w-xl`, `gap-3` fra le card); l'header della modale mostra il titolo "Impostazioni" e il pulsante di chiusura `X` (`onclose`).
- **Avvio di sistema**: prima sezione delle impostazioni (icona `Power`) con switch interattivo per abilitare o disabilitare l'esecuzione in background all'accensione del Mac, sincronizzato direttamente con il LaunchAgent di macOS via `@tauri-apps/plugin-autostart`.
- **Permessi**: microfono e cattura audio sistema con righe divise da `divide-brand-cream/6`, icone `text-ink-secondary` (risaltano a `brand-cream` all'hover) e badge pillola di stato ("Concesso" verde `text-ok` con `Check` vs "Non concesso" grigio); ogni riga è evidenziata delicatamente all'hover (`hover:bg-brand-cream/[0.04]`) e cliccabile per aprire le impostazioni di sistema macOS, con refresh automatico al rientro nell'app (`focus`).
- **Cartella registrazioni**: riga percorso testuale cliccabile per rivelare la cartella nel Finder.
- **Note Markdown**: riga destinazione note `.md` cliccabile per rivelare la cartella nel Finder, pulsante "Scegli cartella" / "Cambia cartella" via dialog nativo macOS (`open({ directory: true })`) e pulsante "Ripristina predefinita".
- **Server STT**: card unificata con id `stt-server` che raggruppa modello e motore in-process in un'unica struttura a righe coerente divise da `divide-brand-cream/6`:
  - Riga **Modello**: nome del modello (Parakeet TDT 0.6B v3) e pulsante a icona per il ri-download.
  - Riga **Stato**: dot semantico e label di stato ("Attivo", "Fermo", "Non scaricato", "Download in corso...") affiancato dalla toolbar compatta di controlli (`Play`, `RotateCcw`, `Square`, `Trash2` o `Download`).
  - Riga **Percorso**: path monospazio cliccabile per rivelare la cartella dei modelli nel Finder.
  - Eventuale barra di avanzamento download o messaggio d'errore in coda.

### `ServerControls.svelte`

Componente dedicato ai controlli di un server locale (`title?`, `status`, `error`, `hint?`, `loading`, `locked`, `canStart`, `dangerLabel`, callbacks). Presenta lo stato in modo compatto e coerente con la card del modello (riga con dot semantico `bg-ok` / `bg-warn` / `bg-brand-light` e testo "Attivo" / "Caricamento..." / "Fermo") e una toolbar orizzontale di azioni a sole icone compatte (`size-7`, pulsanti `Button variant="icon"`):
- **Avvia**: icona `Play`
- **Riavvia**: icona `RotateCcw`
- **Ferma**: icona `Square`
- **Azione distruttiva**: icona `Trash2` con stile di pericolo (`text-danger hover:border-danger/30 hover:bg-danger/10`) e tooltip contestuale (`dangerLabel`).

Tutti i comandi server passano da helper interni (`runStt` / `runLlm`: busy flag, `invoke`, refresh dello stato in `App.svelte`, errore mostrato in rosso sotto la riga di stato).

### `RecordingDetail.svelte`, `TranscriptView.svelte`

Dettaglio della singola registrazione, incapsulato in `Modal.svelte` (`maxWidth="max-w-5xl"`, `h-[85vh]`):

- **Accesso**: l'apertura del dettaglio da `RecordingsList` o URL è consentita solo per registrazioni in stato `transcribed`; tentativi su file non trascritti o inesistenti chiudono la modale e ripristinano l'hash `/`.
- **Testata modale**:
  - Titolo dinamico formattato con la data naturale (`formatRecordingDate(entry.name)`, es. "Oggi, 14:23").
  - Durata audio con icona orologio `Clock` e testo monospazio (`formatSeconds`), senza stile badge.
  - Pulsante a icona "Copia tutto" (`Copy` / `Check`, `Button variant="icon"`) per copiare l'intera conversazione negli appunti con feedback visivo.
  - Pulsante a icona cartella (`Folder`, `Button variant="icon"`) per aprire e rivelare la directory nel Finder (`revealItemInDir`).
  - Pulsante a icona per rilanciare la trascrizione (`RotateCcw`, `Button variant="icon"`).
  - Pulsante di chiusura circolare `X`.
- **Area conversazione (Trascrizione a tutta larghezza stile GPT)**:
  - Layout a tutta larghezza e a tutta altezza senza sidebar né bottom bar: il dialogo sfrutta l'intero corpo della modale.
  - Voci contrassegnate semplicemente da **"Tu:"** (rosso `red-400`, microfono) allineato a destra e **"Altri:"** (viola `purple-400`, sistema) allineato a sinistra.
  - Micro-azione di copia del singolo intervento al passaggio del mouse con feedback visivo temporizzato.

### `TranscriptNotes.svelte`, `TranscriptChat.svelte`, `src/lib/llm.ts`

Riassunto e chat locale sulla trascrizione di una registrazione (vedi `architecture.md` per il flusso completo). `TranscriptNotes` è l'orchestrazione: carica `read_recording_notes` al mount, genera il riassunto in streaming (`streamSummary` da `llm.ts`) automaticamente se `llmStatus === "running"` e non esiste ancora, altrimenti mostra un CTA in base allo stato (nessun modello scelto/server fermo/server in caricamento); espone il bottone "Rigenera" quando un riassunto esiste già. Il CTA "Avvia il server LLM" è `disabled` quando `!llmReady` (modello non scaricato), con il testo che diventa "Scarica un modello LLM dalle impostazioni..." e il `title` "Scarica prima il modello dalle impostazioni": senza modello `start_llm_server` fallirebbe comunque, stesso gate di `canStart` in `ServerControls`. Persiste su disco (`write_recording_notes`) una volta per riassunto completato e una volta per turno di chat completato, mai per token. Un `AbortController` per richiesta è abortito sia da un bottone "Interrompi" sia dal teardown dell'`$effect` (uscire dal dettaglio a metà streaming non deve lasciare occupato lo slot del server locale).

Un errore in chat non persiste il messaggio utente: resta in `pendingQuestion`, passato a `TranscriptChat` come `retryDraft` così l'utente può reinviare senza riscrivere la domanda. `TranscriptChat` è puramente presentazionale: lista messaggi, bolla di streaming, textarea con invio su Enter (Shift+Enter per andare a capo), disabilitata quando `disabled` (LLM non `"running"`) o `busy`.

`llm.ts` è l'unico file che conosce l'AI SDK: `createOpenAICompatible` col `fetch` di `@tauri-apps/plugin-http` (vedi `architecture.md`), `buildTranscriptContext` (righe `[m:ss] IO/INTERLOCUTORE` o testo grezzo, con cap e flag `truncated`), `streamSummary`/`streamChatReply` (wrapper di `streamText`), `parseSummary` (parser tollerante delle quattro sezioni marcate).

Il bottone dell'app: cinque varianti (`ghost`, `icon`, `solid`, `danger`, `primary`) come stringhe di utility Tailwind composte con `cn()` (le classi passate via `class` vincono sui conflitti grazie a tailwind-merge, es. per padding/size diversi a parità di variante). Tutte le varianti usano `inline-flex items-center justify-center` per garantire il perfetto allineamento del contenuto (testo o icone SVG). Non c'è nessuna libreria di primitivi UI.

`icon` è il pulsante rotondo compatto per azioni a icona singola (impostazioni, back, CTA in card e toolbar): `inline-flex items-center justify-center shrink-0 rounded-full border border-brand-cream/15 bg-brand-darker/85 size-8 p-0 text-ink-secondary shadow-raised backdrop-blur-sm`, con feedback tattile `active:scale-[0.95]` e contrasto pieno su hover (`hover:bg-brand-cream hover:text-brand-darker hover:border-brand-cream/30`). La combinazione `size-8 p-0` offre una dimensione base di 32px con zero padding, permettendo l'override pulito con `size-7` (28px) nelle toolbar compatte di `SettingsPanel` e `ServerControls` senza che padding residui decentrino le icone (es. `Play` in `ServerControls` include `translate-x-px` per la compensazione ottica del triangolo).

`solid` e `primary` sono i due bottoni di conferma: entrambi bianchi (`brand-cream`) con testo scuro e hover `brand-cream-dim`, distinti solo dalla taglia (`primary` è il CTA grande dell'onboarding). `danger` resta rosso pieno: il rosso distruttivo è convenzione, non palette di brand.

### `ServerControls.svelte`

Blocco stato + controlli di un server locale, usato due volte da `SettingsPanel` (STT e LLM). Prop: `title`, `status: ServerStatus`, `error: string | null`, `hint?: string | null` (mostrato nella card solo in assenza di errore), `loading`, `locked` (registrazione/trascrizione in corso), `canStart?: boolean` (gate extra su Avvia/Riavvia, default `true`), `dangerLabel` e le callback `onStart`/`onRestart`/`onStop`/`onDanger`. Stato mostrato: "Attivo" (`running`) / "Caricamento..." (`loading`) / "Fermo" (tutto il resto).

### `DownloadProgressBar.svelte`

Barra di avanzamento download modello, usata da `Onboarding` e da `SettingsPanel` (whisper + LLM). Prop: `progress: { step: string; pct: number }` (`step === "done"` → "Completato", altrimenti "Modello N%") e `trackClass?` per il colore della traccia (default `bg-brand-dark`; `Onboarding` passa `bg-brand-darker` perché sta su fondo `brand-dark`). Il riempimento è `brand-cream`.

## Styling

Tailwind CSS v4 con config CSS-first (niente `tailwind.config.js`): plugin `@tailwindcss/vite` e `@import "tailwindcss"` in `src/App.css`. Utility inline nel markup, nessun `<style>` nei componenti.

Palette in `@theme`, namespace `brand-*` per non collidere con i token base di Tailwind. I grigi sono **scuri e profondi con una nota di rosso caldo**: luminanza più profonda (~10%), con la componente rossa calda spostata (tinta sui 12-14°, con R dominante e G > B per evitare dominanti violacee o fredde), conferendo un sottotono caldo/ambra scura coerente con l'accento REC. Un token grigio aggiunto fuori da questa tinta stona con il resto.

| Token | Hex | Uso |
|---|---|---|
| `brand-dark` | `#211512` | sfondo app |
| `brand-darker` | `#170f0c` | superfici, card, pannelli |
| `brand-light` | `#947a72` | grigio chiaro: testo secondario, bolla utente in chat |
| `brand-lighter` | `#3f2923` | grigio medio: superficie rialzata, bottone di conferma disabilitato |
| `brand-lightest` | `#52362f` | gradino grigio più chiaro |
| `brand-cream` | `#f9f5f3` | testo su fondo scuro, bottoni di conferma |
| `brand-cream-dim` | `#ebe3df` | hover dei bottoni di conferma |
| `brand-ink` | `#0c0605` | testo su superfici chiare |

Il rosso non è più colore di brand: vive in due token separati, usati solo dalla registrazione (bottone REC, alone pulsante, timer).

| Token | Hex | Uso |
|---|---|---|
| `rec` | `#ab2b29` | REC a riposo |
| `rec-strong` | `#d23434` | REC in registrazione, timer, glow (`@keyframes pulse-rec`) |

La gerarchia del testo non usa alpha di `brand-cream`: quattro token `ink-*` derivati con `color-mix` (`ink-primary` 100%, `ink-secondary` 70%, `ink-muted` 50%, `ink-faint` 40%). Gli alpha frazionari di `brand-cream` (`/5`–`/20`) restano per bordi e superfici, mai per testo. I semafori di stato sono token (`ok`, `warn`, `danger`, `danger-strong` per la superficie del bottone distruzioni): badge, dot dell'indicatore e messaggi di errore li leggono, niente palette Tailwind grezza. Le ombre sono token (`shadow-raised` per gli elementi flottanti, `shadow-pop` per il logo onboarding, `shadow-rec`/`shadow-rec-active` per l'alone del REC). I due token REC sono stack doppi: sotto il glow `color-mix` sul colore di stato, un `inset 0 2px 0` bianco al 14% dà volume al cerchio senza introdurre un gradiente.

I campi di testo (settings e chat) mostrano il focus della tastiera con bordo e ring su `brand-light`, mai `outline` di default.

Le card di riga condividono la classe `.card` (in `App.css`, `@layer components`): `rounded-xl border border-brand-cream/10 bg-brand-darker px-5 py-4` in un solo posto (liste di registrazioni, note, dettaglio). Le durate animate sono token (`--animate-blink`, `--animate-pulse-rec`), così come lo stack mono per timer e duration (`--font-mono`).

Tema unico dark fisso: niente variante `dark:`, niente token semantici.

Alias `$lib` → `src/lib` dichiarato sia in `vite.config.ts` (`resolve.alias`) sia in `tsconfig.json` (`compilerOptions.paths`, mai `baseUrl`): non essendo SvelteKit non è automatico.

Icone dell'app in `src-tauri/icons/`: `bundle.icon` di `tauri.conf.json` referenzia `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.icns` e `icon.ico` (piattaforma Windows), mentre `icon.png` e `64x64.png` restano come sorgenti a risoluzione piena e non entrano nel bundle. `public/icon.png` (1024×1024) alimenta il logo in `Onboarding.svelte`, la favicon webview in `index.html` e il logo in `README.md`. L'icona per la barra dei menu macOS (`TrayIconBuilder` con `icon_as_template(true)`) è alimentata da `src-tauri/icons/tray-icon.png` (32×32 @2x, sagoma monocromatica su sfondo trasparente), rasterizzata dal master vettoriale `earwig.svg` (presente in `src-tauri/icons/` e `public/`).

## Lint e format

Biome (`biome.json`): `npm run lint` e `npm run lint:fix`. Le regole `noUnusedImports`/`noUnusedVariables` sono disattivate sui `.svelte` perché il parser non legge il markup e produce falsi positivi su simboli usati solo nel template.
