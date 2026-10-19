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
| `download_local_model` | — | `Result<(), String>` | Scarica il modello in streaming su `<model>.bin.part`, emette `download-progress`, fa rename atomico a fine download e aggiorna `local_ready` (informativo, vedi sopra) |
| `start_stt_server` | — | `Result<(), String>` | Spawna `whisper-server` se la porta è libera e attende fino a 60s. Se la porta è già occupata, il lockfile decide: nostro processo col modello atteso → adottato senza rispawn, nostro col modello diverso → terminato e rispawnato, altrimenti si usa quello che risponde (vedi [`architecture.md`](architecture.md)) |
| `stop_stt_server` | — | `Result<(), String>` | Killa il processo tracciato, attende che la porta 8080 si liberi (timeout 5s). Se lo slot è vuoto ma la porta è occupata, termina per pid solo un processo che il lockfile riconosce come nostro; altrimenti errore esplicito |
| `restart_stt_server` | — | `Result<(), String>` | `stop_stt_server` + `start_stt_server`; fallisce con lo stesso errore se lo stop fallisce |
| `delete_local_model` | — | `Result<(), String>` | Ferma il server (il processo tiene il `.bin` aperto), poi cancella il file del modello. Errore se lo stop fallisce |
| `check_stt_server` | — | `String` | `"running"` oppure `"stopped"` |
| `transcribe_recording` | `path: String` | `Result<TranscriptResult, String>` | POST a `/inference`, scrive `transcript.json` accanto al WAV; su errore scrive `transcript_error.json` (sidecar `{ message }`), su successo lo rimuove se presente. Riusata anche per il rilancio: nessuna firma dedicata |

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
| `start_recording` | — | `Result<(), String>` | Errore se già in corso, oppure se il modello non è scaricato o whisper-server non è in esecuzione (vedi `ensure_stt_ready` in [`architecture.md`](architecture.md)); altrimenti avvia mic e audio di sistema |
| `stop_recording` | — | `Result<String, String>` | Ferma, applica AEC, scrive il WAV, ritorna il path |
| `list_recordings` | — | `Result<Vec<RecordingEntry>, String>` | Scansiona la cartella Records, ordina per nome decrescente. `error` (messaggio da `transcript_error.json`) è letto solo quando `transcript` è assente |
| `import_audio_file` | — | `Result<Option<String>, String>` | Stessa guardia `ensure_stt_ready` di `start_recording`; poi picker, decodifica, salva come registrazione. `None` se annullato |

### Permessi OS (`commands/mod.rs` + `permissions.rs`)

| Command | Input | Output | Effetti |
|---|---|---|---|
| `check_screen_recording_permission` | — | `bool` | `CGPreflightScreenCaptureAccess`, sola lettura, nessun prompt |
| `open_permission_settings` | `pane: "microphone" \| "screen-recording"` | `Result<(), String>` | Apre il pannello Privacy & Security via `open x-apple.systempreferences:...` |

`pane` è convertito in un enum chiuso prima di toccare il comando di sistema: un valore non riconosciuto ritorna `Err` e non arriva mai a `open`.

Per il microfono non c'è un check di stato: l'unica API è `AVCaptureDevice.authorizationStatus`, Objective-C, e fare il bridging con `objc2` solo per un pallino non vale la complessità.

## Eventi

| Evento | Payload | Emesso da |
|---|---|---|
| `download-progress` | `{ step: "model" \| "done", pct: number }` | `download_local_model` |
| `llm-download-progress` | `{ step: "llm" \| "done", pct: number }` | `download_llm_model` |

Evento separato dal precedente invece di un nuovo `step` su `download-progress`: quell'evento ha già due listener indipendenti (`SettingsPanel` e `Onboarding`), e uno `step: "done"` condiviso farebbe scattare per errore, nel listener whisper di `SettingsPanel`, il codice che marca `localReady = true`.

## Tipi condivisi (`src/lib/types.ts`)

`SttSettings`, `RecordingEntry`, `TranscriptResult`, `TranscriptSegment`, `DownloadProgress`, `LlmDownloadProgress`, `ServerStatus`, `SttStatus`, `TranscriptStatus`, `NotesSummary`, `ChatRole`, `ChatMessage`, `RecordingNotes`, `HfModelSummary`, `HfGgufFile`, `HfModelDetail`.

`ServerStatus` = `"checking" | "starting" | "running" | "error" | "stopped" | "loading"`, stato di un server locale gestito dall'app (whisper o LLM). `"stopped"` è distinto da `"error"`: è l'esito di uno stop esplicito (impostazioni), non di un fallimento. `"loading"` è specifico dell'LLM (`llama-server` apre la porta prima di aver caricato il modello; whisper-server no, quindi non lo usa mai). `SttStatus` è un alias di `ServerStatus` per non far churnare `SttIndicator`/`SettingsPanel`.

`LlmDownloadProgress { step: "llm" | "done", pct: number }`: stessa forma di `DownloadProgress`, ma su un evento dedicato (vedi sopra).

`SttSettings.llmHfRepo`/`llmHfFile: string` (`""` = nessun modello scelto) sono il modello LLM selezionato dall'utente in Settings. `llmSizeBytes: number` è la size del file scelto (nota dal frontend al momento della selezione, da `HfGgufFile.size_bytes`), usata solo per etichettare il bottone di download. `llmReady: boolean` rispecchia `localReady`: ricalcolato da `get_stt_settings` verificando `llm_model_path` sul disco, mai la fonte di verità è il valore persistito.

`NotesSummary { text, key_points: string[], actions: string[], open_questions: string[], model, generated_at }`, `ChatMessage { id, role: ChatRole, content, at }` (`ChatRole` = `"user" | "assistant"`), `RecordingNotes { version, summary: NotesSummary | null, messages: ChatMessage[] }`: rispecchiano 1:1 le struct Rust in `commands/llm.rs` (snake_case, come `RecordingEntry`/`TranscriptResult`, non camelCase come `SttSettings`).

`HfModelSummary { id, downloads, license }` (risultato di ricerca), `HfGgufFile { filename, size_bytes }`, `HfModelDetail { gated, files: HfGgufFile[] }` (dettaglio repo).

`RecordingEntry.error: string | null` è il messaggio dell'ultimo fallimento di trascrizione (da `transcript_error.json`), sempre `null` quando `transcript` è presente.

`TranscriptStatus` = `"transcribed" | "pending" | "failed"`, stato derivato (non un campo persistito) di una `RecordingEntry`.

Helper di presentazione nello stesso file:

| Helper | Ruolo |
|---|---|
| `speakerInfo(speaker)` | Mappa la chiave grezza a `{ label, color }`: `"0"` → YOU blu, `"1"` → THEM verde, tutto il resto → Sconosciuto grigio |
| `groupSegments(segments)` | Accorpa segmenti consecutivi con lo stesso speaker |
| `formatDuration(ms)` | `hh:mm:ss`, per il cronometro di registrazione |
| `formatSeconds(s)` | `m:ss`, per i timestamp dei segmenti |
| `formatElapsed(ms)` | `12.4s` sotto il minuto, poi `m:ss` |
| `transcriptStatus(entry)` | `RecordingEntry` → `TranscriptStatus` (`transcript` presente → `"transcribed"`, altrimenti `error` presente → `"failed"`, altrimenti `"pending"`) |
| `transcriptStatusInfo(entry)` | `transcriptStatus(entry)` → `{ label, className }` per il badge a 3 stati di `RecordingsList` |

## Store (`src/lib/stores/`)

Moduli `.svelte.ts` singleton: le rune funzionano anche fuori dai componenti, quindi lo stato condiviso non deve più attraversare una catena di props. Ogni store espone un oggetto con getter (`get sttStatus()`, `get all()`, ...) invece delle variabili `$state` nude, perché una variabile esportata direttamente perderebbe la reattività all'import.

Le dipendenze vanno in una sola direzione, `recordings` → `session` → `servers`: mai al contrario, o si formano import circolari.

### `servers.svelte.ts`

Stato dei due server locali: `sttStatus`, `llmStatus` (`ServerStatus`), `modelReady`, `llmReady`.

`refreshStt(opts?: { attemptStart?: boolean }): Promise<SttSettings | null>` è il punto unico di riallineamento fra stato reale (whisper-server + modello sul disco) e stato mostrato in UI: legge `get_stt_settings` (aggiorna `modelReady` da `localReady`) e `check_stt_server`, poi imposta `sttStatus` di conseguenza (`"checking"` mentre gira, poi `"running"` / `"stopped"` / `"starting"`→`"running"` / `"error"`). Con `attemptStart: true` (default) un server non in esecuzione viene avviato, come al mount; con `attemptStart: false` un server fermo resta `"stopped"` invece di essere riavviato, il caso di chi lo ha appena fermato di proposito dalle impostazioni. Ritorna le `SttSettings` lette (o `null` in errore). Chiamata al mount, da `SettingsPanel` dopo ogni azione sul server o sulle impostazioni, e dalla `onContinue` di `Onboarding`; chiunque cambi lo stato del server o del modello la richiama invece di duplicare la logica di check/avvio.

`refreshLlm(opts?)` è il mirror per il server LLM: stessa forma, stessa fonte di verità (`get_stt_settings` + `check_llm_server`), ma **default invertito** — `attemptStart` è `false` qui contro il `true` di STT. Al mount si osserva soltanto: caricare un modello LLM da 1-5GB per una feature che molte sessioni non toccano mai non ha senso, a differenza di whisper che serve sempre. Aggiorna anche `llmReady` (modello LLM presente sul disco, ricalcolato a ogni chiamata): è il gate del bottone "Avvia il server LLM" in `TranscriptNotes`, e resta allineato da sé perché `SettingsPanel` richiama `refreshLlm` dopo download, cambio modello ed eliminazione della cache.

`llmStatus` non si assegna mai direttamente: passa da `setLlmStatus`, che accende un poll di `check_llm_server` ogni 2s finché lo stato è `"loading"`/`"starting"` e lo spegne appena esce. A differenza di whisper il tempo di avvio dell'LLM è imprevedibile e nessun secondo click dell'utente aggiornerebbe altrimenti la UI. Il poll vive nel modulo (`setInterval`), non in un `$effect` di un componente, così non dipende da chi è montato in quel momento.

### `session.svelte.ts`

Sessione di lavoro: `isRecording`, `isTranscribing`, `durationMs`, `transcribeMs`, `error`, più i derivati `locked`, `canRecord`, `recordGateReason`.

`canRecord` (`sttStatus === "running" && modelReady`, letti da `servers`) è il gate lato UI per registrazione e import, e rispecchia `ensure_stt_ready` lato backend (vedi [`architecture.md`](architecture.md)); `recordGateReason` è il messaggio in italiano quando `!canRecord`, che distingue "modello non scaricato" da "server non attivo". Quando `!canRecord`: il bottone REC resta abilitato per lo STOP (non blocca una registrazione già in corso) ma non per l'avvio, "o carica un file" è disabilitato, entrambi mostrano `recordGateReason` come `title`, e lo stesso testo appare come paragrafo sotto i bottoni. `startRecording`/`importFile` ripetono il controllo a runtime, non solo sull'attributo `disabled`, per coerenza con la guardia backend.

`locked` (`isRecording || isTranscribing`) è il lock condiviso: un solo flusso di trascrizione alla volta, e mentre è attivo lista e dettaglio disabilitano **ogni** riga, non solo quella interessata.

`runTranscription(path)` è la primitiva che tiene insieme lock e cronometro; la usano lo stop della registrazione, l'import e il rilancio in `recordings`. I cronometri (`durationMs`, `transcribeMs`) sono timer locali del frontend: il frontend è l'unico a poter avviare i due flussi, quindi conosce già l'istante di inizio e non serve interrogare il backend via IPC.

`importFile(): Promise<boolean>` ritorna `true` solo se un file è stato davvero importato e trascritto, così il chiamante sa se navigare alla lista; `false` copre anche l'annullamento del dialog, che non è un errore.

### `recordings.svelte.ts`

Cache di `list_recordings` (`all`, `loading`) con `load(force = false)` e `byName(name)`. È la fonte unica per lista e dettaglio: senza, il dettaglio raggiunto per URL dovrebbe rifare `list_recordings` per conto suo e le due copie andrebbero riallineate a mano dopo ogni rilancio.

`retryTranscription(folderPath: string): Promise<RecordingEntry[]>` esce subito se `session.locked` (guardia ridondante rispetto al `disabled` dei bottoni), altrimenti chiama `session.runTranscription` su `<folderPath>/recording.wav` ignorando un eventuale errore: l'esito è già persistito su disco da `transcribe_recording` (`transcript.json` o `transcript_error.json`), quindi la `load(true)` finale lo rilegge senza doverlo duplicare in un banner.

## Routing (`src/lib/routes.ts`)

[`svelte-spa-router`](https://github.com/ItalyPaleAle/svelte-spa-router) con hash routing. Sotto Tauri il documento è servito da un protocollo custom e non c'è nessun server che possa riscrivere i path: `#/...` è l'unica forma di URL che regge un reload.

| Rotta | Componente | Note |
|---|---|---|
| `/` | `Recorder.svelte` | schermata REC **e** lista registrazioni |
| `/detail/:id` | `RecordingDetail.svelte` | `id` = `entry.name`, cioè il nome della cartella: già univoco nella recordings dir e già filesystem-safe. Passa da `encodeURIComponent` in scrittura e `decodeURIComponent` in lettura |
| `/settings` | `SettingsPanel.svelte` | |
| `*` | `Recorder.svelte` | un hash sconosciuto non lascia la finestra vuota |

Non esiste una rotta per la lista: le registrazioni stanno in home sotto il bottone REC (`RecordingsList.svelte`), così arrivarci non costa una navigazione.

Navigazione con `push` (`svelte-spa-router`), tranne due casi: le impostazioni chiudono con `pop()`, perché si aprono da qualsiasi schermata e devono tornare da dove si è arrivati; il dettaglio con un `id` inesistente usa `replace("/")`, per non lasciare in history una rotta morta.

Nessuna route è lazy-loaded: sono tre componenti già nel bundle, il code splitting non ripagherebbe l'attesa alla prima navigazione.

### Layout delle pagine

Il guscio in `App.svelte` è `flex h-screen flex-col overflow-hidden`, senza padding né centraggio: ogni rotta decide la propria larghezza e lo scroll vive dentro la rotta, così l'header di pagina può restare `sticky`.

Le due pagine interne hanno la stessa struttura: `PageHeader` in cima, poi un contenitore `flex-1 overflow-y-auto px-6 pt-5 pb-18` a tutta larghezza. Il `pb-18` lascia passare la pillola di stato, `fixed` in basso a sinistra.

La home è divisa in due zone verticali: l'hero REC (`shrink-0`, non scrolla mai) e sotto la lista in `flex-1 overflow-y-auto`, che ha lo stesso `pb-18`. La lista si ferma a `max-w-200` (800px) centrata: righe più larghe allontanerebbero troppo il nome dal badge di stato.

La larghezza piena vale per le impostazioni. Fanno eccezione due punti, per leggibilità: il testo della trascrizione e le note stanno in una colonna `max-w-[72ch]` centrata, e la griglia delle impostazioni si ferma a `max-w-400` (1600px) — oltre, le sue due colonne si stirerebbero lasciando una voragine in mezzo.

### `PageHeader.svelte`

Header comune alle rotte interne: bottone back solo-icona (`ArrowLeft`, `variant="icon"`) sempre nella stessa posizione in alto a sinistra, titolo, e uno snippet `actions` opzionale allineato a destra. `sticky top-0` con `backdrop-blur`, così resta visibile mentre il contenuto scorre sotto.

Prop: `title: string`, `onBack: () => void`, `actions?: Snippet`.

Il bottone impostazioni **non** sta qui: vive in `Recorder.svelte`, `fixed` in alto a destra. Nelle pagine interne si sovrapporrebbe alle azioni dell'header.

## Componenti Svelte

Svelte 5 con le rune (`$state`, `$derived`, `$effect`, `$props`), un componente per file sotto `src/lib/`. Lo stato condiviso sta nelle store qui sopra; le props restano per ciò che è locale a un singolo albero. I componenti-route ricevono dal router solo `params`: tutto il resto lo leggono dalle store.

### `App.svelte` (root)

Guscio dell'app, senza stato proprio: monta `<Router>` e `SttIndicator` (pillola di stato, `fixed` in basso a sinistra, visibile su ogni rotta).

Rendering condizionato da `servers.modelReady`, valutato allo stesso modo indipendentemente dal motivo per cui il modello manca (primo avvio mai configurato, o modello eliminato in un secondo momento dalle impostazioni): se `!modelReady` l'intera UI normale — router compreso — è sostituita da `Onboarding` a schermo intero, nessun ramo separato per i due casi.

Flusso:
1. Mount → `servers.refreshStt()` + `servers.refreshLlm()`; se `!modelReady` si mostra `Onboarding` al posto della UI normale
2. `Onboarding` scarica il modello e a fine download invoca `onContinue` → `refreshStt()` → `modelReady` torna `true` → si torna automaticamente alla UI normale
3. REC → `session.startRecording()` → `start_recording`
4. STOP → `session.stopRecording()` → `stop_recording` → `transcribe_recording`
5. "o carica un file" (solo a riposo) → `session.importFile()` → se ritorna `true`, `recordings.load(true)`
6. Click su una riga della lista in home → `push("/detail/<name>")`
7. Rilancio trascrizione (da `RecordingsList` o `RecordingDetail`) → `recordings.retryTranscription(folderPath)`

Il riallineamento manuale dell'entry aperta dopo un rilancio non serve più: lista e dettaglio derivano entrambi da `recordings`, quindi la `load(true)` finale li aggiorna insieme.

### `Recorder.svelte`

La home: hero REC in alto e `RecordingsList` sotto. Bottone REC/STOP, "o carica un file", cronometro, stato della trascrizione ed eventuale errore: tutto letto da `session`, nessuna prop. Ospita anche il bottone impostazioni (`fixed` in alto a destra).

Dopo uno STOP o un import riusciti chiama `recordings.load(true)`: la lista sta nella stessa schermata e senza quel refresh la registrazione appena fatta non comparirebbe. Il refresh vive qui e non dentro `session` perché la direzione delle dipendenze fra store è `recordings → session`, non il contrario.

### `RecordingsList.svelte`

Lista delle registrazioni, puramente presentazionale: nessuna prop, nessun header, nessuna larghezza propria (la decide chi la monta). Fa `recordings.load()` in un `$effect` al mount. Era la rotta `/list`, prima che la lista si trasferisse in home.

Stato vuoto: card con bordo tratteggiato, icona `AudioLines`, titolo e hint ("Premi REC...") — non una riga ridotta a placeholder, perché è il primo volto della home per un utente nuovo.

Ogni riga non è un unico `<button>`: è un `<div>` che contiene un `<button>` (nome, tempo di trascrizione, badge a 3 stati da `transcriptStatusInfo`, naviga a `/detail/<name>`) e un `Button` "Rilancia" separato (nesting di bottoni non è HTML valido).

Due blocchi distinti, non uno solo:
- la riga è cliccabile sempre tranne durante una registrazione (`session.isRecording`): aprire il dettaglio porterebbe via dalla home, dove vivono stop e cronometro. Durante la sola trascrizione resta cliccabile
- "Rilancia" compare solo dove `transcriptStatus(entry) !== "transcribed"` ed è `disabled` su `session.locked` (registrazione **o** trascrizione, perché due trascrizioni insieme non si possono). Su una registrazione già trascritta il rilancio resta disponibile dal dettaglio

### `SttIndicator.svelte`

Indicatore di stato flottante in basso a sinistra, presente su ogni rotta tranne `/settings` (dove lo stato del server è già mostrato dentro le sezioni `ServerControls`: duplicarlo a schermo è rumore). Unica prop: `status: SttStatus`.

| Stato | Dot | Label |
|---|---|---|
| `checking`, `starting` | `warn` lampeggiante | "Avvio..." |
| `running` | `ok` attenuato | "Server attivo" |
| `error` | `danger` | "Server non disponibile" |
| `stopped` | `brand-light` (idle) | "Server fermo" |

Nessuno stato ha sfondo o ombra: l'indicatore è un dettaglio di servizio, il colore semantico vive solo sul dot e il testo è `brand-cream/50` uniforme. `SttStatus` esclude `loading` (esiste solo per llama-server, che apre la porta a modello da caricare: whisper-server no), quindi l'indicatore STT non lo conosce e mostra solo lo stato STT, non quello LLM (vedi `architecture.md`).

### `Onboarding.svelte`

View a schermo intero (non un modal), mostrata da `App.svelte` al posto della UI normale quando `!modelReady`. Logo (`public/icon.png`), titolo e sottotitolo fissi, poi tre stati in sequenza: CTA "Scarica il modello" → progress bar sull'evento `download-progress` (stesso comando `download_local_model` e stesso evento di `SettingsPanel`) con il messaggio "prenditi un caffè" visibile solo mentre `downloading` è vero → messaggio finale e bottone "Continua". Nessun bottone "salta": l'unica uscita è un download riuscito.

Prop: `onContinue: () => void`, chiamata dal bottone "Continua"; `App.svelte` la collega a `servers.refreshStt()` così `modelReady` si aggiorna e il rendering torna da solo alla UI normale, senza che `Onboarding` gestisca la propria visibilità.

### `SettingsPanel.svelte`

Rotta `/settings` con permessi OS, download dei modelli, controllo server STT/LLM e percorsi. Corpo a griglia (`grid-cols-1 md:grid-cols-2`, tetto `max-w-400`); il `PageHeader` ha solo il back, non c'è un bottone Salva. Colonna sinistra (core, sempre rilevante): Permessi, Modello Whisper, Server STT, Cartella registrazioni. Colonna destra (opzionale, additiva): Modello LLM, Server LLM — stessa distinzione già in [`architecture.md`](architecture.md) fra STT (sempre necessario) e LLM (riassunto/chat, lazy). Sotto il breakpoint `md` (768px) le due colonne si impilano in una sola. La sezione permessi viene per prima perché blocca tutto il resto.

Nessuna prop: era un modal con otto props passate da `App.svelte`, ora legge `servers` e `session` direttamente. Il back chiude con `pop()`, non con una callback del genitore. Non serve un bottone Salva: l'unico campo persistito è la scelta del modello LLM, salvata subito da `set_llm_model` lato Rust, mentre `localReady`/`llmReady` vengono ricalcolati da disco ad ogni `get_stt_settings`.

Le sezioni server STT e LLM sono due istanze di `ServerControls.svelte` (vedi sotto). Tutti i comandi server passano da due helper interni (`runStt(cmd, opts?)` / `runLlm(cmd)`: busy flag, `invoke`, refresh dello stato in `App.svelte`, errore nella card di stato); lato STT:
- **Avvia** → `start_stt_server`, poi `servers.refreshStt()`
- **Riavvia** → `restart_stt_server`, poi `servers.refreshStt()`
- **Spegni** → `stop_stt_server`, poi `servers.refreshStt({ attemptStart: false })` (NON riavvia automaticamente)
- **Elimina modello** → `delete_local_model`, poi `servers.refreshStt()` (aggiorna `modelReady` a `false`, che fa ricomparire `Onboarding` al posto del router) e, se non c'è errore, `pop()`

Sezione "Modello LLM": campo di ricerca con debounce (~400ms, query di default "instruct" a campo vuoto). La prima fetch è lazy: parte al primo focus del campo, non all'apertura del pannello (chi apre le impostazioni per i permessi non costa una chiamata a Hugging Face). Poi → `search_hf_models` → lista repo (nome, licenza dai tag, download) → click espande e chiama `get_hf_model_files` (i file GGUF divisi in shard, `<nome>-NNNNN-of-NNNNN.gguf`, sono filtrati via lato Rust: fuori scope, vedi `DEVLOG.md`) → lista file `.gguf` con dimensione reale e un badge "consigliato/pesante per la tua RAM" (euristica su `get_system_memory_gb`, mai bloccante) → click su un file chiama `set_llm_model` (con la size) e aggiorna `settings` in locale (`llmReady` torna `false` finché il download non completa). Repo `gated` mostrato ma non selezionabile (lucchetto + messaggio, nessun flusso di login HF).

Subito sotto il box "Selezionato" c'è il bottone di download (mirror di quello whisper: stessa progress bar, stesso stile), con 4 stati:

| Condizione | Label |
|---|---|
| nessun modello selezionato | bottone non renderizzato |
| download in corso | "Download in corso..." (disabled) |
| selezionato, non ancora scaricato | "Scarica modello (X GB)" |
| già scaricato (`llmReady`) | "Scarica di nuovo (X GB)" |

Il download (`download_llm_model`) ascolta l'evento dedicato `llm-download-progress`; a `step: "done"` ricarica `settings` da `get_stt_settings` (non setta `llmReady` in ottimismo: la verità è il file su disco) e richiama `servers.refreshLlm`.

Sezione "Server LLM" sotto: seconda istanza di `ServerControls` con `llmStatus`/i comandi `*_llm_server`; Avvia/Riavvia sono `disabled` anche quando `!settings.llmReady` (prop `canStart`, con il messaggio "Scarica prima il modello." come `hint` nella card di stato), e il bottone danger è "Elimina modelli scaricati" (`clear_llm_cache`, cancella sia i modelli scaricati da earwig sia la vecchia cache nativa di llama-server). La scelta di un modello dalla ricerca HF (`selectModel`) chiama anche `servers.refreshLlm({ attemptStart: false })`: il nuovo modello non è sul disco, e senza quel giro la store resterebbe con un `llmReady` stale che tiene abilitato il CTA del dettaglio trascrizione. `sttLoading`/`sttError` e `llmLoading`/`llmError` sono stati locali separati per sezione: un errore sul server LLM non disabilita i controlli whisper e viceversa. I tre box "Percorso"/"Selezionato" sono un unico snippet Svelte locale (`pathBox`).

### `RecordingDetail.svelte`, `TranscriptView.svelte`

Dettaglio con reveal della cartella; rendering della trascrizione raggruppata per speaker con barra colorata a sinistra.

`RecordingDetail` è una rotta e non riceve props dal genitore oltre a `params.id`: legge le registrazioni da `recordings` e il lock da `session.locked`.

In `RecordingDetail` un bottone "Rilancia trascrizione" nell'header (stesso stile di "Apri cartella") chiama `recordings.retryTranscription`: l'aggiornamento torna da solo, perché `entry` è un `$derived` su `recordings.byName(id)`. Finché `recordings.load()` non ha risposto il componente mostra "Caricamento...", non "non trovata": arrivando qui direttamente dall'URL la store è ancora vuota e l'assenza non significherebbe nulla. A `load()` conclusa, un `id` senza riscontro (registrazione cancellata) fa `replace("/")`. Il corpo mostra `TranscriptView` se `entry.transcript` è presente, altrimenti il messaggio d'errore in rosso se `entry.error` è presente, altrimenti "Trascrizione non ancora disponibile." Sotto la card trascrizione, `TranscriptNotes` riceve `entry` e lo stato LLM letto da `servers`.

### `TranscriptNotes.svelte`, `TranscriptChat.svelte`, `src/lib/llm.ts`

Riassunto e chat locale sulla trascrizione di una registrazione (vedi `architecture.md` per il flusso completo). `TranscriptNotes` è l'orchestrazione: carica `read_recording_notes` al mount, genera il riassunto in streaming (`streamSummary` da `llm.ts`) automaticamente se `llmStatus === "running"` e non esiste ancora, altrimenti mostra un CTA in base allo stato (nessun modello scelto/server fermo/server in caricamento); espone il bottone "Rigenera" quando un riassunto esiste già. Il CTA "Avvia il server LLM" è `disabled` quando `!llmReady` (modello non scaricato), con il testo che diventa "Scarica un modello LLM dalle impostazioni..." e il `title` "Scarica prima il modello dalle impostazioni": senza modello `start_llm_server` fallirebbe comunque, stesso gate di `canStart` in `ServerControls`. Persiste su disco (`write_recording_notes`) una volta per riassunto completato e una volta per turno di chat completato, mai per token. Un `AbortController` per richiesta è abortito sia da un bottone "Interrompi" sia dal teardown dell'`$effect` (uscire dal dettaglio a metà streaming non deve lasciare occupato lo slot del server locale).

Un errore in chat non persiste il messaggio utente: resta in `pendingQuestion`, passato a `TranscriptChat` come `retryDraft` così l'utente può reinviare senza riscrivere la domanda. `TranscriptChat` è puramente presentazionale: lista messaggi, bolla di streaming, textarea con invio su Enter (Shift+Enter per andare a capo), disabilitata quando `disabled` (LLM non `"running"`) o `busy`.

`llm.ts` è l'unico file che conosce l'AI SDK: `createOpenAICompatible` col `fetch` di `@tauri-apps/plugin-http` (vedi `architecture.md`), `buildTranscriptContext` (righe `[m:ss] IO/INTERLOCUTORE` o testo grezzo, con cap e flag `truncated`), `streamSummary`/`streamChatReply` (wrapper di `streamText`), `parseSummary` (parser tollerante delle quattro sezioni marcate).

### `Button.svelte`

Il bottone dell'app: cinque varianti (`ghost`, `icon`, `solid`, `danger`, `primary`) come stringhe di utility Tailwind composte con `cn()` (le classi passate via `class` vincono sui conflitti grazie a tailwind-merge, es. per padding/size diversi a parità di variante). Non c'è nessuna libreria di primitivi UI.

`solid` e `primary` sono i due bottoni di conferma: entrambi bianchi (`brand-cream`) con testo scuro e hover `brand-cream-dim`, distinti solo dalla taglia (`primary` è il CTA grande dell'onboarding). `danger` resta rosso pieno: il rosso distruttivo è convenzione, non palette di brand.

### `ServerControls.svelte`

Blocco stato + controlli di un server locale, usato due volte da `SettingsPanel` (STT e LLM). Prop: `title`, `status: ServerStatus`, `error: string | null`, `hint?: string | null` (mostrato nella card solo in assenza di errore), `loading`, `locked` (registrazione/trascrizione in corso), `canStart?: boolean` (gate extra su Avvia/Riavvia, default `true`), `dangerLabel` e le callback `onStart`/`onRestart`/`onStop`/`onDanger`. Stato mostrato: "Attivo" (`running`) / "Caricamento..." (`loading`) / "Fermo" (tutto il resto).

### `DownloadProgressBar.svelte`

Barra di avanzamento download modello, usata da `Onboarding` e da `SettingsPanel` (whisper + LLM). Prop: `progress: { step: string; pct: number }` (`step === "done"` → "Completato", altrimenti "Modello N%") e `trackClass?` per il colore della traccia (default `bg-brand-dark`; `Onboarding` passa `bg-brand-darker` perché sta su fondo `brand-dark`). Il riempimento è `brand-cream`.

## Styling

Tailwind CSS v4 con config CSS-first (niente `tailwind.config.js`): plugin `@tailwindcss/vite` e `@import "tailwindcss"` in `src/App.css`. Utility inline nel markup, nessun `<style>` nei componenti.

Palette in `@theme`, namespace `brand-*` per non collidere con i token base di Tailwind. I grigi sono **caldi**: stessa luminanza dei neutri puri ma con la componente rossa sopra la blu di 3-6 punti. Un token grigio aggiunto fuori da questa tinta stona con il resto.

| Token | Hex | Uso |
|---|---|---|
| `brand-dark` | `#1e1c19` | sfondo app |
| `brand-darker` | `#141311` | superfici, card, pannelli |
| `brand-light` | `#94908a` | grigio chiaro: testo secondario, bolla utente in chat |
| `brand-lighter` | `#3e3b37` | grigio medio: superficie rialzata, bottone di conferma disabilitato |
| `brand-lightest` | `#4e4a45` | gradino grigio più chiaro |
| `brand-cream` | `#f7f5f1` | testo su fondo scuro, bottoni di conferma |
| `brand-cream-dim` | `#e8e4de` | hover dei bottoni di conferma |
| `brand-ink` | `#0b0a08` | testo su superfici chiare |

Il rosso non è più colore di brand: vive in due token separati, usati solo dalla registrazione (bottone REC, alone pulsante, timer).

| Token | Hex | Uso |
|---|---|---|
| `rec` | `#ab2b29` | REC a riposo |
| `rec-strong` | `#d23434` | REC in registrazione, timer, glow (`@keyframes pulse-rec`) |

La gerarchia del testo non usa alpha di `brand-cream`: quattro token `ink-*` derivati con `color-mix` (`ink-primary` 100%, `ink-secondary` 70%, `ink-muted` 50%, `ink-faint` 40%). Gli alpha frazionari di `brand-cream` (`/5`–`/20`) restano per bordi e superfici, mai per testo. I semafori di stato sono token (`ok`, `warn`, `danger`, `danger-strong` per la superficie del bottone distruzioni): badge, dot dell'indicatore e messaggi di errore li leggono, niente palette Tailwind grezza. Le ombre sono token (`shadow-raised` per gli elementi flottanti, `shadow-pop` per il logo onboarding, `shadow-rec`/`shadow-rec-active` per l'alone del REC).

I campi di testo (settings e chat) mostrano il focus della tastiera con bordo e ring su `brand-light`, mai `outline` di default.

Le card di riga condividono la classe `.card` (in `App.css`, `@layer components`): `rounded-xl border border-brand-cream/10 bg-brand-darker px-5 py-4` in un solo posto (liste di registrazioni, note, dettaglio). Le durate animate sono token (`--animate-blink`, `--animate-pulse-rec`), così come lo stack mono per timer e duration (`--font-mono`).

Tema unico dark fisso: niente variante `dark:`, niente token semantici.

Alias `$lib` → `src/lib` dichiarato sia in `vite.config.ts` (`resolve.alias`) sia in `tsconfig.json` (`compilerOptions.paths`, mai `baseUrl`): non essendo SvelteKit non è automatico.

Icone dell'app in `src-tauri/icons/`: `bundle.icon` di `tauri.conf.json` referenzia `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.icns` e `icon.ico` (piattaforma Windows), mentre `icon.png` e `64x64.png` restano come sorgenti a risoluzione piena e non entrano nel bundle.

## Lint e format

Biome (`biome.json`): `npm run lint` e `npm run lint:fix`. Le regole `noUnusedImports`/`noUnusedVariables` sono disattivate sui `.svelte` perché il parser non legge il markup e produce falsi positivi su simboli usati solo nel template.
