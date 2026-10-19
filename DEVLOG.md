# Devlog — Heedm

Journal condiviso di implementazione. Ogni sessione aggiunge un'entry in cima.

Formato:
```
## YYYY-MM-DD hh-mm — <titolo>
**Obiettivo:** ...
**Fatto:** ...
**Decisioni:** ...
**Prossimi passi:** ...
```

---

## 2026-10-19 — Migrazione config di progetto da Claude Code a OpenCode

**Obiettivo:** il progetto si guida con OpenCode, non con Claude Code: config, skill e regola documentazione migrati, nessun residuo Claude nel repo.

**Fatto:**
- `CLAUDE.md` rinominato `AGENTS.md` (titolo e riferimenti depotenziati da "Claude")
- Permessi da `.claude/settings.json` migrati in `opencode.json` (regole `shell` con resource/effect)
- Regola documentazione da hook `PreToolUse` Claude a pre-commit hook git: `githooks/pre-commit` + `core.hooksPath githooks`, bypass invariato `SKIP_DOCS=1`
- Skill `run-heedm` e `fresh-install` spostate in `.opencode/skills/`
- Worktree `design+dark-gray-palette` rimosso da `.claude/worktrees/` (branch intatto), directory `.claude/` eliminata, entry legacy dal `.gitignore` tolta
- `docs/onboarding.md` aggiornato al nuovo nome del file istruzioni

**Decisioni:**
- Il check docs è un hook git e non un plugin OpenCode: vincola chiunque committi, agente o umano, senza plugin TS da mantenere; costo, `core.hooksPath` va impostato a mano in ogni clone (annotato in `AGENTS.md`)

---

## 2026-08-24 — Lista registrazioni in home

**Obiettivo:** vedere le registrazioni precedenti appena sotto il bottone REC, senza passare da un'altra pagina.

**Fatto:**
- Rotta `/list` eliminata: `RecordsList.svelte` diventa `RecordingsList.svelte`, lista presentazionale senza header né larghezza propria, montata dentro `Recorder.svelte`
- Home divisa in due zone: hero REC `shrink-0` in alto (bottone invariato a 120px) e lista `flex-1 overflow-y-auto` sotto, capped a `max-w-200`
- Bottone impostazioni spostato da `SttIndicator` (basso a destra) a `Recorder` (alto a destra), dove stava il bottone lista che è sparito; `SttIndicator` resta la sola pillola di stato
- `RecordingDetail`: back e `replace` dell'id inesistente puntano a `/` invece che a `/list`

**Decisioni:**
- Un solo posto per la lista, non due: con l'elenco in home, una `/list` a schermo intero sarebbe stata una seconda copia da tenere allineata
- **Il bottone impostazioni non può stare nel guscio**: `fixed` in alto a destra collide con le azioni di `PageHeader` nelle pagine interne. Stessa lezione già imparata dal bottone lista, ora scritta in `reference.md`
- Righe bloccate solo durante la registrazione, non durante la trascrizione: navigare via dalla home mentre si registra nasconde stop e cronometro, mentre aprire una registrazione vecchia durante una trascrizione non interferisce. Il "Rilancia" invece resta legato a `session.locked` (due trascrizioni insieme non si possono): dentro la stessa riga il click passa e il rilancio no, ed è voluto
- "Rilancia" solo sulle righe non trascritte: su una schermata che è anche la home, un bottone per riga era rumore. Sul già trascritto resta il bottone del dettaglio
- Il refresh della lista dopo STOP/import sta in `Recorder.svelte` e non in `session`: la direzione delle dipendenze fra store è `recordings → session`

**Prossimi passi:**
- Giro a click da verificare a mano: REC → la nuova riga compare in lista senza refresh, click su una riga durante una trascrizione, rilancio di una fallita
- Restano in coda le due voci dell'entry sul routing (cartelle per i componenti, disposizione impostazioni)

---

## 2026-08-24 — Grigi caldi

**Obiettivo:** palette meno fredda, senza cambiare i rapporti di contrasto già tarati.

**Fatto:** tutti gli otto token `brand-*` in `@theme` rifatti a pari luminanza con la componente rossa sopra la blu di 3-6 punti (prima era il contrario, tinta bluastra).

**Decisioni:**
- I due token `rec` restano invariati: il rosso è già caldo
- La tinta è ora un vincolo scritto in `reference.md`: un grigio aggiunto fuori da questa famiglia stona con il resto, e la tabella dei token è il posto dove uno se ne accorge

**Prossimi passi:** `#6b7280` per lo speaker "Sconosciuto" in `types.ts` è l'unico grigio rimasto fuori palette.

---

## 2026-08-24 — Routing SPA e pagine a schermo intero

**Obiettivo:** lista e impostazioni a tutta finestra invece che, rispettivamente, una colonna da 680px e un modal centrato; e una navigazione vera al posto delle tre variabili in `App.svelte`.

**Fatto:**
- `svelte-spa-router` 5.1.1 con hash routing: quattro rotte (`/`, `/list`, `/detail/:id`, `/settings`) più catch-all su `/`
- Tre store `.svelte.ts` (`servers`, `session`, `recordings`) al posto delle props scese da `App.svelte`; `App.svelte` resta un guscio senza stato proprio
- Schermata REC estratta in `Recorder.svelte`, header comune in `PageHeader.svelte` (back solo-icona in alto a sinistra, titolo, azioni di pagina)
- `SettingsPanel` da modal `fixed inset-0` a rotta; il bottone Salva passa nell'header
- `RecordingDetail` risolve l'entry dal nome cartella nell'URL invece di riceverla come prop

**Decisioni:**
- Libreria invece di uno state machine interno: la scelta era fra `svelte-spa-router` (58k download/settimana, aggiornata, peer dep su Svelte 5) e `@mateothegreat/svelte5-router` (1.4k, un manutentore). Tinro, svelte-navigator e svelte-micro sono fermi a Svelte 3/4; Routify è filesystem-based e sovradimensionato per quattro schermate
- **Hash routing, non history**: sotto Tauri il documento è servito da un protocollo custom e non c'è nessun server che possa riscrivere i path. `#/...` è l'unica forma che regge un reload
- L'id di rotta del dettaglio è `entry.name` (nome cartella): già univoco nella recordings dir e già filesystem-safe, nessun id sintetico da inventare
- La store `recordings` come fonte unica elimina il riallineamento manuale di `selectedEntry` dopo un rilancio, che era un bug latente: lista e dettaglio derivano ora dallo stesso array
- Dipendenze fra store in una sola direzione (`recordings` → `session` → `servers`), per non aprire la porta agli import circolari
- Due eccezioni volute alla larghezza piena: testo trascrizione a `72ch` (leggibilità) e griglia impostazioni a 1600px (oltre, le due colonne si stirano)
- Il poll di `/health` dell'LLM passa da `$effect` in un componente a `setInterval` nella store: non deve più dipendere da chi è montato

**Prossimi passi:**
- **Riorganizzare `src/lib/` in cartelle** invece di tenere tutti i componenti sfusi: `stores/` è il primo passo, mancano le altre (rotte, componenti condivisi, feature). Richiesto esplicitamente
- **Rivedere la disposizione delle impostazioni**: oggi sono ancora le due colonne del vecchio modal (core a sinistra, LLM a destra) messe dentro una pagina intera. Da ripensare per la larghezza che hanno adesso, non da adattare
- Timer di registrazione visibile anche fuori dalla rotta `/`: con le pagine intere si può entrare nelle impostazioni mentre si registra e perdere di vista tempo e stop
- Giro a click da verificare a mano (lista → dettaglio → impostazioni → back, e reload con hash su `/detail/:id`): niente `cliclick` e UI scripting negato su questa macchina

---

## 2026-08-24 — Design system dal rosso al grigio scuro

**Obiettivo:** togliere il rosso come colore di brand e portare l'interfaccia su una scala di grigi scuri neutri, con i bottoni di conferma bianchi.

**Fatto:**
- `App.css`: i 7 token `--color-brand-*` ridefiniti su grigio neutro (`#1b1b1d` fondo, `#121213` superfici, `#3a3a3d` medio, `#4a4a4e`, `#8e8e93` chiaro, `#f5f5f7` testo, `#0a0a0b` ink); nuovo `--color-brand-cream-dim` (`#e5e5e7`) per l'hover dei bottoni bianchi
- Nuovi token `--color-rec` / `--color-rec-strong` con i due rossi di prima (`#ab2b29`, `#d23434`): li usano solo il bottone REC, il suo alone (`@keyframes pulse-rec`, ora via `color-mix` sul token invece di `rgb()` letterale) e il timer di registrazione
- `Button.svelte`: `solid` da fondo rosso a fondo `brand-cream` con testo `brand-ink`, hover `brand-cream-dim`, disabled `brand-lighter` + testo attenuato; `primary` allineato sullo stesso hover
- Barra di download (`DownloadProgressBar`) e bolla dei messaggi utente (`TranscriptChat`) fuori dal rosso: riempimento `brand-cream`, bolla `brand-light` con testo `brand-ink`

**Decisioni:**
- Il rosso resta ma cambia ruolo: da colore di brand a segnale di stato della sola registrazione. Da qui i token `rec`/`rec-strong` separati invece di riusare `brand-lighter`/`brand-lightest`, che erano rossi solo per coincidenza di palette
- `danger` resta rosso pieno: convenzione universale per l'azione distruttiva, e non convive mai con il REC nella stessa schermata (REC in home, Elimina nel dettaglio)
- Stati semantici server (verde/ambra/rosso) e `SPEAKER_COLORS` invariati, coerentemente con la scelta originale di tenerli fuori dal design system
- `brand-cream` mantiene il nome benché ora sia un bianco neutro: rinominarlo tocca 100+ occorrenze e sarebbe un refactor a parte
- `brand-lightest` resta definito ma non ha più utilizzi nel markup (era il rosso del REC)

---

## 2026-08-24 — Lockfile con pid: chiudere il ciclo degli orfani

**Obiettivo:** un server sopravvissuto a una morte violenta dell'app (SIGKILL sul padre, Force Quit, logout) tiene la porta e non è distinguibile da un processo di terzi. L'app lo adottava alla cieca e poi rifiutava di fermarlo ("chiudilo manualmente"), lasciando l'utente fuori dai propri controlli. Il caso non si previene (nessun codice del padre gira): si rimedia alla sessione dopo.

**Fatto:**
- `commands/mod.rs`: `server_lock_path` → `<app_data_dir>/run/<nome>.json`
- `commands/server.rs`: `ServerLock { pid, bin, started_at, model }` scritto da `spawn_tracked`; `read_valid_lock` che valida via `ps -p <pid> -o lstart=,comm=`; `kill_pid` (extern `kill(2)` a mano); `try_adopt_running_server` come punto di decisione dell'avvio; `stop_tracked_server` e `kill_tracked` ora lock-aware
- `stt.rs` / `llm.rs`: il ramo `if port_is_open { return Ok(()) }` in testa allo start è sostituito da `try_adopt_running_server`

**Decisioni:**
- **Adozione invece di kill-e-rispawn**: riusare l'orfano evita di ricaricare il modello (~15s per whisper, di più per un GGUF grosso). Costo: lo slot `Child` resta vuoto per sempre, quindi ogni percorso di terminazione deve saper agire per pid
- **Verifica d'identità su due criteri, binario e istante di avvio**: il pid da solo non basta, l'OS li ricicla e un lock vecchio può puntare a un processo innocente. Se uno dei due non torna, il lock è spazzatura: si cancella e non parte nessun segnale
- **`model` nel lock**: un `llama-server` orfano può avere in RAM un GGUF diverso da quello ora selezionato. Senza questo campo l'utente parlerebbe col modello vecchio credendolo nuovo, e i riassunti finirebbero in `notes.json` col nome sbagliato. Alternativa scartata: chiedere a `/v1/models`, che vale solo per l'LLM (whisper non espone nulla di equivalente)
- **Lo start resta permissivo** quando non c'è un lock valido: usa comunque il processo sulla porta, come ha sempre fatto. Rifiutare sarebbe stata una regressione per chi avvia un server a mano per debug (skill `run-heedm`). Il lock aggiunge capacità, non divieti
- **`kill(2)` dichiarato a mano**, come `sysctlbyname` e `CGPreflightScreenCaptureAccess`: `libc` resta una dipendenza solo transitiva
- **Limiti accettati e documentati**: `lstart` dipende dal locale (un cambio fa fallire il confronto → l'app non adotta, degrada verso la prudenza); due istanze in parallelo condividono il lock

**Verificato (a runtime, con sonde temporanee poi rimosse):**
- Adozione: crash simulato con `kill -9` sull'app → orfano + lock → riavvio → stesso pid riusato, nessun rispawn
- Chiusura: l'orfano adottato (che l'app non ha come `Child`) viene terminato via lockfile, lock cancellato, porta libera. È il ramo senza il quale il ciclo si autoalimenterebbe
- Sicurezza: lock costruito ad arte su un processo innocente vivo, con `started_at` **corretto** e binario diverso → nessun segnale, processo intatto, lock cancellato come stale. Anche il processo estraneo che occupava la porta è rimasto intatto
- Modello diverso: lock valido con `model` alterato → vecchio pid terminato, nuovo server avviato, lock riscritto

**Prossimi passi:** verifica del flusso LLM a runtime (serve un GGUF sul disco: il meccanismo è lo stesso, ma il ramo `model` non è stato esercitato su `llama-server`) e `fresh-install` sull'app installata, dove `app_data_dir` è quello reale e non quello di dev.

---

## 2026-08-24 — Server orfani: due buchi nel cleanup dei processi figli

**Obiettivo:** indagare un `whisper-server` sopravvissuto alla chiusura dell'app, e chiudere i percorsi che possono lasciare un figlio vivo ma non più tracciato.

**Indagine:** il kill-on-exit funziona. Con sonde temporanee su `RunEvent` e su `kill_tracked`, la chiusura dalla finestra produce `CloseRequested → Destroyed → ExitRequested` e poi `slot=Some(pid=...) start_kill=Ok(())`, senza processi residui. L'orfano osservato aveva un'altra causa: nel log il figlio ha stampato `whisper server listening` **dopo** la morte del padre, avendo completato tranquillamente il caricamento del modello. Un processo che riceve SIGKILL non finisce di caricare: nessun segnale l'aveva raggiunto, perché l'app era stata terminata da un segnale esterno (lanciata come task di background) e `ExitRequested` non era mai stato emesso.

**Fatto (`commands/server.rs`):**
- `spawn_tracked`: `kill_on_drop(true)`, così un `Child` droppato senza kill esplicito (unwind da panic, errore a metà stop) non lascia il processo vivo
- `stop_tracked_server`: se `start_kill` fallisce il child viene rimesso nello slot prima di propagare l'errore. Prima usciva dallo slot con `take()` e veniva droppato con l'errore: processo vivo, porta occupata, e `kill_tracked` alla chiusura trovava lo slot vuoto

**Decisioni:**
- Recupero dello slot **solo** su `start_kill` fallito, non su `wait` fallito: nel secondo caso il SIGKILL è già partito e il processo muore comunque, ritracciarlo lascerebbe nello slot un child morto
- Niente handler di segnali per SIGTERM/SIGINT: coprirebbe la chiusura da terminale ma non SIGKILL, e per un'app desktop il percorso reale è la chiusura dalla finestra, che già funziona. Documentato in `architecture.md` come limite noto invece di aggiungere una dipendenza
- Il ramo `if port_is_open { return Ok(()) }` di `start_local_server` resta invariato: adottare un processo non proprio per poi killarlo violerebbe l'invariante "mai kill-by-porta" (2026-07-30)

**Verificato:** `cargo check` pulito; a runtime Spegni → Avvia → chiusura finestra non lascia processi né porte occupate. Il secondo server è stato ucciso mentre era ancora in caricamento (nel log manca `listening`), cioè esattamente la condizione che aveva prodotto l'orfano.

---

## 2026-08-24 — Slimming pass: dead code, dedup, ottimizzazioni runtime

**Obiettivo:** ridurre il codice senza togliere feature: tagliare il codice morto verificato, deduplicare i pattern ripetuti, sistemare le inefficienze runtime a basso rischio.

**Fatto (backend):**
- Rimossi: comando `get_recording_status` + struct `RecordingStatus` (il cronometro è ora un timer locale del frontend, niente poll IPC a 500ms), campo `SttSettings.configured` (scritto e mai letto), `HfModelSummary.likes` e `HfModelDetail.context_length` (fetchati e mai mostrati), `RecorderInner.start_time` (letto solo dal comando rimosso)
- Dipendenza `sysinfo` rimossa: `get_system_memory_gb` legge `hw.memsize` via sysctl (macOS) / `/proc/meminfo` (Linux)
- `server.rs`: nuovi helper condivisi `default_threads`, `spawn_tracked`, `kill_tracked` (dedup con stt/llm/lib.rs); `wait_for_port` polla a 250ms invece che 1s
- `download.rs`: evento di progresso emesso solo al cambio di percentuale intera (prima: uno per chunk, decine di migliaia di eventi IPC su un modello da GB)
- `stt.rs`: upload trascrizione in streaming dal file (`Part::stream`), non più tutto il WAV in RAM
- `recording.rs`: in `stop_recording` il DSP (to_mono/resample/AEC/write_wav) gira in `spawn_blocking` fuori dal lock del RecorderState
- `mic.rs`: un solo builder generico (`SizedSample` + `FromSample`) per F32/I16/U16

**Fatto (frontend):**
- `Button.svelte`: varianti `solid`/`danger`/`primary` al posto delle stringhe di classi copiate 10+ volte
- Nuovi `ServerControls.svelte` (blocco stato+bottoni server, usato 2x da SettingsPanel) e `DownloadProgressBar.svelte` (Onboarding + SettingsPanel x2)
- `SettingsPanel.svelte`: gli 8 handler server collassati in `runStt`/`runLlm`; i 3 box percorso in uno snippet locale `pathBox`; ricerca HF lazy (prima fetch al primo focus del campo, non all'apertura del pannello); ~750 → ~530 righe
- `onSaved` di SettingsPanel ora `() => void` (il parametro non era usato)

**Decisioni:**
- La conversione U16→f32 del mic ora passa da dasp (`FromSample`), che gestisce l'offset unsigned correttamente: differenza dall'implementazione a mano precedente trascurabile (< 1 LSB)
- Non unificati: `refreshSttState`/`refreshLlmState` (default e stati diversi, l'unificazione costa più leggibilità di quanta ne renda), `WhisperServerState`/`LlamaServerState` (lo state Tauri è type-keyed), AEC O(lag×finestra) (~100ms in release, FFT sarebbe ottimizzazione prematura)

**Obiettivo:** un bottone di download esplicito per il modello LLM (oggi implicito nel bottone "Avvia" del server, senza feedback), con una progress bar reale; allargare il modal Impostazioni, oggi un singolo modal stretto di 420px.

**Fatto (backend, `src-tauri/src/commands/`):**
- Nuovo `download.rs`: estratto da `stt.rs` lo streaming HTTP generico (`.part` + rename atomico + cleanup su errore), condiviso ora fra `download_local_model` (whisper) e il nuovo `download_llm_model` (LLM)
- `llm.rs`: nuovo comando `download_llm_model` (scarica il GGUF scelto da `https://huggingface.co/<repo>/resolve/main/<file>`, evento dedicato `llm-download-progress`); `start_llm_server` passa a `--model <path>` invece di `--hf-repo`/`--hf-file`, fallisce subito se il file non esiste; `set_llm_model` accetta anche `size_bytes`; `clear_llm_cache` cancella sia `llm-models/` sia la vecchia `llm-cache/`; `get_hf_model_files` filtra via i GGUF divisi in shard (`-NNNNN-of-NNNNN.gguf`)
- `mod.rs`: `SttSettings` guadagna `llm_size_bytes`/`llm_ready` (quest'ultimo ricalcolato da disco in `get_stt_settings`, mai persistito come verità, mirror di `local_ready`); nuovi helper `llm_model_path`/`llm_models_dir`

**Fatto (frontend, `src/`):**
- `types.ts`: `SttSettings.llmSizeBytes`/`llmReady`; nuovo tipo `LlmDownloadProgress` su un evento separato (vedi Decisioni)
- `SettingsPanel.svelte`: bottone "Scarica modello" nella sezione Modello LLM (mirror di quello whisper, 4 stati, progress bar propria); Avvia/Riavvia del server LLM disabilitati se il modello non è pronto; modal `w-[min(1400px,92vw)]`, corpo a griglia 2 colonne (sinistra: Permessi/Whisper/STT/Cartella; destra: Modello LLM/Server LLM)

**Decisioni:**
- **Corregge la decisione "Niente download manager custom per il modello LLM" del 2026-08-02**: la barra nativa di `llama-server` (`Downloading <file> ─────╴ NN%`, in `common/download.cpp`) è dietro un check `!is_output_a_tty()` — sotto `Stdio::piped()` (necessario per catturarla da un processo figlio Tauri) non produce alcun output. Non è un problema di formato del log: è output zero. Verificato sia leggendo il sorgente al tag pinnato (`b10229`) sia con un avvio reale del binario bundlato sotto pipe
- **Scartata anche la modalità "child download" di llama-server** (`LLAMA_SERVER_CHILD_MODE=download`, JSON pulito su stdout senza gate tty): esiste e avrebbe funzionato, ma è un protocollo interno non documentato legato al tag pinnato, a rischio di rompersi silenziosamente a un bump di versione
- **Scartato anche pre-scaricare noi il file nel layout cache nativo di llama-server**: schema hash-based (`models--org--repo/blobs/<hash>`, stile huggingface_hub), file spesso serviti da HF via redirect a storage "Xet" — hash non derivabile in modo affidabile lato nostro
- **heedm scarica il GGUF da sé** (stesso pattern di `download_local_model`): percentuale byte-esatta da `content_length()`, `llmReady` derivabile dal disco con lo stesso invariante di whisper, zero parsing fragile. Costo accettato: si perde il resume/etag nativo di llama.cpp, stesso livello di servizio che il download whisper ha già senza lamentele
- **Evento `llm-download-progress` separato da `download-progress`**, non un nuovo `step` sullo stesso evento: quello ha già due listener indipendenti (`SettingsPanel` e `Onboarding`), e uno step `"done"` condiviso avrebbe fatto scattare per errore, nel listener whisper di `SettingsPanel`, il codice che marca `localReady = true`
- **GGUF divisi in shard esclusi da questa iterazione**: riguardano solo modelli enormi (40GB+) che nessuna macchina target caricherebbe comunque; supportarli richiederebbe raggruppare gli shard e sommarne le size per una percentuale aggregata, fuori scope
- **Vecchia cache `llm-cache/` non migrata automaticamente**: chi ha già un modello scaricato con la versione precedente lo perde silenziosamente ai fini di `llmReady` (va ririscaricato), ma il file resta sul disco finché l'utente non preme esplicitamente "Elimina modelli scaricati" — nessuna cancellazione automatica a sorpresa

**Verificato:** `npm run typecheck` (0 errori), `npm run lint:fix` (nessuna modifica), `cargo check --manifest-path src-tauri/Cargo.toml` (pulito). URL di download per un repo reale verificato con `curl -I` (redirect Xet, 302, seguito di default da `reqwest`).

**Non verificato:** download reale di un modello in app (richiede `npm run tauri dev`, non avviato di iniziativa in questa sessione su richiesta esplicita dell'utente); layout a 2 colonne verificato solo leggendo il markup, non con uno screenshot reale; `fresh-install` (obbligatorio per modifiche al download del modello, non eseguito).

**Prossimi passi:** i tre punti non verificati sopra, con l'utente presente o su autorizzazione esplicita (skill `run-heedm`, poi `fresh-install`).

---

## 2026-08-02 — Riassunto e chat locale sulla trascrizione (Vercel AI SDK + llama-server)

**Obiettivo:** per ogni registrazione trascritta, un riassunto/appunti generato automaticamente e una chat per domande di follow-up sulla trascrizione, usando Vercel AI SDK contro un LLM locale (nessun cloud), stessa filosofia di whisper.

**Fatto (backend, `src-tauri/src/commands/`):**
- Estratto `server.rs`: `port_is_open`/`wait_for_port`/`stop_tracked_server`, condivisi fra `stt.rs` (refactorato, zero cambio di comportamento) e il nuovo `llm.rs`, per non duplicare due invarianti non ovvi (guard mai tenuto attraverso un `.await`, mai kill-by-porta)
- Nuovo `llm.rs`: ciclo di vita `llama-server` (porta 8081, `LLM_PORT`; `STT_PORT` rinominato da `LOCAL_PORT`), `search_hf_models`/`get_hf_model_files` (proxy Rust verso l'API pubblica di Hugging Face, `reqwest`), `get_system_memory_gb` (`sysinfo`), sidecar `notes.json` (`read_recording_notes`/`write_recording_notes`/`delete_recording_notes`, scrittura atomica)
- `scripts/build-llama-server.sh`: calco di `build-whisper-server.sh`, clona `ggml-org/llama.cpp` (tag `b10229`), universale arm64+x86_64, `-DLLAMA_BUILD_UI=OFF` (nessuna dipendenza da npm/rete in build), `-DLLAMA_BUILD_LIBRESSL=ON` (vedi Decisioni: senza, `--hf-repo`/`--hf-file` falliscono a runtime)
- `tauri.conf.json` bundla `binaries/llama-server`; `capabilities/default.json` aggiunge `http:allow-fetch*` scoped a `http://127.0.0.1:8081/*`; `lib.rs` registra `tauri_plugin_http` e uccide anche il child LLM su `ExitRequested`

**Fatto (frontend, `src/`):**
- `src/lib/llm.ts`: unico file che conosce l'AI SDK. `createOpenAICompatible` con `fetch` di `@tauri-apps/plugin-http` (non la fetch nativa del webview), `buildTranscriptContext`, `streamSummary`/`streamChatReply` (`streamText`), `parseSummary`
- `TranscriptNotes.svelte` (orchestrazione) + `TranscriptChat.svelte` (presentazionale), montati in `RecordingDetail.svelte` sotto la card trascrizione
- `App.svelte`: `llmStatus` + `refreshLlmState` mirror di `sttStatus`/`refreshSttState`, con un poll periodico (2s) mentre `"loading"`/`"starting"`
- `SettingsPanel.svelte`: sezione "Modello LLM" (ricerca live HF con debounce, espansione per repo con dimensione reale + badge RAM, `gated` non selezionabile) e "Server LLM" (Avvia/Riavvia/Spegni/Svuota cache), `serverLoading`/`serverError` split in `sttLoading`/`sttError` + `llmLoading`/`llmError`

**Decisioni:**
- **Non `@ai-sdk/svelte`**: la sua classe `Chat` richiede un backend HTTP proprio che esegua `streamText()` — heedm non ha un server JS a runtime. Si usano le funzioni core (`streamText`) direttamente nel frontend contro `llama-server`
- **Niente download manager custom per il modello LLM**: `llama-server --hf-repo/--hf-file` scarica e cacha da solo (env `LLAMA_CACHE`, puntata a una directory gestita da heedm). Ricerca modelli **live** contro l'API di Hugging Face (verificata con chiamate dirette in fase di piano: `search`, `?blobs=true` per le dimensioni reali, `gated`), non fasce statiche né modello fisso
- **Fetch via `@tauri-apps/plugin-http`, non fetch diretta dal webview, fin dall'inizio**: l'ATS di macOS può bloccare HTTP semplice da un'app pacchettizzata anche verso `127.0.0.1` ([tauri-apps/tauri#4722](https://github.com/tauri-apps/tauri/issues/4722)), e heedm non aveva mai avuto un'eccezione ATS configurata. Scelto come default fin da subito invece che come fallback da scoprire a UI già scritta
- **`-DLLAMA_BUILD_LIBRESSL=ON` obbligatorio, scoperto solo testando davvero il download**: il primo binario compilato (senza questo flag) compilava senza errori ma falliva a runtime con `"HTTPS is not supported"` su qualunque `--hf-repo`. `llama.cpp` ha droppato `libcurl`, il downloader HF ora linka un backend TLS direttamente (OpenSSL/BoringSSL/LibreSSL); LibreSSL vendorizzata è l'unica delle tre che non richiede una libreria di sistema preinstallata, coerente con lo script esistente. Non era nella documentazione consultata in fase di piano: emerso solo eseguendo per davvero un test di download (repo di test minuscolo `ggml-org/tiny-llamas`, non un modello di produzione)
- **`/health`, non solo la porta, per il readiness check LLM**: `llama-server` apre la porta prima di aver caricato il modello (503→200), a differenza di whisper-server
- **`start_llm_server` non attende `/health`**: il primo avvio può scaricare per minuti, bloccare il comando Tauri per quel tempo sarebbe sbagliato; il frontend fa polling
- **`attemptStart` di default `false` per l'LLM** (contro `true` di STT): al mount si osserva soltanto, mai un caricamento automatico di un modello da 1-5GB
- **Generazione riassunto automatica solo se il server è già `"running"`**, altrimenti un CTA esplicito: mai un avvio/download a sorpresa, stessa filosofia di `ensure_stt_ready`
- **whisper-server e llama-server totalmente indipendenti** (porte, processi, stato separati): chi ha poca RAM viene guidato verso un modello più piccolo dal badge RAM, non forzato a spegnere whisper. Nessun blocco di chat/riassunto durante `isRecording`/`isTranscribing`: processi e porte diversi, nessun conflitto reale
- **Nessuna astrazione di stato condivisa fra i due cicli di vita server**: `refreshLlmState` duplica la forma di `refreshSttState` invece di un helper comune, coerente con la scelta già presa per STT di non introdurre state management per pochi valori
- **Riassunto a quattro sezioni marcate** (`RIASSUNTO:`/`PUNTI CHIAVE:`/`AZIONI:`/`DOMANDE APERTE:`), testo semplice non JSON/`generateObject`: lo structured output non fa streaming, uno spinner di 20-60s è peggio del testo che appare
- **Un solo `notes.json`** (non `summary.json`+`chat.json`): un solo scrittore, whole-document replace, niente da riconciliare fra due file
- **Truncation invece di map-reduce** per trascrizioni lunghe (`--context-shift` disattivato lato server ⇒ prompt troppo lungo è un errore secco): il map-reduce è il fix corretto ma fuori scope, annotato come prossimo passo
- **Licenza Llama 3.2** (se scelto dall'utente in ricerca): richiede menzione "Built with Llama" in caso di distribuzione dell'app — non bloccante per uso personale, nota per un eventuale About screen futuro

**Verificato:** `cargo check` e `npm run typecheck` puliti. Binario `llama-server` compilato (universale arm64+x86_64), flag CLI (`--hf-repo`, `--hf-file`, `--ctx-size`, `--n-gpu-layers`, `--parallel`, `--alias`, `--jinja` on-by-default, `--no-webui`) confermati con `--help` sul binario reale. **End-to-end reale** sul binario che verrà bundlato: `--hf-repo`/`--hf-file` scarica e cacha da Hugging Face (repo di test minuscolo, non un modello di produzione da GB), `/health` passa da assente a `200`, `/v1/chat/completions` risponde in streaming SSE fino a `[DONE]` — prima con LibreSSL mancante (fallimento riprodotto), poi con la build corretta (successo). `npm run tauri dev` avviato e verificato con screenshot reale: l'app mostra correttamente `Onboarding` su questa macchina di sviluppo (nessun modello whisper scaricato qui), nessuna regressione visibile all'avvio con i nuovi plugin/stato registrati. Bonus non cercato: un `settings.json` preesistente su questa macchina (scritto prima dei campi `llm_hf_repo`/`llm_hf_file`) è stato letto correttamente grazie a `#[serde(default)]`, conferma pratica del rischio di parsing che aveva motivato quella scelta.

**Non verificato:** smoke test STT record→transcribe con un vero modello whisper (nessun modello scaricato su questa macchina di sviluppo, il download da 1.5GB non è stato avviato per questa sola verifica); il flusso end-to-end completo dentro l'app (dettaglio → scelta modello dalla ricerca HF in Settings → riassunto → chat → persistenza dopo riavvio); `fresh-install` sulla build reale installata per il rischio ATS/capability (§3.1) — tutti e tre restano prossimi passi.

**Prossimi passi:** i tre punti non verificati sopra. Map-reduce per trascrizioni molto lunghe, se il cap attuale (~30000 caratteri) risultasse troppo stretto in pratica.

---

## 2026-07-31 — Ciclo di vita STT: onboarding obbligatorio, gate registrazione, controlli server, rilancio trascrizione

**Obiettivo:** quattro buchi collegati nel ciclo di vita di whisper-server e del modello locale: si poteva premere REC/importare senza server o modello pronti; il primo avvio apriva solo un modal impostazioni invece di un onboarding vero; non c'era modo di fermare/riavviare/eliminare il server o il modello dall'app; un errore di trascrizione non lasciava traccia e non si poteva rilanciare senza una nuova registrazione.

**Fatto (backend, `src-tauri/src/commands/`):**
- `get_stt_settings` ricalcola `local_ready` a ogni chiamata verificando il file sul disco (`local_model_path(...).exists()`), invece di fidarsi del flag persistito in `settings.json`
- `download_local_model` scrive su `<model>.bin.part` e fa `rename` atomico al path finale solo a download completato; su errore il `.part` viene ripulito
- Nuovi comandi in `stt.rs`: `stop_stt_server`, `restart_stt_server`, `delete_local_model`, sopra una `stop_local_server` condivisa che non tiene il `MutexGuard` di `WhisperServerState` oltre un `.await`, e che polla il rilascio della porta 8080 (timeout 5s) prima di ritornare
- Nuova `ensure_stt_ready(&AppHandle)` in `recording.rs`, condivisa da `start_recording` (che guadagna il parametro `app: AppHandle`) e `import_audio_file`: blocca con `Err` se modello o server non sono pronti, senza mai tentare un auto-riavvio
- `transcribe_recording` avvolge la logica precedente (rinominata `run_transcription`): su `Err` scrive `transcript_error.json` (`TranscriptError { message }`) accanto al WAV, su `Ok` lo rimuove se presente (copre il rilancio riuscito). `RecordingEntry` guadagna `error: Option<String>`, popolato da `list_recordings` solo quando `transcript.json` è assente

**Fatto (frontend, `src/`):**
- Nuovo `Onboarding.svelte`: view full-screen (non un modal) con logo, CTA download, progress bar (stesso evento `download-progress`/comando `download_local_model` di `SettingsPanel`), messaggio "caffè" durante il download, messaggio finale + "Continua". `App.svelte` la mostra al posto della UI normale quando `!modelReady`, sostituendo la vecchia `if (!s.configured) showSettings = true`: primo avvio e "modello eliminato in seguito" sono lo stesso caso, un solo ramo, nessun bottone "salta"
- `App.svelte`: `refreshSttState(opts?: { attemptStart?: boolean })` come punto unico di riallineamento fra stato reale e UI (usata da mount, `Onboarding`, `SettingsPanel`, rilancio trascrizione); `canRecord`/`recordGateReason` derivati per il gate REC/import (REC disabilitato solo per l'avvio, lo STOP resta sempre possibile); `retryTranscription(folderPath)` riusa lo stesso lock `isTranscribing` di REC/import (nessun flag secondario) e ricarica `list_recordings` come unica fonte di verità dopo un rilancio, riallineando `selectedEntry`
- `SettingsPanel.svelte`: nuova sezione server STT con Avvia/Riavvia/Spegni/Elimina modello. Riceve `sttStatus` e `onServerRefresh` (= `refreshSttState` di `App.svelte`) come prop: ogni azione richiama `onServerRefresh` (con `attemptStart: false` per "Spegni") invece di tenere una copia locale dello stato, altrimenti chiudere il modal con la X invece che con "Salva" avrebbe lasciato `App.svelte` con lo stato stale
- `RecordsList.svelte`/`RecordingDetail.svelte`: badge a 3 stati (`TranscriptStatus` in `types.ts`) e bottone "Rilancia"/"Rilancia trascrizione", disponibile anche su record già trascritti (non solo falliti). Riga di `RecordsList` ristrutturata da bottone unico a `<div>` con `<button>` (select) + `Button` (rilancia) separati, per evitare il nesting di bottoni

**Decisioni:**
- **`local_ready` persistito è informativo, non autoritativo**; niente flag "onboarding fatto" separato: la presenza del `.bin` sul disco è l'unica verità, anche per "utente cancella il modello a mano" (che riporta a `Onboarding`)
- **Nessun comando `get_model_status`/nuovo endpoint per lo stato server**: `get_stt_settings`/`check_stt_server` esistenti bastano, riletti al momento giusto
- **Caso orfano (porta 8080 occupata da un processo non tracciato) non viene killato per porta**: `stop_local_server` ritorna un errore esplicito piuttosto che rischiare di terminare un processo di terze parti
- **Un solo lock globale (`isTranscribing`) per registrazione, import e rilancio**: si escludono a vicenda, nessun flag secondario per "rilancio in corso"
- **Stato condiviso frontend centralizzato in `App.svelte` con props/callback**, nessun nuovo modulo di state management: prima astrazione di questo tipo nel progetto, scartata perché non necessaria per 4-5 valori di stato

**Verificato:** `cargo check` e `npm run typecheck` puliti su tutto il lotto, `npm run lint:fix` senza modifiche fuori scope. **Non verificato end-to-end**: in questo ambiente `whisper-server` crasha al caricamento del modello (`unknown tensor '' in model file`, `GGML_ASSERT` in `ggml-metal-device.m`), problema di ambiente preesistente e indipendente da queste modifiche (probabile mismatch fra la versione di whisper.cpp con cui è compilato il binario e quella con cui è stato generato il modello scaricato). Bloccava anche il test visivo dell'onboarding/gate/rilancio con screenshot reale (permesso Screen Recording non concesso al terminale).

**Prossimi passi:** risolvere il mismatch whisper-server/modello (probabile ricompilazione con `scripts/build-whisper-server.sh` o ri-scaricamento del modello), poi eseguire la skill `fresh-install` per la verifica end-to-end completa (onboarding da stato vergine, smoke test record→transcribe, rilancio riuscito).

---

## 2026-07-31 — Skill `fresh-install`: test da stato vergine

**Obiettivo:** poter provare l'app come la vede chi la installa per la prima volta, in modo ripetibile.

**Fatto:**
- `scripts/fresh-install.sh`: preflight, chiusura di app e orfani, wipe dello stato, reset permessi TCC, build release del solo `.app`, install in `/Applications`, lancio e checklist
- `.claude/skills/fresh-install/SKILL.md`: quando usarla, cosa distrugge, e i punti del first-run che si sbagliano più facilmente
- CLAUDE.md: la skill entra fra i comandi e fra le regole di verifica

**Decisioni:**
- **Wipe totale, modello incluso.** Il download da 1.5 GB con la sua progress bar è codice che può rompersi e fa parte del primo avvio: parcheggiare il modello avrebbe reso il test più veloce ma avrebbe saltato proprio quel pezzo
- **Registrazioni cancellate, non archiviate.** Scelta esplicita dell'utente dopo che avevo proposto l'archiviazione. Il `rm -rf` è protetto da una guardia: se il path non corrisponde a `~/Documents/*/Records` lo script si ferma
- **Script versionato + SKILL.md, non solo markdown.** La sequenza di wipe contiene l'unico `rm -rf` del repo: averla in un file leggibile in diff e lanciabile a mano vale i due file in più
- **Solo il `.app`, niente `.dmg`** (`--bundles app`): il dmg allunga il build e per un test locale non serve

**Ceiling:** i permessi concessi in `tauri dev` appartengono a un'identità di codice diversa dall'app in `/Applications`, quindi il flusso onboarding in dev non è osservabile. Da qui la skill.

**Da verificare al primo giro reale:** il bundle non ha `signingIdentity`, quindi è firmato ad-hoc. Se la firma cambia a ogni rebuild, macOS può accumulare voci duplicate per Heedm in Privacy & Security. Lo script stampa `codesign -dv` dopo l'install per poterlo osservare.

---

## 2026-07-31 — Diarizzazione delegata a whisper, doc collassati

**Obiettivo:** terzo lotto. Smettere di reimplementare in Rust una diarizzazione che whisper.cpp fa già, e ridurre i doc da 5 file a 2.

**Fatto:**
- `stop_recording` scrive un WAV **stereo** quando c'è audio di sistema (mic a sinistra, sistema a destra) invece del mix mono. Senza audio di sistema resta mono
- `transcribe_recording` passa `diarize=true` nel form: whisper riempie `segments[].speaker` con `"0"` (sinistra) e `"1"` (destra)
- Cancellati `recorder/diarization.rs`, il sidecar `.diarization.json`, `load_diarization_sidecar`, `dominant_speaker` e `audio::mix` (sostituita da `interleave_stereo`)
- Cancellata `prepare_for_whisper`: i file prodotti sono già nel formato che `read_wav` accetta, quindi i byte vanno diretti nel multipart. Sparisce anche `audio::encode_wav`, che esisteva solo per lei
- Frontend: `SPEAKER_INFO` passa da `mic`/`sys`/`both` a `"0"`/`"1"`
- `whisper-server` ricompilato a **v1.9.1** (era v1.7.4, dicembre 2024); avvio con `--flash-attn` e `--threads` dedotti dalla macchina
- `docs/` collassata: `architecture.md` (backend, pipeline, whisper) e `reference.md` (comandi, tipi, componenti) al posto di 5 file. Da ~710 a 302 righe

**Decisioni:**
- **Delegare invece di mantenere.** `estimate_diarization_speaker` in `server.cpp` fa esattamente ciò che faceva `diarization.rs`: confronta l'energia dei due canali per segmento. Tenere due implementazioni della stessa euristica non ha senso
- **Verificato prima di cancellare.** Costruito un WAV stereo con due voci distinte (`say` su canali separati) e inviato al server: il canale sinistro esce come `speaker: "0"`, il destro come `"1"`, e senza il campo `diarize` il campo `speaker` non compare. Solo dopo ho rimosso il codice Rust
- **`"?"` non diventa una terza etichetta.** Quando le energie sono a meno del 10% l'una dall'altra whisper ritorna `"?"`: `normalize_speaker` lo riduce a `None`, cioè lo stesso stato dei file mono. Le etichette restano due, YOU e THEM
- **Gli import restano mono.** I canali L/R di un file esterno non sono "io" e "gli altri": lasciarli separati produrrebbe etichette casuali
- **L'AEC diventa load-bearing.** Prima influenzava solo la qualità audio, ora l'echo non cancellato falserebbe direttamente l'attribuzione degli speaker

**Costo:** il WAV stereo pesa il doppio (64 KB/s invece di 32).

**Verificato:** `cargo check` e `npm run typecheck` verdi; contratto HTTP della diarizzazione verificato end-to-end contro il server reale. Non verificato: una registrazione vera dal microfono, che richiede la GUI.

**Prossimi passi:** VAD con il suo modello e i due interruttori in SettingsPanel, fix orfani whisper-server, upgrade delle 4 major Rust.

---

## 2026-07-31 — Split di commands.rs e primitive audio condivise

**Obiettivo:** secondo lotto della riduzione. Sciogliere il monolite `commands.rs` (824 righe, un terzo del codice) e togliere le duplicazioni nel Rust.

**Fatto:**
- `commands.rs` → `commands/mod.rs` (settings, percorsi, permessi), `commands/stt.rs` (modello, server, trascrizione), `commands/recording.rs` (cattura, import, listing). Nessun file sopra le ~300 righe
- Nuovo `recorder/audio.rs`: `TARGET_SAMPLE_RATE`, `rms`, `to_mono`, `resample_linear`, `mix`, `write_wav`, `encode_wav`. Assorbe `mixer.rs` e `writer.rs`, che spariscono
- Deduplicate tre copie: `rms` era in `aec.rs` e `diarization.rs`, la conversione `f32 -> i16` in `writer.rs` e `prepare_for_whisper`, il downmix a mono in `import_audio_file` e `prepare_for_whisper`
- Nuovo helper `new_recording_path`: il blocco "timestamp → crea cartella → path del WAV" era copiato in `stop_recording` e `import_audio_file`
- `SttSettings` usa `#[derive(Default)]` al posto dell'`impl Default` scritto a mano
- Rimosso `system_audio/windows.rs` (stub che ritornava solo `Err`); `lib.rs` registra i comandi con il percorso completo del modulo
- Tolti tre `unwrap()` dal path principale: `model_path.parent()`, `model.to_str()` e il lock di `WhisperServerState`

**Decisioni:**
- **`stop_recording` scrive sempre mono.** Prima passava a `write_wav` il numero di canali del microfono, ma applicava `resample_linear` al buffer **interleaved**: con un mic a 2 canali l'interpolazione lineare avrebbe mescolato campioni sinistri con destri, corrompendo l'audio. Non si vedeva perché il mic in uso è mono. Ora il downmix a mono è esplicito e precede il resample
- **Percorsi completi in `generate_handler!`** invece di `pub use` nel `mod.rs`: la macro di Tauri genera anche `__cmd__<nome>` e non segue i re-export

**Verificato:** `cargo check` verde. Non verificato: registrazione reale e smoke test STT, che richiedono la GUI.

**Prossimi passi:** delega della diarizzazione a whisper (WAV stereo + `diarize=true`), fix orfani whisper-server, flag v1.9.1.

---

## 2026-07-31 — Rimozione di shadcn-svelte

**Obiettivo:** primo lotto del lavoro di riduzione della repo. Togliere lo stack shadcn-svelte, mai entrato in uso.

**Fatto:**
- Cancellati `src/lib/components/ui/button/` (`button.svelte` + `index.ts`) e `components.json`
- `src/lib/utils.ts` ridotto al solo `cn()`: i tipi `WithoutChild`, `WithoutChildren`, `WithoutChildrenOrChild`, `WithElementRef` avevano come unico consumatore il button generato
- `src/App.css`: via i due import (`tw-animate-css`, `shadcn-svelte/tailwind.css`), la `@custom-variant dark`, i ~40 token semantici nel `@theme`, i ~30 in `:root` e il `@keyframes shimmer` mai referenziato. Le regole `@layer base` che usavano `bg-background`/`text-foreground` ora usano direttamente `bg-brand-dark`/`text-brand-cream`
- Rimosse le devDependencies `shadcn-svelte`, `tailwind-variants`, `tw-animate-css`

**Decisioni:**
- **Ribaltata la decisione del 2026-07-25** ("shadcn resta, è la base per i componenti futuri"): a un mese e mezzo dall'integrazione l'unico componente installato non è mai stato importato, mentre il bottone realmente in uso (`Button.svelte`, scritto a mano) è nato dopo. Tenere una libreria per componenti futuri che non arrivano costa 219 righe e 3 dipendenze
- **La scala dei raggi resta**, ma come valori letterali (`--radius-lg/xl/2xl`) invece della catena `calc(var(--radius) * n)`: senza, ogni `rounded-lg` sarebbe passato da 0.625rem al default Tailwind di 0.5rem, cioè un cambio visivo non richiesto. Tenuti solo i tre gradini usati nel markup
- **`* { @apply border-border outline-ring/50 }` eliminata senza sostituto**: nessuna utility `outline-*` o `ring-*` compare nel markup, e tutti i bordi dichiarano già il proprio colore

**Verificato:** `npm run typecheck` (3765 file, 0 errori) e `npm run build` verdi.

**Prossimi passi:** dedup Rust (`rms` duplicata, conversione f32→i16, downmix mono), split di `commands.rs` in tre moduli, delega della diarizzazione a whisper via WAV stereo.

---

## 2026-07-25 — Riallineamento config Claude e pulizia superficie comandi

**Obiettivo:** togliere dal repo config Claude non versionata, comandi Tauri morti e dipendenze sovradimensionate, dopo il giro di migliorie alla config globale.

**Fatto:**
- `.claude/` non è più gitignorata: `settings.json`, hook e skill `run-heedm` entrano in git. Fuori restano solo `settings.local.json` (permessi con path assoluti di questa macchina) e i `.lock` di runtime
- Rimossi i due worktree in `.claude/worktrees/` (`chore/trim-claude-md`, `worktree-cleanup-ponytail-refs`): branch intatti in locale e su origin
- Eliminato `.claude/commands/commit.md`: duplicava la skill globale `/commit`, che già legge la sezione `## Commit` del CLAUDE.md locale
- `settings.json`: rimosso il campo `if` dall'hook (non è nello schema, veniva ignorato), allowlist estesa ai comandi di sessione (typecheck, cargo build/test/fmt/clippy, health whisper, pkill)
- Rimossi 3 command morti (`pick_directory`, `get_local_model_status`, `request_screen_recording_permission`) e `start_local_server` declassato a funzione interna: il frontend usava solo il suo alias `start_stt_server`
- `permissions.rs`: via anche `request_screen_recording` + la FFI `CGRequestScreenCaptureAccess`, senza più chiamanti
- `tokio` da `features = ["full"]` alle 8 feature realmente usate; `package.json` rinominato da `tauri-app` a `heedm` + script `typecheck`
- CLAUDE.md: mappa moduli riallineata (mancavano `permissions.rs`, `aec.rs`, `diarization.rs`, tutta la UI post-refactor), doc rule estesa a `permissions.rs`

**Decisioni:**
- **shadcn resta**, anche se `components/ui/button` non è importato da nessuno: è la base per i componenti futuri, non codice morto da rimuovere
- **`start_local_server` interno invece che cancellato**: la logica di spawn serve, era solo esposta due volte come command
- **`settings.local.json` resta fuori da git**: è per-macchina, i permessi condivisibili vivono in `settings.json`

**Ceiling:** `cargo test` non linka su questo setup (solo-CLT, `__swift_FORCE_LOAD_$_swiftCompatibility56` via `apple_metal`). Preesistente, verificato con stash: la verifica reale resta `cargo check`.

**Prossimi passi:** split di `commands.rs` (850 righe, 5 responsabilità), test sulle funzioni pure (`aec`, `diarization`, `mixer`, `groupSegments`), decisione su `system_audio/windows.rs` stub.

---

## 2026-07-17 — Import file audio esterno da trascrivere

**Obiettivo:** oltre a registrare, permettere di caricare un file audio già esistente (dal Finder) e trascriverlo con lo stesso flusso di una registrazione.

**Fatto:**
- Nuovo command `import_audio_file` (commands.rs): picker file → `decode_audio` (symphonia) → downmix mono + `resample_linear` a 16kHz → scrive `recording.wav` in nuova cartella Records → ritorna path. Output identico a `stop_recording`, così l'entry riusa Records + `transcribe_recording` senza rami nuovi
- `App.svelte`: link "o carica un file" sotto il bottone REC (solo a riposo) → import → transcribe → `view = "list"`
- `Cargo.toml`: aggiunto `symphonia` (features mp3/aac/isomp4/flac/vorbis/ogg/pcm/wav)

**Decisioni:**
- **symphonia, non ffmpeg:** decoder Rust puro, coerente con la scelta già fatta per il resample (`resample_linear`, niente ffmpeg/rubato). Zero binari esterni da bundle
- **Decode a monte, non a valle:** `import` scrive già un WAV 16kHz canonico invece di passare il file grezzo a `transcribe_recording` (che via `prepare_for_whisper`/hound legge solo WAV). Così `list_recordings` e la trascrizione restano invariati
- Nessun sidecar diarizzazione: sorgente singola → `speaker = None` (già gestito)

**Ceiling:** mp4/m4a = solo traccia audio AAC. Codec esotici / file corrotti → errore propagato alla UI.

**Prossimi passi:** —

---

## 2026-06-28 — AEC: rimozione echo acustico dal microfono

**Obiettivo:** quando l'utente usa le casse del Mac, l'audio di sistema (colleghi) veniva ripreso dal microfono causando diarizzazione errata (tutto "both") e overlapping fasullo in trascrizione.

**Root cause:** `estimate_timeline` confronta energia RMS mic vs sys, ma mic già contiene l'echo di sys → i livelli risultano sempre comparabili.

**Fatto:**
- Nuovo `src-tauri/src/recorder/aec.rs`: AEC offline puro Rust — cross-correlazione normalizzata per stimare il delay acustico (finestra 2s, max 200ms), least-squares per stimare α, sottrazione con clamp
- `stop_recording` (commands.rs): applica `aec::cancel_echo(&mic, &sys, sr)` dopo il resample del mic e prima di `estimate_timeline`/`mix` — solo quando sys attivo
- Nessun nuovo crate

**Decisioni:**
- Offline (post-processing) invece di real-time: più semplice, funziona per il caso d'uso (registrazione poi trascrizione)
- Skip AEC se correlazione < 0.10: evita di peggiorare registrazioni senza echo
- Limite noto documentato nel commento in testa ad `aec.rs` — se in futuro serve gestire reverb multi-path, upgrade a `webrtc-audio-processing`

---

## 2026-06-28 — Folder-per-recording + Records view

**Obiettivo:** struttura a cartella per registrazione, persistenza trascrizione su disco, UI separata per sfogliare i record storici.

**Fatto:**
- `stop_recording` ora crea `records_dir/YYYY-MM-DD HH.MM.SS/` e salva `recording.wav` dentro (prima: file piatto con nome lungo)
- `transcribe_recording` scrive `transcript.json` nella cartella del WAV a fine trascrizione (best-effort, `.ok()`)
- Il sidecar diarizzazione segue automaticamente: `recording.diarization.json` nella stessa cartella
- Nuovo comando `list_recordings`: scansiona le sottocartelle, carica `transcript.json` se presente, ordine cronologico inverso
- Nuovo `RecordsList.svelte`: vista elenco con badge "trascritto"/"in attesa"
- Nuovo `RecordingDetail.svelte`: vista dettaglio con `TranscriptView` + "Apri cartella" via plugin opener
- `App.svelte`: state `view` a 3 vie (`record|list|detail`), bottone navigazione lista (fixed top-right), rimosso array in-memory `recordings` e componente `RecordingItem`

**Decisioni:**
- WAV si chiama `recording.wav` (non il timestamp completo): la cartella ha già il timestamp nel nome
- `transcript.json` best-effort: un errore di scrittura non fa fallire la trascrizione (già in memoria per il frontend)
- Array `Recording[]` in-memory rimosso: la fonte di verità è ora il filesystem; `list_recordings` carica al mount della lista
- `RecordingItem.svelte` eliminato: sostituito da `RecordsList` + `RecordingDetail`

---

## 2026-06-15 — Integrazione shadcn-svelte

**Obiettivo:** introdurre shadcn-svelte come libreria di componenti UI primitivi, mantenendo la palette brand esistente.

**Fatto:**
- `npx shadcn-svelte@latest init` (style `vega`, base color `neutral`, icon library `lucide`) → genera `components.json`, `src/lib/utils.ts` (`cn()`), aggiunge dipendenze (`clsx`, `tailwind-merge`, `tailwind-variants`, `tw-animate-css`, `@fontsource-variable/inter`)
- `vite.config.ts` / `tsconfig.json`: alias `$lib` → `src/lib` (progetto non-SvelteKit, alias non automatico)
- `src/App.css`: token semantici shadcn (`--background`, `--primary`, ecc.) rimappati in `:root` sulla palette `--color-brand-*` esistente; rimosso blocco `.dark{}` generato dal CLI
- Primo componente installato: `Button` (`src/lib/components/ui/button/`)
- Doc: `docs/frontend/ui.md` — sezione "shadcn-svelte" con tabella mapping token

**Decisioni:**
- Niente `.dark{}`/toggle: Heedm è sempre in dark theme fisso, i token shadcn diventano l'unica fonte di verità puntando alla palette brand — le utility `dark:*` dei componenti generati restano inerti
- Style `vega` ("the classic shadcn/ui look") scelto come preset base — più neutro da skinare via token, rispetto a preset con font/icon-set più opinionati
- `@lucide/svelte` spostato da `dependencies` a `devDependencies` dal CLI (convenzione shadcn-svelte per progetti Vite) — non impatta il bundle finale, lasciato com'è

**Prossimi passi:** migrare progressivamente i componenti custom (`SettingsPanel`, `RecordingItem`, ecc.) verso primitive `ui/*` dove sensato (es. bottoni, dialog/modal).

---

## 2026-06-08 18-35 — Termina whisper-server alla chiusura dell'app

**Obiettivo:** `whisper-server` restava in background (processo orfano) dopo il quit di Heedm, perché veniva spawnato senza tenerne l'handle.

**Fatto:**
- `commands.rs`: nuovo `WhisperServerState(Mutex<Option<tokio::process::Child>>)`, gestito come state Tauri; `start_local_server` ora salva l'handle del `Child` appena spawnato
- `lib.rs`: la build dell'app passa da `.run()` diretto a `.build()` + `app.run(...)` con gestione di `RunEvent::ExitRequested` — estrae il `Child` dallo state e chiama `start_kill()` (kill non bloccante, va bene in handler sincrono)
- Doc: `docs/backend/stt.md` — sezione "Terminazione alla chiusura dell'app"

**Decisioni:**
- `start_kill()` invece di `kill().await`: l'handler di `RunEvent` è sincrono, `start_kill` invia il segnale senza bisogno di un runtime async
- Gestito solo `ExitRequested` (non anche `Exit`): copre quit standard (Cmd+Q / menu Quit); un eventuale orfano residuo da crash/force-quit andrebbe gestito separatamente (es. check porta 8080 all'avvio)

**Prossimi passi:** —

---

## 2026-06-08 — Cartella registrazioni default: da ~/Music a ~/Documents

**Obiettivo:** spostare la cartella default delle registrazioni da `~/Music/Heedm/Records/` a `~/Documents/Heedm/Records/`.

**Fatto:**
- `default_recordings_dir` in `commands.rs`: sostituito `app.path().audio_dir()` con `app.path().document_dir()`
- Doc aggiornati: `docs/backend/stt.md` (path default e commento `recordings_dir`), `docs/frontend/ui.md` (commento `recordingsDir`)

**Decisioni:**
- **Documents invece di Music**: i file prodotti (WAV + trascrizioni) sono documenti utente generati da un'app di produttività, non contenuti musicali — `~/Documents` è la posizione convenzionale macOS per questo tipo di output, oltre a restare visibile in Finder e indicizzata da Spotlight come `audio_dir`. Nessuna capability/entitlement aggiuntiva richiesta (`document_dir()` è API standard Tauri v2)
- Nessuna migrazione automatica delle registrazioni già presenti in `~/Music/Heedm`: resta a discrezione dell'utente spostarle

**Prossimi passi:** —

---

## 2026-06-08 — Implementata opzione 2: registrazioni a 16kHz fisso (era rate nativo del mic)

**Obiettivo:** completare la riduzione dimensione WAV avviata con l'entry sotto (16-bit PCM) — applicare anche l'opzione 2 dell'analisi originale, abbassando il sample rate di registrazione a 16kHz fisso (lo stesso richiesto da whisper). Ulteriore ~3× di riduzione (16-bit@48kHz ≈ 5.6MB/min → 16-bit@16kHz ≈ 1.83MB/min — combinato ~6× rispetto all'originale Float32@48kHz ~11.5MB/min).

**Fatto:**
- `commands.rs`: `WHISPER_SAMPLE_RATE` rinominata `TARGET_SAMPLE_RATE` — ora rate condiviso tra registrazione e trascrizione (stesso valore, stesso significato: registrare già al rate richiesto da whisper elimina il resampling per le nuove REC)
- Estratta `resample_linear(samples, from_rate, to_rate)`: la logica di interpolazione lineare già scritta dentro `prepare_for_whisper` (gestisce rate arbitrari/rapporti non interi, es. 44100→16000), ora condivisa fra trascrizione e registrazione, con early-return passthrough se `from_rate == to_rate`
- `start_recording`: l'audio di sistema viene catturato passando `TARGET_SAMPLE_RATE` invece di `mic_info.sample_rate` — su macOS `SCStream::with_sample_rate` lo converte **nativamente** a 16kHz (zero codice nostro). Il rate nativo del mic viene salvato a parte in `RecorderInner::mic_native_rate` (nuovo campo)
- `stop_recording`: il buffer mic (unico a non arrivare già a 16kHz — cpal non garantisce di poter forzare il rate di cattura) viene ricampionato con `resample_linear(mic_native_rate → TARGET_SAMPLE_RATE)` **prima** di `estimate_timeline`/`mixer::mix` — entrambi richiedono buffer allo stesso rate. `RecorderInner::sample_rate` ora rappresenta il rate di *output* (fisso), non più quello (variabile) del mic
- Doc aggiornati: `docs/backend/recorder.md` (campo `mic_native_rate`, sezione mixer/resample, sys audio macOS), `docs/backend/stt.md` (sezione `prepare_for_whisper` — passthrough per nuove REC, `resample_linear` condivisa), `docs/architecture.md` (diagramma con step di resample mic)

**Decisioni:**
- **Resample solo del buffer mic, non dell'audio di sistema**: ScreenCaptureKit converte nativamente al rate richiesto in cattura (`with_sample_rate`) — sys arriva già a 16kHz senza alcun lavoro nostro. cpal invece non garantisce di poter forzare un rate arbitrario sulla maggior parte dei microfoni — l'unica via è catturare al rate nativo e ricampionare dopo
- **Costante condivisa `TARGET_SAMPLE_RATE` invece di due costanti uguali**: evita il rischio di drift (una cambiata, l'altra no) e documenta esplicitamente che la scelta del rate di registrazione *è* la scelta del rate richiesto da whisper — non sono due esigenze in conflitto da bilanciare, sono la stessa cosa
- **Nessuna migrazione per registrazioni esistenti**: restano al loro rate nativo originale (es. 48kHz) — `prepare_for_whisper` le gestisce comunque tramite lo stesso `resample_linear` (percorso "legacy", non passthrough)
- **Tradeoff qualità accettato esplicitamente dall'utente**: rompe la promessa "salva a piena qualità" (vedi entry "Diarizzazione reale" sotto, e l'analisi originale che classificava questa opzione come "cambio architetturale, non solo ottimizzazione"). Perdita reale ma contenuta: 16kHz = qualità "voce HD telefonica" (Nyquist 8kHz), trasparente per il parlato, percepibile solo su contenuti non-vocali (musica, rumori acuti)
- **Latent mismatch preesistente non toccato**: `system_audio/linux.rs` ignora il parametro `sample_rate` passato (usa il rate nativo del device "monitor") — bug preesistente (già presente prima di questo cambio, quando il rate passato era quello nativo del mic), non aggravato qui, fuori scope: Linux è target futuro, non ancora supportato attivamente

---

## 2026-06-08 — Implementata opzione 1: WAV registrati ora 16-bit PCM (era Float32)

**Obiettivo:** chiudere l'analisi dell'entry sotto — applicare l'opzione raccomandata (16-bit PCM) per dimezzare la dimensione delle registrazioni (~10MB/min → ~5MB/min).

**Fatto:**
- `recorder/writer.rs::write_wav`: spec output `{ bits_per_sample: 16, sample_format: Int }` (era `{ 32, Float }`); ogni sample `f32` convertito con clamp `[-1.0, 1.0]` poi scala per `i16::MAX` — stessa conversione già usata in `prepare_for_whisper` (`commands.rs`)
- Doc aggiornati: `docs/backend/recorder.md` (spec `write_wav`), `docs/backend/stt.md` (rimosso riferimento a "float32"/"piena qualità" come motivazione — ora 16-bit è il formato salvato), `docs/architecture.md` (diagramma: "WAV Float32" → "WAV Int16 PCM"), commento in `commands.rs::prepare_for_whisper`

**Decisioni:**
- Nessuna migrazione per i WAV float32 esistenti su disco — restano leggibili (`prepare_for_whisper` già gestisce sia `Float` che `Int` in lettura), semplicemente più pesanti delle nuove registrazioni
- Bit-depth 16 è lo stesso identico formato che `prepare_for_whisper` produce comunque per l'invio a whisper-server — nessuna perdita percepibile aggiuntiva rispetto alla pipeline STT già in uso

---

## 2026-06-08 — Analisi: ridurre dimensione file WAV registrati

**Obiettivo:** un minuto di registrazione pesa ~10MB (mono Float32 a sample rate
nativo del mic, es. 48kHz: `48000 × 4 byte × 60s ≈ 11.5MB`, vedi
`recorder/writer.rs`). Valutare come ridurlo prima di archiviare grandi volumi
di registrazioni.

**Opzioni valutate:**
1. **16-bit PCM invece di Float32** — dimezza la size (~5MB/min). `hound`
   supporta `Int`/16-bit nativamente, modifica isolata a `write_wav`. Perdita
   di precisione bit-depth impercettibile su voce — `prepare_for_whisper`
   converte comunque tutto a 16-bit prima dell'invio (vedi `stt.md:152`).
   **Zero dipendenze nuove.**
2. **Sample rate più basso in registrazione (es. 16kHz)** — taglio fino a 1/3,
   coincide col rate target di whisper (eliminerebbe pure il resampling in
   `prepare_for_whisper`). Tradeoff: contraddice la scelta attuale e
   *documentata* di salvare "a piena qualità... per preservare l'archivio
   dell'utente" (`stt.md:152`) — è un cambio architetturale, non solo
   un'ottimizzazione, perché degrada permanentemente l'archivio.
3. **Compressione lossless (FLAC)** — riduce ~50-60% senza perdita, ma
   introduce una dipendenza Cargo esterna, contro il principio "niente
   toolchain esterni" già adottato per whisper-server (niente ffmpeg, niente
   crate di resampling).

**Raccomandazione:** opzione 1 (16-bit PCM) — guadagno consistente, nessuna
nuova dipendenza, nessuna perdita percepibile, coerente con la pipeline STT
che già normalizza a 16-bit.

**Decisione:** non ancora presa — entry di analisi, in attesa di conferma
prima di toccare `writer.rs`.

---

## 2026-06-07 — Sezione "Permessi" nelle impostazioni (microfono + registrazione schermo)

**Obiettivo:** l'app richiede due permessi macOS (microfono, registrazione schermo per ScreenCaptureKit) ma non offriva alcun modo, da dentro l'app, di sapere se erano concessi né di raggiungere il pannello di sistema giusto — screen recording mancante falliva *silenziosamente* (`eprintln!` lato Rust), mic mancante mostrava solo l'errore grezzo di `cpal`. L'unica doc di "dove abilitarli" viveva nella memory di sviluppo, non nell'app.

**Fatto:**
- Nuovo modulo `src-tauri/src/permissions.rs` (top-level, non sotto `recorder/`: è OS integration, non audio):
  - `screen_recording_granted()`/`request_screen_recording()` — FFI minimale verso `CGPreflightScreenCaptureAccess`/`CGRequestScreenCaptureAccess` (CoreGraphics, framework pubblico, **zero nuove dipendenze Cargo**); stub `true`/no-op su target non-macOS
  - `open_settings_pane(PermissionPane)` — apre il pannello Privacy & Security pertinente via `open x-apple.systempreferences:...?Privacy_Microphone|Privacy_ScreenCapture`; routing su enum chiuso, mai stringa utente nel comando di sistema
- Nuovi command: `check_screen_recording_permission`, `request_screen_recording_permission`, `open_permission_settings(pane: "microphone"|"screen-recording")` (valida l'input, `Err` su valori sconosciuti)
- `SettingsPanel.svelte`: nuova sezione "Permessi" in cima al modal (rinominato da "Trascrizione" a "Impostazioni" — ora copre più che STT) — riquadro Microfono (descrizione + CTA apri impostazioni) e riquadro Registrazione schermo (pallino di stato stile `SttIndicator` + CTA "Richiedi accesso"/"Apri Impostazioni di Sistema")

**Decisioni:** nessuno stato live per il microfono. L'unica API per l'authorization status è `AVCaptureDevice.authorizationStatus(for:)`, un metodo Objective-C — richiederebbe bridging `objc2`/AVFoundation (non presenti come dipendenza diretta, solo crate correlati trasportati transitivamente da `screencapturekit`) per mostrare un singolo pallino colorato. Un'indicazione di stato sbagliata o stantia sarebbe peggio di nessuna — meglio descrizione + CTA e basta.

---

## 2026-06-07 — Rimosso label `Silence` dalla diarizzazione

**Obiettivo:** eliminare il concetto di "silenzio" dalla diarizzazione "a 2 vie" — etichetta in pratica mai utile in UI (un blob grigio "Silenzio" tra due interventi non aggiunge informazione) e fonte di rumore nel timeline (soglia `SILENCE_RMS` arbitraria che spezzava intervalli altrimenti contigui).

**Fatto:**
- `recorder/diarization.rs`: rimossa variante `SpeakerLabel::Silence`, costante `SILENCE_RMS` e il ramo dedicato in `from_energies` — finestre a bassa energia ora ricadono nel confronto `Mic`/`Sys`/`Both` per rapporto di dominanza, niente più etichetta a parte
- `src/lib/types.ts`: rimossa voce `silence` da `SPEAKER_INFO` (resta `mic`/`sys`/`both`, fallback "Sconosciuto" per chiavi ignote)
- Doc aggiornati: `docs/architecture.md`, `docs/backend/stt.md`, `docs/backend/recorder.md`, `docs/frontend/ui.md` (rimossi riferimenti a `silence`/"Silenzio", contatori "4 valori" → "3 valori")

**Decisioni:** nessuna migrazione per sidecar `.diarization.json` esistenti con label `"silence"` — `serde` deserializza con `#[serde(rename_all = "lowercase")]`, un valore sconosciuto fallisce silenziosamente il parse del sidecar intero (`load_diarization_sidecar` ritorna `None` via `.ok()`), quindi al più si perde la diarizzazione di vecchie registrazioni, mai un crash.

---

## 2026-06-07 — Diarizzazione reale "a 2 vie" (mic vs. audio sistema), via timeline energetica app-side

**Obiettivo:** `TranscriptSegment.speaker`/`groupSegments`/`speakerColor` esistevano già in codice e doc (`architecture.md`, `ui.md` parlavano di "speaker diarization" come se funzionasse), ma **nulla** produceva mai un valore: `start_local_server` non passa `--diarize`/`--tinydiarize` a `whisper-server`, quindi `speaker` arrivava sempre `None` → UI mostrava un unico blob "Unknown". Wired una diarizzazione reale.

**Fatto:**
- Verificato (sorgente `examples/server/server.cpp` v1.7.4) che le due opzioni native di whisper.cpp non sono percorribili qui:
  - `--tinydiarize` richiede un modello tdrz fine-tuned esistente solo in **inglese** — incompatibile con `language: "it"` hardcoded in `transcribe_recording`
  - `--diarize` (split per canale stereo) inietta `(speaker N)` solo nell'output testuale legacy — il `verbose_json` che usiamo per segmenti/timestamp non guadagna mai un campo `speaker`, a prescindere dal flag. Per averlo via JSON servirebbe patchare `server.cpp`, mantenere la patch ad ogni bump di `WHISPER_TAG`, ricompilare il binario bundled, e riscrivere tutta la pipeline di registrazione per produrre WAV stereo (mic=L, sys=R) invece del mix mono — sproporzionato
- Nuovo modulo `recorder/diarization.rs`: `estimate_timeline(mic, sys, sample_rate)` confronta l'energia RMS di mic e audio di sistema a finestre da 200ms (soglie `SILENCE_RMS = 0.01`, `DOMINANCE_RATIO = 1.5`), etichetta ciascuna `Mic`/`Sys`/`Both`/`Silence`, accorpa finestre consecutive uguali in intervalli `{start, end, label}`
- `stop_recording`: chiama `estimate_timeline` sui buffer `mic_samples`/`sys_samples` **grezzi, prima** di `mixer::mix` (che li fonde in mono — a quel punto "chi" è perso), scrive il risultato come sidecar `<nome registrazione>.diarization.json` accanto al WAV. Solo se `sys_capture` era attivo (altrimenti il timeline sarebbe banalmente sempre "mic"); scrittura best-effort, log-and-continue — non deve mai far fallire una registrazione il cui WAV è già su disco
- `transcribe_recording`: dopo aver ricevuto `TranscriptResult`, cerca il sidecar e per ogni segmento sceglie l'etichetta dell'intervallo con maggiore sovrapposizione temporale `[start, end]`; assenza di sidecar (registrazioni vecchie o solo-mic) → `speaker: None`, nessuna regressione
- Frontend: sostituita l'euristica `speakerColor` (regex `(\d+)$` su un presunto formato `SPEAKER_NN` mai prodotto) con `SPEAKER_INFO`/`speakerInfo()` — mappa esplicita a 4 voci fisse (`mic`→"Tu", `sys`→"Sistema", `both`→"Sovrapposti", `silence`→"Silenzio") + fallback "Sconosciuto"; `groupSegments` ora raggruppa sulla chiave grezza (`"mic"|"sys"|...|"unknown"`), la label italiana viene risolta solo in rendering
- Doc aggiornati: `docs/backend/recorder.md` (nuova sezione "Diarizzazione"), `docs/backend/stt.md` (nuova sottosezione "Diarizzazione" + commento `speaker` aggiornato), `docs/backend/commands.md` (side effect sidecar su `stop_recording`/`transcribe_recording`), `docs/frontend/ui.md` (descrizione `TranscriptView`/`speakerInfo` aggiornata), `docs/architecture.md` (diagramma flusso con ramo diarizzazione)

**Decisioni:**
- **Stima energetica app-side + sidecar invece di patchare whisper.cpp**: Heedm cattura già mic e audio di sistema in buffer **separati** prima del mix (`RecorderInner.mic_samples`/`sys_samples`) — un dato che whisper.cpp non ha mai (lui vede solo il WAV finale, mono o stereo che sia). Sfruttarlo evita: fork/patch/rebuild del binario bundled, manutenzione della patch nel tempo, riscrittura del formato di registrazione, e il vincolo English-only di tinydiarize. Risultato: stima persino più accurata di quella nativa di whisper.cpp (che deve *ricavare* la separazione da un segnale stereo già "sporco", noi la *abbiamo* a monte)
- **Modello "a 2 vie" (tu = mic vs. sistema), non N-speaker generico**: è il massimo che la pipeline può onestamente distinguere — due sorgenti audio separate, ciascuna potenzialmente multi-persona (es. chiamata di gruppo lato sistema viene etichettata come un unico "Sistema"). Riflesso nei doc, che prima millantavano diarizzazione generica `SPEAKER_NN` mai esistita
- **Sidecar JSON best-effort, non parte del formato di registrazione**: se fallisce la scrittura non blocca/corrompe nulla — il WAV (l'archivio reale dell'utente) resta invariato e a piena qualità, il sidecar è puro arricchimento opzionale rigenerabile in teoria (anche se oggi non c'è un comando per rigenerarlo da una registrazione esistente — fuori scope, richiederebbe conservare i buffer grezzi)

**Prossimi passi:**
- Se in futuro servisse diarizzazione multi-persona vera (es. distinguere più partecipanti nello stesso canale), servirebbe un modello ML dedicato (pyannote/ecc.) — rottura del principio "no dipendenze cloud/pesanti" attuale, da valutare con cura

---

## 2026-06-07 — Fix pipeline trascrizione: endpoint sbagliato + formato WAV incompatibile ("error decoding response body")

**Obiettivo:** Risolvere l'errore "error decoding response body" alla trascrizione di una registrazione reale (visibile solo ora che il bundling ha sbloccato l'avvio del server — vedi entry sotto).

**Fatto:**
- Root cause #1 — **endpoint sbagliato**: `transcribe_recording` POSTava a `/v1/audio/transcriptions` (path OpenAI-compatible). Verificato sul sorgente `examples/server/server.cpp` di whisper.cpp (sia tag `v1.7.4` che `master`): il server espone **solo** `/inference`, `/load`, `/health` — mai un endpoint OpenAI-compatible. Il 404 risultante veniva interpretato come JSON → fallimento di parsing → "error decoding response body". Fix: URL → `/inference`, rimosso anche il campo form `model: "whisper-1"` (non parte dell'API whisper.cpp)
- Root cause #2 — **formato WAV incompatibile**: le registrazioni sono salvate mono, rate nativo del mic (cpal-determined, dinamico — es. 48kHz), float32 (`recorder/writer.rs`). `read_wav` di whisper.cpp (`common.cpp`) richiede **esclusivamente** mono 16kHz 16-bit PCM, altrimenti `{"error":"failed to read WAV file"}`
- Provata la direzione del fix end-to-end: convertita manualmente una registrazione a 16kHz/16-bit/mono con `afconvert` e inviata a `/inference` → risposta 200 con trascrizione corretta (`"ciao ragazzi ciao a tutti tutto ok"`), struttura JSON compatibile con `TranscriptResult`/`TranscriptSegment` (incl. `speaker: Option<String>`, deserializzato correttamente da chiave assente — comportamento nativo serde, nessun `#[serde(default)]` necessario)
- Aggiunta `prepare_for_whisper` (`commands.rs`): converte il WAV **in memoria, solo per la richiesta** — legge con `hound::WavReader`, normalizza a `f32`, downmix a mono (media canali), resample a 16kHz via interpolazione lineare (rate arbitrario in ingresso — 44100/48000/altro, non un rapporto intero fisso), converte a `i16`, ri-codifica con `hound::WavWriter` in un `Cursor<Vec<u8>>`. Eseguita in `tokio::task::spawn_blocking` (CPU-bound)
- Doc aggiornati: `docs/backend/stt.md` (sezione "Trascrizione" + nuova sottosezione "Conversione audio in memoria"), `docs/backend/commands.md`

**Decisioni:**
- **Conversione in-memory al momento dell'invio, file salvato invariato**: il file WAV su disco resta a piena qualità (rate nativo, float32) — l'archivio dell'utente non perde qualità per un vincolo del motore STT. Solo i bytes spediti a whisper-server vengono convertiti, scartati subito dopo
- **Interpolazione lineare pura-Rust invece di crate di resampling (`rubato`) o `ffmpeg`**: `--convert` di whisper.cpp richiederebbe `ffmpeg` sulla macchina dell'utente — stesso problema di dipendenza esterna già risolto bundlando il binario, da evitare. Un crate dedicato aggiungerebbe peso/complessità per un guadagno di qualità irrilevante su parlato/ASR (non audio musicale ad alta fedeltà). L'interpolazione lineare è ~15 righe, zero dipendenze, e gestisce correttamente rate arbitrari (incl. rapporti non interi come 44100→16000)

---

## 2026-06-07 — Bundling di whisper-server nell'app (fix "Could not find EOCD" al download)

**Obiettivo:** Risolvere l'errore `invalid Zip archive: Could not find EOCD` segnalato durante il download del modello.

**Fatto:**
- Root cause: `BIN_URL` (`commands.rs`) puntava a un asset GitHub Releases di whisper.cpp **mai esistito** — placeholder mai validato (c'era persino un commento "Update these URLs..."). Verificato via API GitHub: la release `v1.7.4` ha `assets: []`, e nessuna delle ultime 100 release contiene un asset `osx`/`macos`/`darwin`/`server` — whisper.cpp non ha mai distribuito un binario `server` precompilato per macOS. L'app scaricava la pagina 404 e la interpretava come zip → errore EOCD
- Rimossi `BIN_URL`, lo step di download/estrazione zip e la dipendenza `zip` (ormai inutilizzata) da `download_local_model`/`Cargo.toml`
- `whisper-server` ora **bundled come risorsa Tauri**: `tauri.conf.json` → `bundle.resources` (`binaries/whisper-server` → `whisper-server`), risolto a runtime con `app.path().resolve(_, BaseDirectory::Resource)` (nuova `bundled_bin_path` in `commands.rs`, sostituisce `local_bin_path`)
- Compilato `whisper-server` da whisper.cpp v1.7.4: statico (`BUILD_SHARED_LIBS=OFF`, zero dylib esterne — verificato `otool -L`), Metal abilitato (`GGML_METAL=ON`), **binario universale arm64+x86_64** via `lipo -create` (coerente con `minimumSystemVersion: 13.0`)
- Aggiunto `scripts/build-whisper-server.sh` che automatizza l'intero processo (clone, build per le due architetture, lipo merge, posizionamento in `src-tauri/binaries/`, gitignored — binario grande e generato)
- `download_local_model` ora scarica solo il modello (un unico step `"model"` invece di `"binary"`+`"model"`); `SettingsPanel` aggiornato di conseguenza (label progress, testo descrittivo)
- Doc aggiornati: `docs/backend/stt.md` (nuova sezione "Binario whisper-server — bundled come risorsa app"), `docs/backend/commands.md`, `docs/frontend/ui.md`

**Decisioni:**
- **Bundlare il binario precompilato** invece di scaricarlo (ovviamente impossibile, non esiste) o richiederlo come dipendenza esterna. Vincolo guida: l'app deve essere usabile da utenti finali consumer, non da sviluppatori. Due alternative scartate:
  - *Homebrew come dipendenza*: l'utente dovrebbe installare Homebrew + `brew install` whisper.cpp prima di usare l'app. Scartata perché gli utenti consumer non hanno Homebrew — attrito enorme per un'app desktop "double-click and use", e servirebbe logica fragile di detection/guida all'installazione (path, versioni, edge case)
  - *Build da sorgente al primo avvio*: l'app scarica i sorgenti whisper.cpp e li compila in locale al primo lancio. Scartata perché richiede Xcode Command Line Tools + cmake sulla macchina dell'utente (quasi mai presenti su un Mac consumer), compilazione lenta (minuti, CPU/batteria) al primo avvio, e fragile (drift versioni toolchain, necessità di rete per i sorgenti, errori di build da gestire in UI)
  - Bundling: zero passi extra per l'utente, pattern standard per tool "local-AI" desktop, nessuna dipendenza runtime da toolchain esterni — unica opzione compatibile col vincolo "usabile da utenti finali"
- Binario **statico** (no dylib condivise): più semplice da bundlare/firmare/notarizzare (un solo file eseguibile, nessun problema di rpath dentro `.app`), a fronte di un binario leggermente più grande
- **Universal binary** (arm64+x86_64) invece di due risorse separate per architettura: un solo percorso da risolvere a runtime, niente `#[cfg(target_arch)]` lato Rust — più semplice da mantenere, e coerente con quanto il vecchio `BIN_URL` già distingueva per architettura
- Aggiornamenti futuri: whisper.cpp → ri-lanciare lo script con un tag più recente e ricompilare l'app (singolo comando documentato); modello → resta disaccoppiato, si scarica a runtime da HuggingFace, basta cambiare la costante URL/filename senza toccare il binario

---

## 2026-06-07 — "Mostra nel Finder" sempre visibile per la cartella modello

**Obiettivo:** Il bottone "Mostra nel Finder" della cartella modello compariva solo a download completato (`localReady`), inconsistente con la cartella registrazioni (sempre visibile).

**Fatto:**
- `get_local_model_path` (`commands.rs`): crea sempre la cartella padre (`models/`) se assente, prima di restituire il path
- `revealModel` (`SettingsPanel.svelte`): rivela la cartella padre di `modelPath` invece del file `.bin` — il file potrebbe non esistere ancora se il modello non è stato scaricato
- Bottone visibile appena `modelPath` è noto, non solo quando `localReady`

**Decisioni:**
- Stesso pattern già usato per `recordings_dir` (creazione cartella eager) — `revealItemInDir` richiede che il target esista su disco, rivelare la cartella invece del file evita di ripetere il bug "Mostra nel Finder non funziona" già risolto in precedenza

---

## 2026-06-07 — Adozione Tailwind CSS + design system colori (ri-skin completo)

**Obiettivo:** Sostituire il CSS plain centralizzato (`App.css`, 617 righe, classi globali) con utility Tailwind inline e introdurre un design system colori basato su una palette brand fornita dall'utente.

**Fatto:**
- Aggiunte dipendenze `tailwindcss@4` + `@tailwindcss/vite@4` (config CSS-first, niente `tailwind.config.js`)
- `vite.config.ts`: registrato plugin `tailwindcss()`
- `App.css` ridotto da 617 righe a ~70: `@import "tailwindcss"`, token `@theme` (namespace `brand-*`), reset base in `@layer base`, 3 `@keyframes` custom (`pulse-rec`, `blink`, `shimmer`)
- Tutti e 5 i componenti Svelte (`App`, `SttIndicator`, `RecordingItem`, `TranscriptView`, `SettingsPanel`) ri-skinnati con classi utility inline — rimosse tutte le classi globali `.foo {}`
- `biome.json`: abilitato `css.parser.tailwindDirectives` (altrimenti Biome non riconosce `@theme`/`@import "tailwindcss"` e fallisce su parse/format)
- Doc aggiornato: `docs/frontend/ui.md` (sezione styling + tabella token + animazioni custom)

**Decisioni:**
- Token brand namespacizzati `brand-*` (`brand-dark #290808`, `brand-darker #120404`, `brand-light #c4807f`, `brand-lighter #ab2b29`, `brand-lightest #d23434`, `brand-cream #fdf6f6`, `brand-ink #020000`) per evitare collisioni con i token base di Tailwind (`white`/`black`)
- Palette intrinsecamente scura (sfondo `#290808`, testo chiaro `#fdf6f6`) → diventa l'unico tema dell'app; rimosse tutte e 3 le media query `prefers-color-scheme: dark` esistenti (nessun bisogno di doppio tema)
- Stati semantici (successo/errore/warning/STT status) **non** seguono la palette brand — restano i default Tailwind (`green-*`/`red-*`/`amber-*`/`gray-*`) per non perdere il significato convenzionale dei colori
- `pulse` rinominato `pulse-rec` nei `@keyframes` custom per non collidere con l'utility `animate-pulse` nativa di Tailwind
- `SPEAKER_COLORS`/`speakerColor()` (`src/lib/types.ts`) lasciati invariati — palette separata per distinguere speaker, non parte del design system brand

---

## 2026-06-07 — Cartelle configurabili: modello whisper + registrazioni audio

**Obiettivo:** Permettere all'utente di vedere e cambiare dove l'app mette il modello whisper (+ binary) e i file audio registrati, invece di averli nascosti/scelti a runtime con dialog.

**Fatto:**
- `SttSettings` esteso con `model_dir: Option<String>` e `recordings_dir: Option<String>` (`None` = default)
- Path helper in `commands.rs` ora prendono `&SttSettings` già caricato: `model_dir`, `local_bin_path`, `local_model_path`, nuove `default_recordings_dir`/`recordings_dir`
- Nuovi command: `get_recordings_dir` (path corrente), `pick_directory` (dialog `pick_folder`, riusato per entrambe le cartelle)
- `stop_recording`: rimosso il dialog "salva con nome" — ora scrive direttamente in `recordings_dir/recording-<unix_ts>.wav`, creando la cartella se serve
- `SettingsPanel.svelte`: due righe `.path-row` (modello / registrazioni), ciascuna con path corrente + "Cambia cartella" + "Mostra nel Finder"
- Doc aggiornati: `docs/backend/stt.md`, `docs/backend/commands.md`, `docs/frontend/ui.md`

**Decisioni:**
- Default scelti seguendo convenzioni macOS: `model_dir` resta in `app_data_dir` (dati persistenti non-utente, `~/Library/Application Support/...`), `recordings_dir` di default in `audio_dir()/Heedm/Records` = `~/Music/Heedm/Records/` (contenuto utente, visibile, indicizzato Spotlight; sottocartella `Records` separa le registrazioni da altri eventuali file app dentro `Heedm`)
- Nome file con timestamp leggibile locale invece di unix epoch (`Registrazione 2026-06-07 15.30.12.wav`) — aggiunta dipendenza `chrono` (già presente come transitiva, ora diretta) per `Local::now().format(...)`
- Cambio `model_dir` dopo download non fa migrazione automatica del file (~1.5GB): il pannello chiede conferma esplicita e avvisa che servirà riscaricare nella nuova posizione (`localReady` resettato a `false`)
- `stop_recording` salva sempre in automatico nella cartella configurata invece di chiedere ogni volta — coerente con l'idea di "cartella di default" e più semplice da spiegare

---

## 2026-06-07 — UI: finestra più grande + sezione modello in Settings

**Obiettivo:** Finestra leggermente più ampia; pannello impostazioni che mostra stato/percorso del modello Whisper locale e permette di riscaricarlo.

**Fatto:**
- `tauri.conf.json`: dimensioni finestra `480x360` → `560x420`
- Nessuna configurazione "trascrizione cloud" presente (verificato — già rimossa in setup iniziale, niente da fare)
- Nuovo command `get_local_model_path` (`commands.rs`) — ritorna percorso assoluto di `models/ggml-large-v3-turbo.bin` in `app_data_dir`
- `SettingsPanel.svelte`: mostra sempre stato installazione + percorso del modello su disco; pulsante "Scarica"/"Scarica di nuovo" (label dipende da `localReady`); pulsante "Mostra nel Finder" → `revealItemInDir` (`@tauri-apps/plugin-opener`)
- CSS aggiunto in `App.css`: `.model-path`, `.model-actions`, `.reveal-btn`
- Doc aggiornati: `docs/frontend/ui.md`, `docs/backend/stt.md`

**Decisioni:**
- Riuso di `tauri-plugin-opener` (già dipendenza) per "Mostra nel Finder" invece di aggiungere plugin nuovo

---

## 2026-06-07 — Setup linting/formatting con Biome

**Obiettivo:** Lint e format consistenti su TS/Svelte/JSON, sostituendo strumenti sparsi.

**Fatto:**
- Aggiunto `biome.json`: indent 2 spazi, virgolette doppie, organize imports on save
- `noUnusedImports`/`noUnusedVariables` disabilitati per `*.svelte` — Biome non parsa il markup Svelte, quindi segnala falsi positivi su import/variabili usati solo nel template
- Script `npm run lint` / `lint:fix` in `package.json`
- Primo pass di formattazione su tutto il repo (`App.css` e altri file riformattati)
- Doc aggiornato: `docs/frontend/ui.md`

**Decisioni:**
- Biome al posto di ESLint+Prettier separati — un solo tool, config unica, più veloce

---

## 2026-06-07 — Migrazione frontend React → Svelte

**Obiettivo:** Sostituire React con Svelte 5, ridurre dipendenze e bundle size.

**Fatto:**
- `App.tsx` (monolitico, 4 componenti inline) splittato in `App.svelte` + componenti separati sotto `src/lib/` (`SttIndicator`, `SettingsPanel`, `RecordingItem`, `TranscriptView`)
- Tipi e helper condivisi estratti in `src/lib/types.ts`
- `main.tsx` → `main.ts` (mount via `mount()` di Svelte invece di `ReactDOM.createRoot`)
- State management con runes Svelte 5 (`$state`, `$derived`, `$effect`, `$props`) al posto di `useState`/`useEffect`/`useRef`
- Rimosso `settingsRef` (stato write-only mai letto, già morto in origine — emerso con `noUnusedLocals` su Svelte)
- Build: `vite.config.ts` passa da `@vitejs/plugin-react` a `@sveltejs/vite-plugin-svelte`; aggiunto `svelte.config.js`; `tsconfig.json` estende `@tsconfig/svelte`; typecheck via `svelte-check` al posto di `tsc`
- Dipendenze rimosse: `react`, `react-dom`, `@vitejs/plugin-react`, `@types/react`, `@types/react-dom`
- Dipendenze aggiunte: `svelte`, `@sveltejs/vite-plugin-svelte` (v6, compatibile con vite 7), `@tsconfig/svelte`, `svelte-check`
- `index.html`: `#root` → `#app`, entry `main.ts`
- Bundle JS: ~42 KB (gzip 16 KB) vs build React precedente

**Decisioni:**
- `@sveltejs/vite-plugin-svelte@7` richiede vite 8 (peer dep) — usata `^6.2.4` che supporta vite `^7.0.0`
- Componenti separati per file (idiomatico Svelte) invece di mantenere monolite
- Callback props (`onClose`, `onSaved`, `onSettingsClick`) invece di `createEventDispatcher`, in linea con le API moderne Svelte 5

**Prossimi passi:**
- Verificare l'app in `tauri dev` (build/typecheck già passano puliti)

---

## 2026-06-07 — Setup iniziale repo

**Obiettivo:** Pulire boilerplate, strutturare repo, aggiungere documentazione.

**Fatto:**
- Rimosso Docker e modalità STT esterna — app usa solo whisper-server locale
- `SttSettings` semplificata: rimossi `mode` e `external_url`
- Eliminati asset default Vite/React inutilizzati (SVG, README boilerplate)
- Icona app impostata con `tauri icon` da sorgente custom
- Titolo app: "Heedm" (maiuscolo in `productName`, `title`, `Info.plist`, `index.html`)
- `CFBundleDisplayName` aggiunto a `Info.plist` per nome corretto nel Dock macOS
- CSS morti rimossi da `App.css` (`.mode-toggle`, `.mode-btn`, `.url-*`)
- Icone Android/iOS/Windows Store rimosse (app macOS-only per ora)
- Struttura documentazione creata (`CLAUDE.md`, `DEVLOG.md`, `docs/`)
- 5 commit organizzati per area logica

**Decisioni:**
- Linux e Windows mantenuti come target futuri (`linux.rs`, `windows.rs`, `icon.ico`)
- STT Docker rimosso definitivamente — complessità non giustificata per uso locale
- Docs-as-code in `docs/` con aggiornamento contestuale obbligatorio (regola in CLAUDE.md)

**Prossimi passi:** da definire
