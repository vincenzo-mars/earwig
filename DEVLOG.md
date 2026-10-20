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

## 2026-10-20 15-40 — Rimossa la modalità ridotta (edge drawer / mini bar)

**Obiettivo:** eliminare la visualizzazione mini a bordo schermo (linguetta edge drawer con il suo pulsante REC), su richiesta: l'unico modo di avviare una registrazione resta il REC della dashboard.

**Fatto:**
- Frontend: eliminato `CompactStrip.svelte`; rimossi stato `isCompact` e `setCompactMode` da `session.svelte.ts`, bottone `PanelRightClose` e la relativa branch/Classi `compact-mode` da `App.svelte`, stili `compact-mode` (sfondo trasparente) da `App.css`.
- Backend: da `commands/window.rs` rimossi `set_compact_mode`/`set_compact_hover`/`set_compact_y`/`save_compact_y`/`get_compact_position` con `WindowModeState`/`SavedWindowState` e le costanti di size; rimosso campo `compactY` da `SttSettings` (`mod.rs`); deregistrati i comandi e il manage dello state in `lib.rs`. Restano `set_dock_visible`/`show_dashboard`/`hide_dashboard` e la tray icon.
- Doc: rimosse le sezioni dedicate da `docs/architecture.md` e `docs/reference.md`, aggiornato `README.md`.

**Decisioni:**
- Tray icon (Impostazioni / Esci) e gestione dinamica del Dock restano: era la parte "sempre attiva" utile. Toltà solo la mini-barra a bordo schermo con la sua azione REC.
- `transparent: true` / `macOSPrivateApi` in `tauri.conf.json` lasciati intatti: non necessari ma innocui con lo sfondo solido, evitiamo cambi di config non richiesti.
- Campo `compactY` rimosso dallo struct; `settings.json` vecchi che lo contengono restano validi (serde ignora i campi sconosciuti).

**Prossimi passi:** nessuno, chiuso.

## 2026-10-20 15-10 — Trascrizione-first: preset modello int8/fp32 e ASR input ottimizzato

**Obiettivo:** far trascrivere le call con la stessa precisione di Handy (stesso modello Parakeet v3, input diverso). Tre leve decise con l'utente: input ASR senza gain, scelta di precisione int8/fp32, chunking sui silenzi.

**Fatto:**
- **Preset modello** (`stt.rs`, `mod.rs`): tabella `STT_PRESETS` (int8 ~671MB con dir storica, fp32 ~2.55GB con `encoder-model.onnx.data`), `sttModelId` in settings (serde default int8), `local_parakeet_dir`/`is_stt_model_ready` per preset, `download_local_model` sostituisce la dir del preset precedente e azzera la cache del motore, nuovo comando `get_stt_presets` per la UI.
- **Input ASR adattivo** (`recording.rs`, `audio.rs`): `channels_correlated` confronta gli inviluppi RMS mic/sistema (frame 50ms, lag ±500ms) per distinguere cuffie da altoparlanti. Con cuffie il canale Tu trascrive il mic nudo (zero processing); con altoparlanti il mic AEC-only (il leakage va rimosso, altrimenti le parole dell'altro finirebbero nel testo Tu). Il canale Altri è audio digitale pulito e va al modello com'è. Il segnale mic ottimizzato viene salvato come `mic_asr.wav` (mono 16k, senza gain): `run_transcription` lo preferisce quando esiste, con fallback alla catena storica per i WAV vecchi.
- **Gain solo sull'ascolto**: `balance_speech_levels`/`normalize_speech` restano sul WAV stereo salvato ma non toccano più l'input ASR (prima c'era un doppio AGC: GainController2 di WebRTC + il nostro boost fino a +20dB).
- **Chunking sui silenzi** (`stt.rs`): i blocchi da ~120s si spezzano nel frame più silenzioso di una finestra ±5s, niente più tagli a metà parola.
- **UI**: radio preset in `SettingsPanel` (download immediato al cambio) e due card in `Onboarding` (Consigliato pre-selezionato); `sttModelId` esposto in `types.ts`.

**Decisioni:**
- Il sistema audio resta registrato (l'utente registra call) e la diarizzazione Tu/Altri resta: la semplificazione non tocca il prodotto.
- AEC mantenuto solo quando serve davvero (leakage rilevato): con cuffie era processing gratuito che degradava la voce.
- Nessun URL custom per il modello: solo preset curati, scelto con l'utente.
- fp32 nel repo istupakov usa pesi esterni `.data`: se onnxruntime non li risolve, il preset si ritira (rischio monitorato).

**Prossimi passi:** smoke test record→transcribe su `tauri dev` (cuffie e altoparlanti), switch int8→fp32 con trascrizione reale, `fresh-install` per l'onboarding.

## 2026-10-20 14-45 — Fix qualità trascrizione: resample anti-alias nel path mic

**Obiettivo:** chiudere la diagnostica "il modello sembra più stupido": il test TTS a 48kHz (`say` + esempio `transcribe_wav`) ha confermato l'aliasing del resample lineare — sullo stesso audio il testo lineare conteneva errori ("dotte", "resempling") che con il passa-basso anti-alias spariscono.

**Fatto:**
- `recorder/audio.rs`: `resample_linear` sostituita da `resample` = FIR passa-basso (Hamming 63 tap, cutoff 0.45 × Nyquist destinazione) + interpolazione lineare. In upsample il filtro viene saltato. Call site aggiornati: mic in `stop_recording`, import file, resample in `run_transcription` per WAV non-16k.
- Verifica: `cargo check` verde; il confronto quantitativo è nello screenshot dei due testi del DEVLOG precedente (lineare con 2 errori di parola, anti-alias zero).

**Decisioni:**
- Filtro FIR scritto in casa invece di `rubato`: l'implementazione è già validata empiricamente dalla diagnostica, evita una dipendenza e le sue API stateful su un uso batch.
- Il filtro gira solo in downsample: in upsample l'interpolazione lineare è sufficiente (nessun aliasing possibile).

**Prossimi passi:** ripetere la registrazione reale con l'app installata aggiornata e confrontare la trascrizione con Handy; se restano differenze, resta in coda il preset fp32 (quantizzazione).

## 2026-10-20 14-30 — Diagnostica qualità trascrizione: mic raw + esempio A/B

**Obiettivo:** spiegare perché lo stesso Parakeet TDT v3 in Handy riconosce meglio. Fase 1 del piano: misurare se la pipeline audio di Earwig degrada l'input prima che arrivi al modello.

**Fatto:**
- `stop_recording` salva anche `mic_raw.wav` nella cartella della registrazione: mic f32 al sample rate nativo del device (48kHz tipico), prima di downmix/resample/AEC/balance. File di diagnostica, da rimuovere o nascondere dietro flag a indagine chiusa.
- Nuovo esempio `src-tauri/examples/transcribe_wav.rs`: trascrive un WAV arbitrario con `parakeet-rs` fuori dall'app. Su input non-16k stampa due trascrizioni: resample lineare (identico all'app) e resample con FIR passa-basso anti-alias (63 tap Hamming) prima della decimazione. Stereo: trascrive L (mic) e R (sistema) separatamente come l'app.
- Uso: `cargo run --example transcribe_wav -- <wav> [model_dir]`, model_dir default `~/Library/Application Support/com.vincenzomars.earwig/parakeet-tdt-0.6b-v3`.

**Ipotesi in ordine:** (1) resample lineare senza anti-alias aliasa >8kHz nel banda vocale, (2) AEC3 + balance fino a 10× degradano la voce, (3) quantizzazione int8 (il preset fp32 già pianificato la testa), (4) `to_mono` media i canali (-6dB su mono device stereo).

**Prossimi passi:** registrare una frase nota, confrontare i tre testi (Earwig, mic_raw lineare, mic_raw anti-alias) e Handy; fix guidato dall'esito (rubato anti-alias / clamp balance / preset fp32).

## 2026-10-20 13-45 — Linguetta compatta: animazione a padrone unico e Y persistente

**Obiettivo:** chiudere due voci aperte della linguetta ridotta: gli scatti nelle transizioni di hover (due animazioni in parallelo: CSS 200ms contro resize nativo istantaneo via IPC) e la posizione verticale che si azzerava a ogni avvio.

**Fatto:**
- **Scatti di hover:** il timing è ora governato dal frontend (`CompactStrip.svelte`). Il resize nativo parte a hover-start (crescita verso sinistra, invisibile sullo sfondo trasparente) e il restringimento viene rimandato di ~220ms dopo il mouseleave, quando la transizione CSS di larghezza è già finita; il timer è cancellato dal rientro del cursore, che ri-espande. `transition-all` ristretto a `transition-[width,border-color]` per non animare altro durante lo swap del contenuto.
- **Persistenza della Y:** campo `compactY` in `settings.json` (`#[serde(default)]`), nuovo comando `save_compact_y` invocato dal frontend a fine drag, `set_compact_mode` carica la Y da settings all'attivazione se in sessione non c'è ancora stato un drag.
- Documentazione aggiornata (`docs/architecture.md`, `docs/reference.md`) e voci 5 e 6 chiuse in `TODO.md`.

**Decisioni:**
- Singolo padrone dell'animazione: il CSS nel webview. L'alternativa (animare il resize nativo con NSAnimationContext via objc2) scartata: API non esposta da Tauri e complessità objc2 per lo stesso risultato visivo.
- Salvataggio a fine drag e non dentro `set_compact_y`: quel comando scorre a ogni pointermove e scrivere il file a ogni frame sarebbe inaccettabile.
- Il ritardo di restringimento è 220ms (sopra i 200ms della transizione): nel frattempo la finestra resta larga ma trasparente, e un rientro del cursore nella zona trasparente è intenzionalmente un ri-hover.

**Prossimi passi:** verifica visiva su `tauri dev` (hover, ri-hover entro i 220ms, drag seguito da riavvio con Y ripristinata).

## 2026-10-20 — Centratura orizzontale geometrica ed ottica dell'icona Play in Recorder

**Obiettivo:** Correggere la centratura orizzontale dell'icona Play (filigrana a riposo) nel pulsante REC principale della schermata home (`Recorder.svelte`), eliminando l'offset in eccesso verso destra introdotto in precedenza e bilanciando perfettamente margini e peso visivo.

**Fatto:**
- **Analisi geometrica SVG Lucide:**
  - Il path di `Play` in viewBox 24x24 si estende da `x=5.0` (base verticale) a `x=20.0` (vertice destro).
  - La bounding box è centrata a `x=12.5` (+0.5 unità / +1.83px a destra rispetto al centro 12.0 dell'icona da 88px).
  - L'applicazione precedente di `translate-x-0.5` (+2px) spingeva l'icona a un totale di +3.83px a destra, lasciando un margine sinistro di 40.3px e un margine destro di soli 32.7px (squilibrio visibile di ~8px).
- **Recorder (`src/lib/Recorder.svelte`):**
  - Rimosso `translate-x-0.5` e applicata la compensazione ottica calibrata `-translate-x-[1.5px]` (margine sinistro: 36.8px, margine destro: 36.2px, differenza < 0.6px).
  - Rimossa la classe `tracking-[0.14em]` ereditata dal bottone che poteva introdurre distorsioni di spaziatura inline.
- **Documentazione:**
  - Aggiornati `docs/reference.md` e `DEVLOG.md`.

## 2026-10-20 — Menu bar tray icon, gestione dinamica del Dock e avvio in background (Autostart)

**Obiettivo:** Rendere Earwig un'utility di sistema sempre attiva nella barra dei menu (menu bar macOS in alto a destra), con apertura/chiusura della dashboard al click, rimozione dal Dock quando chiusa/minimizzata (modalità accessorio), e avvio automatico in background all'accensione del Mac configurabile sia da Onboarding che dalle Impostazioni.

**Fatto:**
- **Sistema Tray e Menu Bar (`src-tauri/src/lib.rs`):**
  - Creata la `TrayIcon` di sistema con `TrayIconBuilder`, icona template monocromatica adattiva (`icon_as_template(true)`) e menu nativo contestuale (tasto destro: "Impostazioni..." ed "Esci da Earwig").
  - Click sinistro sulla tray: toggle rapido tra visualizzazione e nascondimento della dashboard principale.
- **Dock Dinamico macOS (`NSApplicationActivationPolicy` in `src-tauri/src/commands/window.rs`):**
  - Implementata la funzione `set_dock_visible`: usa AppKit via `objc2` per commutare `NSApplicationActivationPolicy::Regular` (quando la dashboard è aperta, presente nel Dock e in Cmd+Tab) e `NSApplicationActivationPolicy::Accessory` (quando la dashboard è chiusa o minimizzata, rimossa dal Dock e da Cmd+Tab).
  - Intercettazione della chiusura finestra (`CloseRequested`) e minimizzazione (`Focused(false)` con `is_minimized`): invece di uscire dall'app, la finestra si nasconde e passa in background nella menu bar.
  - Gestito l'evento `RunEvent::Reopen` per ripristinare la finestra se cliccata dall'utente nel Finder/Spotlight mentre è in background.
- **Avvio Automatico di Sistema (Autostart con `tauri-plugin-autostart`):**
  - Integrato `tauri-plugin-autostart` con launcher nativo `MacosLauncher::LaunchAgent` (`~/Library/LaunchAgents/`) e flag `--background`.
  - Configurate le capability di sicurezza in `src-tauri/capabilities/default.json` (`autostart:default`).
  - Finestra configurata con `visible: false` in `tauri.conf.json`: su avvio normale viene mostrata istantaneamente da `.setup()`, mentre su avvio con `--background` resta silente in memoria.
- **Frontend Svelte (`src/lib/`):**
  - `Onboarding.svelte`: aggiunto toggle interattivo abilitato per default per consentire l'avvio con il Mac al termine della configurazione iniziale.
  - `SettingsPanel.svelte`: aggiunta nuova sezione di vertice "Avvio di sistema" (icona `Power`) con switch reattivo sincronizzato con lo stato del LaunchAgent macOS.
  - `App.svelte`: aggiunto listener per l'evento `open-settings` inviato dalla tray icon.
- **Documentazione:**
  - Aggiornati `docs/architecture.md`, `docs/reference.md` e `DEVLOG.md`.

---

## 2026-10-20 — Icona fullscreen nuda e cliccabile nella linguetta compatta

**Obiettivo:** Rimuovere il bordo e il background circolare (round button) dal pulsante per espandere/fullscreen nella modalità ridotta, lasciando solo l'icona nuda e cliccabile con transizione all'hover.

**Fatto:**
- **CompactStrip (`src/lib/CompactStrip.svelte`):**
  - Rimosso l'utilizzo del componente `Button variant="icon"` per il pulsante `Maximize2`.
  - Sostituito con un `<button>` nativo privo di bordo circolare, background e ombre: solo icona nuda centrata (`size-7`), con colore discreto `text-ink-faint`, hover reattivo a `text-ink-primary` e micro-feedback `scale-[0.92]`.
  - Rimosso l'import non più utilizzato di `Button.svelte`.
- **Documentazione:**
  - Aggiornati `docs/architecture.md`, `docs/reference.md` e `DEVLOG.md`.

---

## 2026-10-20 — Semplificazione a 2 stati per la linguetta: rimozione stato intermedio e icona play poco a destra

**Obiettivo:** Rimuovere lo stato intermedio di prossimità non più utile, aumentare leggermente di qualche pixel la larghezza della linguetta iniziale a riposo e centrare l'icona Play poco verso destra.

**Fatto:**
- **CompactStrip (`src/lib/CompactStrip.svelte`):**
  - **Rimozione stato intermedio (Proximity):** Eliminato lo step intermedio a 85px. La linguetta ora ha solo due stati chiari: a riposo (Idle) ed espansa (Hover).
  - **Linguetta a riposo aumentata:** Portata da `w-[6px]` a `w-[12px]`, con dot discreto centrato verticalmente e bordo curvo morbido.
  - **Icona Play centrata poco a destra:** Sostituito l'offset a sinistra con `translate-x-0.5` per una perfetta centratura ottica coerente con l'icona Play della schermata principale.
  - Semplificata la gestione eventi: ascolto e transizione diretta tra `idle` ed `expanded`.
- **Backend Rust (`src-tauri/src/commands/window.rs`):**
  - Rimosso il loop in background su thread secondario che eseguiva polling continuo su `CGEventGetLocation` ogni 40ms.
  - Eliminate le definizioni C/FFI CoreGraphics non più necessarie (`CGEventCreate`, `CGEventGetLocation`, `CFRelease`, `CGPoint`).
  - Aggiornata la larghezza finestra per lo stato di riposo: `COMPACT_WIDTH_IDLE = 18.0`.
  - La funzione `set_compact_hover` ora gestisce direttamente il passaggio tra `COMPACT_WIDTH_IDLE` (18px) ed `COMPACT_WIDTH_EXPANDED` (86px).
- **Documentazione:**
  - Aggiornati `docs/architecture.md`, `docs/reference.md` e `DEVLOG.md`.

---

## 2026-10-20 — Ottimizzazione micro-spazi e rimozione timer nella barra ridotta

**Obiettivo:** Perfezionare la linguetta ridotta a bordo schermo: ridurre lo stato idle a pochissimi pixel (`w-[6px]`), eliminare il timer in registrazione mantenendo solo il tasto circolare di stop, azzerare lo spazio tra REC e maximize, centrare otticamente l'icona play e avvicinare i 6 puntini al bordo dello schermo.

**Fatto:**
- **CompactStrip (`src/lib/CompactStrip.svelte`):**
  - **Stato 1 (Idle):** Ridotto a sottilissima linguetta di pochissimi pixel (`w-[6px]`) con micro dot discreto (`size-1` a riposo, `size-1.5` durante la registrazione).
  - **Stato 2 (Prossimità):** Ridotto a `w-[32px]` con miniatura circolare del bezel e dot rosso.
  - **Stato 3 (Hover / Espanso):** Larghezza complessiva ridotta a soli `w-[82px]`:
    - Margine sinistro compresso (`pl-1`).
    - Eliminato completamente il timer in registrazione: sia REC sia STOP sono ora bottoni circolari identici `size-7`.
    - Icona `Play` centrata otticamente leggermente verso sinistra (`-translate-x-0.5`).
    - Spazio tra tasto REC/STOP e pulsante `Maximize2` azzerato (`gap-0`).
    - Icona a 6 puntini di trascinamento verticale `GripVertical` posizionata a filo del bordo destro dello schermo (`pr-0.5`, `w-4`).
- **Backend Rust (`src-tauri/src/commands/window.rs`):**
  - Unificate le geometrie della finestra per gli stati di idle e registrazione (grazie alla rimozione del timer): `COMPACT_WIDTH_IDLE = 16.0`, `COMPACT_WIDTH_PEEK = 38.0`, `COMPACT_WIDTH_EXPANDED = 86.0`.
  - Risolti i warning di variabili non usate nel loop di monitoraggio cursore.
- **Documentazione:**
  - Aggiornati `docs/architecture.md`, `docs/reference.md` e `DEVLOG.md`.

---

## 2026-10-20 — Rimozione scritta REC nella linguetta: pulsante circolare pulito

**Obiettivo:** Rimuovere completamente la dicitura testuale "REC" dalla linguetta in modalità ridotta, rendendo il mini pulsante una replica circolare e pura del bottone hero dell'app (solo cerchio rosso con watermark Play).

**Fatto:**
- **CompactStrip (`src/lib/CompactStrip.svelte`):**
  - Rimossa la stringa "REC" sia dallo Stato 2 (prossimità) sia dallo Stato 3 (hover).
  - Il pulsante REC a riposo è ora circolare (`size-7` con bezel concentrico esterno e watermark `Play`), perfettamente coordinato con il pulsante principale da 128px della home (anch'esso privo di testo).
  - Ridotta la larghezza del cassetto espanso a riposo da 176px a 124px (`w-[124px]`), per una barra ultra-compatta e bilanciata contenente il cerchio REC, il pulsante `Maximize2` e la presa a 6 puntini `GripVertical`.
- **Backend Rust (`src-tauri/src/commands/window.rs`):**
  - Aggiornate le costanti di larghezza della finestra: `COMPACT_WIDTH_EXPANDED_IDLE = 128.0`, `COMPACT_WIDTH_PEEK_IDLE = 48.0`.
- **Documentazione:**
  - Aggiornati `docs/reference.md` e `DEVLOG.md`.

---

## 2026-10-20 — Refinements barra ridotta: 6 puntini a destra e mini pulsante REC coordinato

**Obiettivo:** Posizionare l'icona di trascinamento a 6 puntini (`GripVertical`) all'estrema destra della barra espansa e coordinare lo stile grafico del pulsante REC/STOP piccolo con quello della hero nella schermata principale dell'app.

**Fatto:**
- **CompactStrip (`src/lib/CompactStrip.svelte`):**
  - **Mini pulsante REC/STOP:**
    - Ripreso il design concentrico a doppio anello dell'hero: bezel esterno `p-0.5 rounded-full border border-brand-cream/10 bg-brand-darker/60 shadow-raised`.
    - Pulsante interno in `bg-rec` (con hover `bg-rec-strong`, feedback tattile `scale-[0.96]`) e icona `Play` watermark centrata a opacità 20% con testo "REC".
    - Durante la registrazione: bezel `border-rec-strong/30 bg-rec-strong/10`, pulsante `bg-rec-strong`, icona `Square` di stop e cronometro `formatDuration` in font monospazio.
    - Negli stati 1 (punta smussata) e 2 (prossimità), la punta mostra la miniatura coordinata del bezel e del dot rosso.
  - **Disposizione controlli in hover (Stato 3):**
    - A sinistra: il pulsante REC/STOP coordinato.
    - Al centro/destra: pulsante icona `Maximize2` per espandere la finestra.
    - All'estrema destra: l'icona a 6 puntini `GripVertical` per il trascinamento verticale della linguetta.
- **Backend Rust (`src-tauri/src/commands/window.rs`):**
  - Calibrate le larghezze della finestra espansa (`COMPACT_WIDTH_EXPANDED_IDLE = 180.0`, `COMPACT_WIDTH_EXPANDED_REC = 192.0`) per alloggiare con equilibrio il nuovo pulsante REC e la maniglia a 6 puntini a destra.
- **Documentazione:**
  - Aggiornati `docs/reference.md` e `DEVLOG.md`.

---

## 2026-10-20 — Linguetta modalità ridotta: 3 stati puliti, grafica macOS premium e prossimità a ~85px

**Obiettivo:** Calibrare la distanza di prossimità da 200px a ~85px (con tolleranza verticale di 55px), per evitare attivazioni involontarie mentre si lavora su altre applicazioni e rendere l'interazione intenzionale e discreta.

**Fatto:**
- **Backend Rust (`src-tauri/src/commands/window.rs`):**
  - Definite le costanti `PROXIMITY_RANGE_X = 85.0` e `PROXIMITY_RANGE_Y = 55.0`.
  - La linguetta scivola verso sinistra (Stato 2 - Prossimità) solo quando il cursore entra intenzionalmente nell'area circostante la linguetta (entro 85px dal bordo destro e 55px dal centro verticale della linguetta).
- **Documentazione:**
  - Aggiornati `docs/architecture.md`, `docs/reference.md` e `DEVLOG.md`.

---

## 2026-10-20 — Linguetta modalità ridotta: 3 stati puliti, grafica macOS premium e prossimità a 200px

**Obiettivo:** Rimuovere qualsiasi animazione appariscente (ping, gradienti neon, glow) e implementare i 3 stati puliti richiesti per la linguetta a bordo schermo:
1. Non vicino (> 200px): si vede solo la punta sinistra smussata della linguetta con sobrio dot rosso fermo, grafica Mac premium coerente con l'app.
2. Prossimità (range entro 200px): la linguetta scivola verso sinistra con interpolazione fluida macOS per farsi prendere facilmente.
3. Hover: la linguetta si allarga a piena dimensione mostrando i controlli completi.

**Fatto:**
- **Backend Rust (`src-tauri/src/commands/window.rs`):**
  - Implementato il monitoraggio della posizione globale del cursore del mouse tramite le API CoreGraphics macOS (`CGEventGetLocation`), a impatto zero su CPU (loop leggero a 40ms quando la finestra è in modalità compatta).
  - Quando il cursore entra nel range di 200px orizzontali e 80px verticali rispetto alla linguetta, il backend allarga la finestra a 68px (`COMPACT_WIDTH_PEEK`) ed emette l'evento `compact-stage: proximity`.
  - Quando il mouse si allontana oltre 200px, la finestra torna all'ingombro minimo di 28px (`COMPACT_WIDTH_IDLE`) ed emette `compact-stage: idle`.
- **Frontend Svelte (`src/lib/CompactStrip.svelte`):**
  - Rimossa ogni animazione "fancy" (niente ping, pulsazioni o gradienti colorati).
  - Progettata una linguetta singola, smussata a sinistra (`rounded-l-2xl`), integrata nei colori scuri e caldi dell'app (`bg-brand-darker/95`, `border-brand-cream/15`).
  - Stato 1 (Idle): sporge solo la punta sinistra (`w-[14px]`) con dot rosso fisso.
  - Stato 2 (Proximity): scivola fluidamente verso sinistra a `w-[48px]` (`w-[62px]` se in REC con timer) con curva macOS `ease-[cubic-bezier(0.16,1,0.3,1)]`.
  - Stato 3 (Hover): si allarga a `w-[168px]` (`w-[176px]` se in REC) svelando i controlli (grip, REC/STOP, maximize).
- **Documentazione:**
  - Aggiornati `docs/architecture.md`, `docs/reference.md` e `DEVLOG.md`.

---

## 2026-10-20 — Icona a 3 puntini verticali a destra per lo spostamento della barra ridotta

**Obiettivo:** Riposizionare l'area di presa e trascinamento verticale della barra compatta sull'icona con 3 puntini verticali (`MoreVertical`) collocata all'estrema destra della barra una volta aperta.

**Fatto:**
- **CompactStrip (`src/lib/CompactStrip.svelte`):**
  - Sostituita l'icona precedente con l'icona a 3 puntini verticali `MoreVertical` (`size={16}`).
  - Posizionata la nuova icona a tutta destra all'interno della barra estesa (affiancata al pulsante `Maximize2`), lasciando i comandi primari (REC / STOP con timer) sulla sinistra.
  - Agganciati gli eventi pointer (`onpointerdown={handleGripPointerDown}`) e i cursori dedicati (`cursor-grab` e `cursor-grabbing` durante il drag) all'icona a 3 puntini.
- **Documentazione:**
  - Aggiornato `docs/reference.md`.

## 2026-10-20 — Transizione animata fluida da hover a non-hover nella modalità ridotta

**Obiettivo:** Eliminare lo scatto istantaneo al disimpegno del mouse dalla modalità compatta, implementando una transizione bidirezionale elastica e fluida sia in entrata che in uscita.

**Fatto:**
- **CompactStrip (`src/lib/CompactStrip.svelte`):**
  - Sostituito il toggle condizionale `{#if !isHovered}` con elementi persistenti nel DOM coordinati via stato reattivo `isExpanded`.
  - Applicata curva di decelerazione elastica `cubic-bezier(0.16, 1, 0.3, 1)` a 280ms su scala, traslazione orizzontale e opacità con micro-blur.
  - Al `mouseleave`, la card estesa scivola verso l'esterno verso destra mentre l'indicatore compatto (5px a riposo o 92px in REC) scivola in posizione.
  - Introdotto un timer di collasso (280ms) per sincronizzare il ridimensionamento della finestra OS Tauri (`set_compact_hover` con `hovered: false`) esattamente al completamento della transizione CSS, evitando tagli visivi o scatti anticipati.
  - Gestita l'interruzione continua: un rientro del mouse durante la fase di chiusura cancella il timer e inverte la transizione senza salti.
- **Documentazione:**
  - Aggiornato `docs/reference.md`.

## 2026-10-20 — Modalità ridotta a bordo schermo: prossimità ~50px, 5px a riposo, draggable e animazioni ricche

**Obiettivo:** Perfezionare la modalità ridotta a bordo schermo rendendola quasi impercettibile a riposo (5px sul bordo estremo dello schermo), reattiva alla prossimità del mouse (~50px), completamente spostabile in verticale (draggable) con clamping anti-collisione macOS (menu bar e Dock) e arricchita con animazioni elastiche, bagliori, micro-interazioni e glassmorphism.

**Fatto:**
- **Finestra trasparente nativa:**
  - Abilitato `transparent: true` nella configurazione Tauri (`tauri.conf.json`).
  - Gestita trasparenza tramite classe `.compact-mode` su `html` e `body` in `App.svelte` e `App.css`, consentendo al webview di essere trasparente nella zona di prossimità.
- **Rilevamento prossimità (50px) e animazioni:**
  - A riposo la finestra misura 52px con soli 5px visibili come indicatore luminoso con gradiente e glow ambrato/rosso (`animate-pulse`).
  - Appena il cursore entra nella zona di 50px dal bordo dello schermo (`onmouseenter`), la barra scivola fuori espandendosi a 168px con animazioni slide/fade, riverbero luminoso e pulsante REC animato con onde ping.
- **Spostamento verticale (Draggable lungo il bordo):**
  - Implementati comandi Tauri `set_compact_y` e `get_compact_position` in `src-tauri/src/commands/window.rs`.
  - Aggiunto grip handle `GripVertical` sia nello stato a riposo esteso sia in registrazione: trascinando il mouse, la finestra segue in tempo reale la coordinata Y, con clamping automatico che impedisce sovrapposizioni con la barra dei menu macOS in alto o con il Dock in basso.
  - La posizione verticale scelta viene memorizzata in `SavedWindowState` e preservata attraverso tutti i cambi di stato (collassato/espanso/registrazione).
- **Stato registrazione:**
  - A riposo: pillola compatta con bagliore rosso, dot pulsante `animate-ping` e timer.
  - Al passaggio del mouse: espansione fluida a 176px con pulsante STOP con timer, grip di spostamento e pulsante di espansione `Maximize2`.
- **Documentazione:**
  - Aggiornati `docs/architecture.md` e `docs/reference.md`.

## 2026-10-20 — Tipografia titoli accordion impostazioni più grande

**Obiettivo:** Rendere più grandi e leggibili i titoli delle sezioni accordion nel pannello impostazioni (`SettingsPanel`), migliorando la gerarchia visiva rispetto alle voci interne delle card.

**Fatto:**
- **AccordionSection (`src/lib/AccordionSection.svelte`):**
  - Incrementata la dimensione del font del titolo da `text-xs` (12px) a `text-sm` (14px, `font-semibold tracking-wide`), distinguendo chiaramente i titoli di sezione dal testo dei contenuti interni.
  - Adeguata otticamente la dimensione dell'icona a sinistra da 16px a 17px e dell'icona chevron da 14px a 15px.
- **Documentazione:**
  - Aggiornato `docs/reference.md`.

## 2026-10-20 — Fix sfondo bianco in modalità ridotta (WebKit window transparency su macOS)

**Obiettivo:** Risolvere il problema visivo per cui la modalità ridotta (edge drawer / mini bar a bordo schermo) mostrava un riquadro rettangolare bianco opaco attorno alla barra o all'indicatore invece di fondersi in modo trasparente con il desktop di macOS.

**Causa tecnica:**
- Su macOS, `WKWebView` possiede di default uno strato di rendering opaco bianco (`#FFFFFF`).
- Anche se il CSS imposta `background-color: transparent`, WebKit dipinge comunque il suo canvas bianco a meno che non venga invocata l'API privata `[WKWebView _setDrawsBackground:NO]`.
- In Tauri v2, questa chiamata è condizionata a due prerequisiti:
  1. Il feature flag `macos-private-api` sul crate `tauri` in `Cargo.toml`.
  2. La configurazione `"macOSPrivateApi": true` in `tauri.conf.json` sotto `"app"`.
- Inoltre, in `src/App.css` solo `body.compact-mode` azzerava lo sfondo, mentre l'elemento root `html` non aveva regole per `compact-mode`, permettendo al browser di ricorrere allo sfondo bianco predefinito.

**Fatto:**
- **Configurazione Tauri (`src-tauri/tauri.conf.json`):**
  - Aggiunto `"macOSPrivateApi": true` nella sezione `"app"` (affiancato a `"transparent": true` sulla finestra principale).
- **Stili CSS (`src/App.css`):**
  - Estesa la trasparenza in `compact-mode` su `html`, `body` e `#app` con `background: transparent !important`.
  - Impostato `bg-brand-dark` esplicitamente solo quando `not(.compact-mode)`.
  - Assicurato `height: 100%` anche sul contenitore `#app`.
- **Documentazione:**
  - Aggiornati `docs/architecture.md` e `DEVLOG.md`.

---

## 2026-10-20 — Palette colore: accordatura a un rosso caldo (ember/ambra scura) più scuro e profondo

**Obiettivo:** Abbassare la luminanza complessiva della palette mantenendo l'accordo caldo a base di rosso/ambra scura (`Hue ~12-14°`, `R > G > B`), per un look più profondo e scuro stile ossidiana.

**Fatto:**
- **Palette Tailwind (`src/App.css`):**
  - `--color-brand-dark`: `#211512` (sfondo app, luminanza ~10%).
  - `--color-brand-darker`: `#170f0c` (superfici e card).
  - `--color-brand-lighter`: `#3f2923`.
  - `--color-brand-lightest`: `#52362f`.
  - `--color-brand-light`: `#947a72`.
  - `--color-brand-cream`: `#f9f5f3`.
  - `--color-brand-cream-dim`: `#ebe3df`.
  - `--color-brand-ink`: `#0c0605`.
- **Documentazione:**
  - Aggiornati `docs/reference.md` e `DEVLOG.md`.

---

## 2026-10-20 — Header actions in RecordingDetail: pulsante copia nell'header e durata senza badge

**Obiettivo:** Spostare l'azione "Copia tutto" dal corpo della trascrizione all'header del dettaglio registrazione, e rimuovere lo stile badge (bordo, sfondo e padding da pillola) attorno al tempo di durata nell'header lasciando solo icona e testo.

**Fatto:**
- **RecordingDetail (`src/lib/RecordingDetail.svelte`):**
  - Rimossa la visualizzazione a badge attorno alla durata della registrazione nell'header: ora è un elemento pulito e discreto con sola icona `Clock` e testo numerico tabular monospace (`text-ink-secondary`).
  - Spostato il pulsante "Copia tutta la conversazione" (`Button variant="icon"` con icone `Copy` / `Check`) direttamente nelle `headerActions` della modale, sempre visibile e allineato alle altre azioni (`Folder`, `RotateCcw`).
  - Rimosso il container sticky galleggiante del pulsante di copia dall'area di lettura della trascrizione, pulendo il layout del corpo del testo.
- **Documentazione:**
  - Aggiornato `docs/reference.md`.

## 2026-10-20 — Note Markdown agnostiche nelle impostazioni

**Obiettivo:** Rendere la sezione dedicata all'esportazione delle note Markdown nelle impostazioni (`SettingsPanel`) e nell'onboarding (`Onboarding`) completamente agnostica rispetto ad applicazioni di terze parti, rimuovendo ogni menzione esplicita a Obsidian.

**Fatto:**
- **SettingsPanel (`src/lib/SettingsPanel.svelte`):**
  - Rinomata la sezione da `"Note Markdown & Obsidian"` a `"Note Markdown"`.
  - Aggiornata la descrizione: le note vengono esportate come file `.md` con frontmatter YAML e testo dei turni di dialogo, salvabili nella cartella predefinita o in una cartella a piacere.
  - Aggiornato il pulsante di selezione da `"Collega Vault Obsidian"` a `"Scegli cartella"`.
  - Rimossa la menzione di Obsidian dal titolo del file picker nativo macOS.
- **Onboarding (`src/lib/Onboarding.svelte`):**
  - Titolo e testi uniformati al linguaggio agnostico ("Note Markdown", "Scegli cartella per le note Markdown").
- **Documentazione:**
  - Aggiornato `docs/reference.md` per riflettere la titolazione agnostica.

## 2026-10-20 — Palette colore: grigi scuri con nota di rosso caldo

**Obiettivo:** Rimodulare la palette dei grigi scuri dell'app (`brand-dark`, `brand-darker`, `brand-lighter`, `brand-lightest`, `brand-cream`) per mantenere la stessa profondità e luminanza dei grigi attuali, infondendo al contempo una nota elegante e sottile di rosso/bordeaux coerente con l'accento REC.

**Fatto:**
- **Palette Tailwind (`src/App.css`):**
  - Spostata la tinta dei token `brand-*` da un sottotono oliva/giallastro a un sottotono rosso caldo (R > G/B di 6-12 punti):
    - `--color-brand-dark`: da `#1e1c19` a `#241a1c` (sfondo app).
    - `--color-brand-darker`: da `#141311` a `#191214` (card, modali, pannelli).
    - `--color-brand-lighter`: da `#3e3b37` a `#483538`.
    - `--color-brand-lightest`: da `#4e4a45` a `#5a4447`.
    - `--color-brand-light`: da `#94908a` a `#9e8a8d`.
    - `--color-brand-cream`: da `#f7f5f1` a `#f8f4f4` (bianco/crema pulito privo di dominanti gialle).
    - `--color-brand-cream-dim`: da `#e8e4de` a `#e8e1e2`.
    - `--color-brand-ink`: da `#0b0a08` a `#0f090a`.
- **Documentazione:**
  - Aggiornati `docs/reference.md` e `DEVLOG.md`.

---

## 2026-10-20 — Refactoring layout dettaglio registrazione: chat a tutta altezza e header actions

**Obiettivo:** Rimuovere la barra inferiore (bottom stripe bar) e la sidebar destra dal dettaglio della registrazione per offrire una visualizzazione a tutta larghezza e tutta altezza in stile conversazione GPT. Trasferire la durata con icona orologio (`Clock`) e il percorso cartella Finder con icona cartella (`Folder`) direttamente nell'header in alto a destra, affiancati dal pulsante di rilancio trascrizione (`RotateCcw`).

**Fatto:**
- **RecordingDetail (`src/lib/RecordingDetail.svelte`):**
  - Eliminata la barra inferiore (`<footer>` bottom nav): il corpo della modale è interamente dedicato alla lettura della conversazione.
  - Nell'header (`headerActions` di `Modal.svelte`):
    - Aggiunta la pillola della durata con icona tempo `Clock` e durata audio in font monospazio tabular.
    - Aggiunto il pulsante `Folder` (`Button variant="icon"`) per rivelare istantaneamente la cartella nel Finder (`revealItemInDir`).
    - Mantenuto il pulsante `RotateCcw` per rilanciare la trascrizione.
  - Nell'area di conversazione: mantenuto il pulsante sticky CTA "Copia tutto" a sola icona in alto a destra.
- **Documentazione:**
  - Aggiornato `docs/reference.md`.

---

## 2026-10-20 — Refinement RecordingsList: rimozione icona ripetuta a sinistra e fix click affidabile

**Obiettivo:** Rimuovere l'icona semantica ripetuta a sinistra su ogni record della lista registrazioni e rendere il click sulle righe fluido, immediato e privo di zone morte.

**Fatto:**
- **RecordingsList (`src/lib/RecordingsList.svelte`):**
  - Rimossa l'icona ripetuta a sinistra (`AudioWaveform` / `AlertCircle` nel box con bordo) per ogni record: il lato sinistro presenta direttamente il titolo formattato con la data naturale (`formatRecordingDate`) e la durata audio in font mono.
  - Per le registrazioni già trascritte (`status === "transcribed"`), l'intera riga è trasformata in un `<button type="button">` nativo a tutta larghezza (`w-full`): l'intera superficie della card (compresi padding ed aree tra testo e badge) è ora cliccabile con `cursor-pointer`, azzerando i punti morti che impedivano l'apertura al click sui bordi o padding del contenitore.
  - Stati `hover`, `active` e navigazione (`push("/detail/<name>")`) spostati sul `<button>` principale, preservando accessibilità e feedback tattile.
  - Per le registrazioni non trascritte o in errore, la riga rimane un `<div>` informativo con pulsante retry dedicato, prevenendo annidamenti non validi di tag button.
- **RecordingDetail (`src/lib/RecordingDetail.svelte`):**
  - Corretta la derivazione di `id`: evita `decodeURIComponent` ridondante quando `propId` è già decodificato da `App.svelte`, scongiurando errori a runtime `URIError` su nomi con caratteri speciali.
- **Documentazione:**
  - Aggiornato `docs/reference.md` per descrivere la nuova struttura della card e la rimozione dell'icona a sinistra.

## 2026-10-20 — Esportazione Markdown strutturata per Obsidian e configurazione Vault

**Obiettivo:** Generare per ogni trascrizione un file `.md` strutturato e ottimizzato per Obsidian (frontmatter YAML con metadati, link all'audio WAV e dialogo formattato con `**Tu:**` e `**Altri:**`), rendendo configurabile il percorso di destinazione (Vault Obsidian o cartella personalizzata) sia durante l'onboarding iniziale sia nelle impostazioni, mantenendo al tempo stesso il salvataggio predefinito nella cartella della registrazione (`transcript.md`).

**Fatto:**
- **Struttura Markdown (`src-tauri/src/commands/stt.rs`):**
  - Implementata la funzione `build_transcript_markdown`:
    - **Frontmatter YAML**: `title`, `duration` formattata (`mm:ss`), `tags: [trascrizione, earwig]`, `source: "audio.wav"`.
    - **Link audio**: `[Ascolta registrazione audio](file://...)` che punta al file WAV locale.
    - **Dialogo sequenziale**: accorpamento dei turni consecutivi con speaker in grassetto (`**Tu:**` e `**Altri:**`).
  - Al termine di `run_transcription`, il file viene scritto sempre come `transcript.md` nella cartella della registrazione, e se è configurata una `transcripts_dir` esterna (es. Vault Obsidian), viene duplicato lì come `<timestamp>.md` (`{entry_name}.md`).
  - In `list_recordings` (`recording.rs`), aggiunta la generazione retroattiva automatica di `transcript.md` per le registrazioni storiche che non ne dispongono ancora.
- **Configurazione e comandi backend (`src-tauri/src/commands/mod.rs` e `lib.rs`):**
  - Aggiunto `transcripts_dir: Option<String>` in `SttSettings` con `#[serde(default)]`.
  - Introdotti i comandi Tauri `get_transcripts_dir` e `set_transcripts_dir`.
- **Interfaccia Onboarding (`src/lib/Onboarding.svelte`):**
  - Una volta completato il download del modello vocale, l'onboarding presenta un passaggio dedicato per scegliere la destinazione delle note Markdown: consente di selezionare subito il Vault Obsidian tramite native file picker dialog (`@tauri-apps/plugin-dialog`) o proseguire con la cartella predefinita.
- **Pannello Impostazioni (`src/lib/SettingsPanel.svelte`):**
  - Aggiunta la sezione collassabile `AccordionSection` per "Note Markdown & Obsidian": visualizzazione del percorso con pillola cliccabile per aprire nel Finder (`revealItemInDir`), pulsante "Collega Vault Obsidian" / "Cambia cartella" e opzione per ripristinare la cartella predefinita.
- **Tipi e helper (`src/lib/types.ts`):**
  - Estesa `SttSettings` con `transcriptsDir: string | null`.
- **Verifiche & Documentazione:**
  - `npm run typecheck`: 0 errori e 0 warning.
  - `npm run lint:fix`: formattazione validata con Biome.
  - `cargo check`: superato senza warning.
  - Aggiornati `docs/architecture.md`, `docs/reference.md` e `DEVLOG.md`.

---

**Obiettivo:** Trasformare sia il dettaglio della registrazione (`RecordingDetail`) sia il pannello impostazioni (`SettingsPanel`) in finestre modali sovrapposte alla schermata principale (`Recorder`), evitando lo smontaggio della schermata home e garantendo continuità allo stato della sessione (REC, elaborazione audio, timer e scroll della lista registrazioni).

**Fatto:**
- **Componente modale condiviso (`src/lib/Modal.svelte`):**
  - Finestra di dialogo accessibile con `role="dialog"`, `aria-modal="true"` e stopPropagation sui click interni.
  - Comportamento responsive a **tutta finestra**: quando la finestra è troppo piccola (`innerWidth < 640` o `innerHeight < 600`), la modale occupa il 100% dell'area utile della finestra (`h-full w-full rounded-none border-0 shadow-none p-0`), azzerando i margini per massimizzare lo spazio su schermi compatti o dimensioni default dell'app (400x500). Quando la finestra è sufficientemente capiente, si trasforma automaticamente in modale flottante centrata con angoli arrotondati e ombra.
  - Backdrop scuro e sfocato (`bg-black/60 backdrop-blur-sm`) con transizione fluida `fade` (150ms).
  - Finestra centrata con bordo `border-brand-cream/10`, sfondo `bg-brand-dark`, ombra elevata `shadow-2xl` e transizione `scale` (start 0.96, 150ms) per l'effetto pop-in tipico delle finestre di dialogo native desktop macOS.
  - Header integrato con titolo uppercase, azioni opzionali e pulsante di chiusura circolare `Button variant="icon"` con icona `X`.
  - Gestione chiusura multipla: tasto `Escape` su window, click sul backdrop esterno (con controllo su `mousedown` per evitare chiusure accidentali durante selezioni testo all'interno), tasto `X` e pulsanti interni.
- **Dettaglio Registrazione (`src/lib/RecordingDetail.svelte`):**
  - Rimosso `PageHeader` a tutta pagina; il componente ora incapsula `Modal` (`maxWidth="max-w-5xl"`, altezza bloccata `h-[85vh]`).
  - Titolo dinamico formattato con la data naturale (`formatRecordingDate(entry.name)`).
  - Layout responsive dinamico: a due colonne su finestre ampie (`md:flex-row`), e a scorrimento verticale su finestre compatte (`flex-col`) con trascrizione sopra e dock metadati/azioni in basso.
  - Supporta le prop `id` e `onclose`.
- **Impostazioni (`src/lib/SettingsPanel.svelte`):**
  - Rimosso `PageHeader` a tutta pagina; il componente ora incapsula `Modal` (`maxWidth="max-w-xl"`).
  - Titolo fisso "Impostazioni" e scroll verticale interno per le sezioni collassabili.
  - Supporta la prop `onclose` e mantiene il deep-linking automatico a sezioni specifiche tramite querystring (`#stt-server`).
- **Guscio applicazione (`src/App.svelte` e `src/lib/routes.ts`):**
  - `Recorder` diventa la schermata base permanente sempre montata, evitando layout shift o interruzioni di stato audio/timer.
  - `App.svelte` ascolta reattivamente `router.location` per montare come overlay la modale impostazioni (quando `router.location === "/settings"`) o la modale dettaglio (quando `router.location.startsWith("/detail/")`).
  - La funzione `closeModal()` invoca `replace("/")` per chiudere le modali e riallineare l'hash URL.
- **Documentazione & Verifiche:**
  - Aggiornati `AGENTS.md` e `docs/reference.md`.
  - `npm run typecheck`: 0 errori e 0 warning.
  - `npm run lint:fix`: 0 errori Biome.
  - `npm run build`: build Vite completata con successo.
  - `cargo check`: check completato con successo.

---

## 2026-10-20 — Refactoring pagina dettaglio: conversazione fluida stile GPT e sidebar metadati essenziale

**Obiettivo:** Snellire radicalmente la vista di dettaglio della registrazione: rimuovere i box superiori, i bordi e i badge degli interventi; formattare la trascrizione a sinistra come una conversazione fluida stile ChatGPT con solo "Tu:" e "Altri:"; inserire una sidebar a destra con sole scritte per "Stato", "Durata" e "Posizione file".

**Fatto:**
- **PageHeader (`src/lib/PageHeader.svelte`):**
  - Reso il titolo puramente condizionale (`{#if title}`): non viene mostrato alcun titolo di fianco al pulsante Indietro quando omesso o vuoto.
- **Accesso vincolato allo stato trascritto (`src/lib/RecordingsList.svelte` e `RecordingDetail.svelte`):**
  - In `RecordingsList`, solo le registrazioni con trascrizione completata (`status === "transcribed"`) sono cliccabili ed evidenziate da `ChevronRight`.
  - In `RecordingDetail`, redirect automatico alla home (`replace("/")`) per id senza trascrizione disponibile.
  - Rimossa la voce "Stato" dalla sidebar: chi naviga nel dettaglio è per definizione su una registrazione già trascritta con successo.
- **Colori Speaker (`src/lib/TranscriptView.svelte` e `src/lib/types.ts`):**
  - `"Tu:"`: colore Rosso (`text-red-400`, allineato a destra).
  - `"Altri:"`: colore Viola (`text-purple-400`, allineato a sinistra).
- **Vista Dettaglio & Sidebar essenziale (`src/lib/RecordingDetail.svelte`):**
  - Eliminato il box superiore e la griglia di metriche.
  - Rimosso il tempo "Parakeet:" sotto la durata (resta solo la durata audio).
  - Allargata la sidebar a destra di 150px (`w-80`, 320px).
  - Allineata la riga "Percorso:" all'identico layout e pillola interattiva con micro-hover delle Impostazioni (`border-brand-cream/8 bg-brand-cream/[0.03]`).
  - Spostate le azioni "Copia tutto" e "Rilancia" dal PageHeader direttamente all'interno della sidebar a destra.
  - PageHeader pulito senza titolo né azioni laterali.
- **Documentazione & Verifiche:**
  - Aggiornati `docs/reference.md` e `DEVLOG.md`.
  - `npm run typecheck`: 0 errori e 0 warning.
  - `npm run lint:fix`: 0 errori Biome.
  - `cargo check`: completato con successo.

---

## 2026-10-20 — Refactoring UI: accordion unificato Server STT, disattivazione LLM e redesign elenco registrazioni lineare

**Obiettivo:** Snellire e pulire l'interfaccia utente: disattivare la chat/riassunto LLM in attesa del modello locale futuro, unificare la configurazione del motore STT in un unico accordion coerente ("Server STT"), rendere i percorsi cartella interattivi senza icone rindondanti, e ridisegnare l'elenco delle registrazioni secondo uno stile lineare e minimalista (ispirato a Things 3 / Linear / Raycast).

**Fatto:**
- **Disattivazione LLM:**
  - Rimosso `TranscriptNotes` e i controlli LLM da `RecordingDetail.svelte`, lasciando la vista focalizzata sulla trascrizione per speaker e timestamp.
  - Rimossa la colonna/sezione LLM da `SettingsPanel.svelte`.
  - Disattivato il check automatico `servers.refreshLlm()` al boot in `App.svelte`.
- **Armonizzazione colori e superfici tra Settings ed Elenco Registrazioni:**
  - Aggiornato `AccordionSection.svelte` per adottare lo stesso identico contenitore `bg-brand-dark/40`, bordo `border-brand-cream/8` e raggio `rounded-lg` della lista registrazioni, eliminando lo stacco con il fondo scuro `bg-brand-darker`.
  - Rimossa la nidificazione superflua di box dentro box in `SettingsPanel`: i permessi e le righe del server STT sono ora organizzati direttamente come liste divise da `divide-brand-cream/6`, con padding `px-3.5 py-2.5` e hover uniforme `hover:bg-brand-cream/[0.04]`.
  - Rimosso il doppio spinner in elaborazione nell'icona sinistra della lista, mantenendo solo il loader sul badge di stato a destra.
- **Redesign Elenco Registrazioni (Opzione 2: Lineare & Minimalista):**
  - In `src/lib/types.ts`: introdotto `formatRecordingDate(name)` che converte i timestamp grezzi `YYYY-MM-DD HH.mm.ss` in date umane relative ("Oggi, 14:23", "Ieri, 09:12", "18 Ott, 16:45").
  - In `src/lib/RecordingsList.svelte`: sostituite le card singole con un elenco compatto `divide-y divide-brand-cream/6` all'interno di un unico guscio scuro (`bg-brand-darker` con bordo fine).
  - A sinistra: icona circolare `FileAudio` con colore semantico in base allo stato di trascrizione (verde per trascritta, arancione pulsante durante il salvataggio/trascrizione, rosso per fallita).
  - Al centro: data naturale leggibile affiancata dalla durata audio in font monospazio (`tabular-nums`).
  - A destra: badge di stato discreto con dot colorato, cronometro elapsed/tempo impiegato, bottone a icona `RotateCcw` per riprovare se fallita o pending, e `ChevronRight` con micro-transizione orizzontale al passaggio del mouse.
- **Verifiche:**
  - `npm run typecheck`: 0 errori e 0 warning.
  - `npm run lint:fix`: formattazione pulita e verificata con Biome.
  - `cargo check`: superato senza warning.
- Aggiornati `docs/reference.md` e `DEVLOG.md`.

---

## 2026-10-20 — Migrazione motore STT a NVIDIA Parakeet TDT 0.6B v3 in-process (ONNX)

**Obiettivo:** Sostituire whisper-server (Whisper Large-v3-Turbo su porta 8080) con NVIDIA Parakeet TDT 0.6B v3 in-process tramite `parakeet-rs` e ONNX Runtime, per migliorare radicalmente l'accuratezza della trascrizione in italiano, la punteggiatura e l'affidabilità della diarizzazione, eliminando al tempo stesso processi esterni, porte di rete (8080), lockfile e gestione di processi orfani.

**Fatto:**
- Integrato `parakeet-rs = "0.4.1"` in `src-tauri/Cargo.toml` con backend ONNX Runtime (`ort`).
- In `src-tauri/src/commands/stt.rs`:
  - Riscritto completamente il modulo: rimosso l'avvio del processo esterno `whisper-server`, le chiamate multipart HTTP e i lockfile su porta 8080.
  - Implementato `SttEngineState` (gestito come state in Tauri `lib.rs`) con caching in-process `Option<Arc<ParakeetTDT>>`.
  - Implementato il download sequenziale multi-file dei componenti del modello (`vocab.txt`, `decoder_joint-model.int8.onnx`, `encoder-model.int8.onnx` da Hugging Face `istupakov/parakeet-tdt-0.6b-v3-onnx`, ~670 MB totali) con file temporanei `.part`, rename atomico e notifica unificata su `download-progress`.
  - Implementata la trascrizione a chunk di 120 secondi (1.920.000 campioni a 16 kHz) con `TimestampMode::Sentences`, garantendo segmenti naturali e punteggiatura nativa senza picchi di memoria su audio lunghi.
  - Implementata la diarizzazione fisica deterministica: per registrazioni stereo, i canali L (mic/YOU) e R (sys/THEM) vengono de-interlacciati e trascritti separatamente, assegnando rigorosamente `speaker: "0"` per il microfono e `speaker: "1"` per il sistema, unendo e ordinando i segmenti cronologicamente.
  - Per registrazioni mono (es. audio importati), trascrizione del singolo canale con `speaker: None`.
- In `src-tauri/src/commands/mod.rs`:
  - Aggiunte le funzioni `local_parakeet_dir` e `is_stt_model_ready`.
  - Aggiornato `get_stt_settings` per verificare la presenza di tutti e tre i file ONNX/vocab sul disco.
  - Rimosso `STT_PORT` non più utilizzato.
- In `src-tauri/src/commands/recording.rs`:
  - Aggiornato `ensure_stt_ready` per basarsi sulla presenza del modello con `is_stt_model_ready`.
- In `src-tauri/src/lib.rs`:
  - Sostituito `WhisperServerState` con `SttEngineState`.
  - Rimosso il codice di terminazione di `whisper-server` all'uscita dell'app (`ExitRequested`), mantenendo solo quello per `llama-server`.
- In `src/lib/SettingsPanel.svelte` e `src/lib/SttIndicator.svelte`:
  - Aggiornati testi e label: "Modello vocale (STT)", "Parakeet TDT 0.6B v3 (~670 MB)", "Motore STT (In-process)", "STT pronto", "STT non scaricato".
- Benchmark prestazionale:
  - 124.35 secondi di audio in italiano trascritti in 4.54 secondi (~27.4x real-time) su Apple Silicon con segmentazione fluida in frasi e punteggiatura naturale.
- Aggiornati `docs/architecture.md` e `docs/reference.md`.

**Decisioni:**
- **In-process via `parakeet-rs` invece di un server HTTP:** elimina la necessità di compilare o gestire `whisper-server`, azzera overhead di rete e serialization multipart, e garantisce che l'inferenza condivida il ciclo di vita dell'app Tauri senza processi zombie.
- **Chunking a 120s:** evita overhead eccessivi di memoria nell'encoder FastConformer per file molto lunghi, mantenendo al contempo un contesto sufficientemente ampio per la decodifica del linguaggio e della punteggiatura.
- **Diarizzazione per canale hardware:** grazie all'AEC3 e al bilanciamento RMS implementati in precedenza, i due canali stereo contengono voci pulite e separate. Trascrivere i canali indipendentemente garantisce speaker attribution al 100% deterministica, senza errori di clustering statistico.

**Prossimi passi:**
- Test end-to-end e verifica con registrazione dal vivo.

---

## 2026-10-19 — Stato di caricamento inline nella lista registrazioni e rimozione doppio loader

**Obiettivo:** Rimuovere il doppio loader (spinner nel bottone REC e pillola nel dock) durante il processing dell'audio. Mostrare la fase di cancellazione rumore e quella di trascrizione direttamente nella riga della registrazione nell'elenco, con badge dedicati e card non cliccabile durante il caricamento, eliminando la dicitura separata "trascrizione in corso" dal dock.

**Fatto:**
- In `src-tauri/src/commands/recording.rs`:
  - `stop_recording` accetta il parametro opzionale `timestamp: Option<String>` e lo passa a `new_recording_path`: questo garantisce che il nome della cartella generato dal frontend all'istante di STOP coincida esattamente con il filesystem, senza discrepanze temporali.
- In `src/lib/types.ts`:
  - Aggiunta funzione `formatTimestamp` (formato `%Y-%m-%d %H.%M.%S`).
  - Definiti i tipi `ProcessingPhase` (`"noise_cancellation" | "transcribing"`) e `ProcessingItem`.
- In `src/lib/stores/session.svelte.ts`:
  - Aggiunto lo stato `processingItem` che traccia l'elemento in corso (`name`, `folderPath`, `phase`).
  - In `stopRecording`: calcolato il timestamp, impostato `processingItem` su `phase: "noise_cancellation"`, invocato `stop_recording`, poi passato a `phase: "transcribing"` prima di avviare `runTranscription`.
  - In `importFile`: impostato `processingItem` su `phase: "transcribing"` durante la trascrizione del file importato.
- In `src/lib/stores/recordings.svelte.ts`:
  - `recordings.all` deriva la lista includendo in testa l'eventuale `session.processingItem` prima ancora che esista su disco: la card compare istantaneamente al click di STOP.
  - In `retryTranscription`: impostato `processingItem` inline per mostrare lo stato di caricamento direttamente sulla riga selezionata.
- In `src/lib/Recorder.svelte`:
  - Rimosso lo spinner concorrente all'interno del pulsante REC (il pulsante resta disabilitato/opaco in idle col Play icon).
  - Rimosse le pillole separate di salvataggio e trascrizione dal dock di stato: nessun messaggio "a parte", il feedback vive solo nella card in lista.
- In `src/lib/RecordingsList.svelte`:
  - Quando un elemento corrisponde a `session.processingItem`, la riga è disabilitata (non cliccabile, `disabled:cursor-default`).
  - Il badge di stato riflette la fase:
    - `"cancellazione rumore..."` con badge giallo/warn e mini-spinner durante l'AEC/elaborazione WAV.
    - `"trascrizione in corso..."` con cronometro trascorso e mini-spinner durante Whisper.
  - Al completamento, la card passa automaticamente al badge verde `"trascritto"` e diventa cliccabile.
- Aggiornata la documentazione in `docs/reference.md`.

---

## 2026-10-19 — Automatic Gain Control (AGC2) e bilanciamento RMS tra microfono e sistema

**Obiettivo:** Risolvere il problema della voce dell'utente troppo bassa sul canale sinistro rispetto all'audio digitale di sistema sul canale destro (15-20 dB di dislivello), causa principale di allucinazioni ed errori di trascrizione in Whisper e di speaker attribution sbilanciata.

**Fatto:**
- In `src-tauri/src/recorder/aec.rs`:
  - Attivato `GainController2::default()` (WebRTC AGC2) nella configurazione di `sonora`: applica controllo adattivo digitale del guadagno guidato da Voice Activity Detection (RNN) e limiter anti-clipping.
- In `src-tauri/src/recorder/audio.rs`:
  - Implementate le primitive `active_speech_rms` (calcolo energia RMS sulle porzioni di parlato attivo ignorando i silenzi), `normalize_speech` (normalizzazione con guadagno controllato e peak limiter a 0.95) e `balance_speech_levels` (allineamento dell'RMS del mic al livello dell'audio di sistema).
  - Aggiunti test unitari per verificare clamping, assenza di distorsione e pareggiamento dei livelli.
- In `src-tauri/src/commands/recording.rs`:
  - Applicato `balance_speech_levels` sul buffer mic prima della composizione del file stereo `interleave_stereo`: entrambi i canali hanno ora presenza ed energia paragonabili.
- Aggiornati `docs/architecture.md` e `docs/reference.md`.

---

## 2026-10-19 — Disattivazione opaca del bottone registrazione e rimozione label "Already recording"

**Obiettivo:** Evitare la visualizzazione della label di errore "Already recording" e disattivare visivamente in modo opaco (`disabled`, `opacity-40`, ombra rimossa, `cursor-not-allowed`) il pulsante di registrazione quando registrare non è consentito.

**Fatto:**
- In `src/lib/stores/session.svelte.ts`:
  - Aggiunto lo stato `isStarting` (speculare a `isStopping`) per coprire la latenza dell'IPC `start_recording`.
  - Aggiornato `locked` (`isRecording || isStarting || isStopping || isTranscribing`): nessun altro flusso può interferire mentre un'operazione è in corso.
  - In `startRecording` e `stopRecording`: prevenute chiamate concorrenti/doppi click, e gestito l'errore backend ("Already recording" / "Registrazione già in corso") allineando silenziosamente lo stato a `isRecording = true` senza propagarlo come errore UI.
- In `src/lib/Recorder.svelte`:
  - Calcolato `isRecordDisabled`: quando la registrazione non è attiva, il bottone è disabilitato se il modello non è pronto, se whisper-server è fermo, se c'è una trascrizione in corso o se `isStarting`/`isStopping` è attivo.
  - Aggiornato lo stile disabled del pulsante REC: bezel a `opacity-40 shadow-none border-brand-cream/5 bg-brand-darker/40`, bottone con `disabled:cursor-not-allowed disabled:shadow-none` e transizione fluida, rendendo il controllo visibilmente opaco e non interattivo.
  - Filtrato il dock degli errori affinché eventuali stringhe "Already recording" / "Registrazione già in corso" non compaiano mai come label rossa d'errore.
  - Tooltip dinamico su REC: spiega la specifica ragione di disattivazione ("Trascrizione in corso", gate STT, o stato di transizione).
- In `src-tauri/src/commands/recording.rs`:
  - Localizzate le stringhe d'errore in italiano (`"Registrazione già in corso"` e `"Nessuna registrazione in corso"`).
- Aggiornata la documentazione in `docs/reference.md`.

---

## 2026-10-19 — WebRTC AEC3 via Sonora per l'eliminazione dell'echo da altoparlanti

**Obiettivo:** Risolvere il problema dell'audio di sistema (voce dell'interlocutore nelle chiamate Teams/Meet) che rientra nel microfono quando si registra senza cuffie, degradando e falsando la diarizzazione L/R di Whisper.

**Fatto:**
- Sostituito l'AEC euristico semplificato in `src-tauri/src/recorder/aec.rs` (cross-correlazione a ritardo unico + attenuazione scalare) con **WebRTC AEC3** tramite il crate puro-Rust `sonora` (`0.2.0`).
- Configurato il pipeline con:
  - Echo Canceller 3 (AEC3): filtro adattivo a partizioni di frequenza con stima automatica del ritardo e gestione double-talk.
  - Noise Suppression (NS): soppressione moderata del rumore stazionario di fondo.
  - High-Pass Filter (HPF): eliminazione DC offset e frequenze sub-basse.
- L'elaborazione opera a blocchi da 10ms (160 campioni a 16 kHz), alimentando il canale di sistema nel percorso *render* e il canale microfono nel percorso *capture*.
- Zero impatto sull'interfaccia pubblica: la firma `cancel_echo(mic: &[f32], sys: &[f32], sample_rate: u32) -> Vec<f32>` resta identica, eseguita in `spawn_blocking` a fine registrazione.
- Aggiornata la documentazione in `docs/architecture.md`.

---

## 2026-10-19 — Centratura icone nelle CTA e varianti di Button

**Obiettivo:** Risolvere il problema delle icone decentrate (spostate verso il basso a destra) nei pulsanti CTA a icona compatta della schermata Impostazioni (`size-7`), e garantire una centratura perfetta per tutti i pulsanti.

**Fatto:**
- In `Button.svelte`:
  - Aggiunto `inline-flex items-center justify-center` a tutte le varianti (`ghost`, `icon`, `solid`, `danger`, `primary`).
  - Aggiornata la variante `icon` con `shrink-0 size-8 p-0`: il default a 32px con padding nullo consente a `tailwind-merge` di sovrascrivere la dimensione con classi come `size-7` (28px) senza che un padding ereditato (`p-2`, non sovrascritto da utilities di dimensione) riduca l'area utile e provochi l'overflow e il decentramento delle icone.
- In `ServerControls.svelte`:
  - Aggiunta la compensazione ottica `translate-x-px` sull'icona `Play` per bilanciare il baricentro del triangolo asimmetrico rispetto ai pulsanti simmetrici (`RotateCcw`, `Square`, `Trash2`).
- Aggiornata la documentazione in `docs/reference.md`.

---

## 2026-10-19 — Riprogettazione UI pagina impostazioni e check permesso microfono

**Obiettivo:** Riorganizzare la schermata `/settings` con sezioni collassabili (aperte di default senza animazione al landing), icone tematiche, titoli più grandi, check verde per il microfono, riordino sezioni ("Cartella Registrazioni" dopo "Permessi"), layout informativo coerente con CTA a sole icone compatte per i modelli e i server, e raggruppamento dell'LLM in una sezione unificata.

**Fatto:**
- **Backend Rust (`permissions.rs`, `commands/mod.rs`, `lib.rs`)**:
  - Implementato `check_microphone_permission` su macOS tramite FFI nativo su `AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio` (`libobjc` + `AVFoundation`), senza aggiungere dipendenze esterne.
  - Registrato il nuovo comando Tauri `commands::check_microphone_permission`.
- **Sezioni Collassabili animate (`AccordionSection.svelte`, `SettingsPanel.svelte`)**:
  - Creata la componente riutilizzabile `AccordionSection.svelte` con `transition:slide={{ duration: 200 }}` di Svelte per una transizione fluida e naturale in altezza sia in apertura che in chiusura, senza alcun effetto di transizione indesiderato al caricamento iniziale (le sezioni iniziano già aperte).
  - Sfondo delle accordion impostato a `bg-brand-darker` con bordo `border-brand-cream/10` per uno stacco visivo poco più scuro rispetto al fondo pagina (`bg-brand-dark`), in armonia con le `.card` dell'app.
  - Rotazione della freccia `ChevronDown` sincronizzata a 200ms.
  - Auto-scroll da deep-linking (`router.querystring` con parametro `section`) aggiornato con `openStt` e `openLlm` per espandere automaticamente la sezione target se chiusa.
- **Sezione Permessi**:
  - Rilevamento in tempo reale di entrambi i permessi (Microfono e Registrazione Schermo) con badge "Concesso" (`text-ok` con check) e "Non concesso".
  - Hover evidenziato e rifinito (`hover:bg-brand-cream/8 hover:border-brand-cream/20`) con cursore puntatore e tooltip espliciti.
  - Aggiornamento automatico dei permessi al focus della finestra dell'app.
- **Cartella Registrazioni**:
  - Spostata immediatamente dopo "Permessi".
  - Riga percorso testuale cliccabile e pulsante a icona `Folder` in linea per rivelare la cartella nel Finder.
- **Sezione Modello Whisper**:
  - Riga "Nome:" con il nome del modello (`Whisper large-v3-turbo (~1.5 GB)`) e CTA per il download/redownload ridotta a icona in linea (`Download` / `RotateCcw` / `LoaderCircle`).
  - Riga "Stato:" con dot semantico coerente e label di stato.
  - Riga "Percorso:" con path testuale cliccabile e pulsante `Folder` in linea.
  - Barra di download visibile sotto la card solo durante il download.
- **Sezione Server STT (Speech To Text)**:
  - Titolo aggiornato con specifica "(Speech To Text)".
  - Controlli server ridisegnati in `ServerControls.svelte`: riga di stato con dot semantico coordinato e pulsanti azione ridotti a sole icone compatte in linea (`Play`, `RotateCcw`, `Square`, `Trash2`).
- **Sezione "LLM locale per chat"**:
  - Raggruppate tutte le funzionalità LLM (info RAM, card modello GGUF selezionato con download a icona e stato coerente, ricerca Hugging Face, e controlli del server LLM con icone compatte) in una macro-sezione collassabile nella colonna destra.
- **Documentazione**:
  - Aggiornati [`docs/reference.md`](reference.md) con la firma del nuovo comando `check_microphone_permission` e la descrizione della nuova architettura UI di `SettingsPanel.svelte` e `ServerControls.svelte`.

---

## 2026-10-19 — Navigazione da indicatore server attivo a sezione impostazioni

**Obiettivo:** Rendere l'indicatore di stato del server (`SttIndicator`) interattivo esclusivamente per aprire la pagina delle impostazioni e posizionarsi direttamente sulla sezione del server STT.

**Fatto:**
- In `SttIndicator.svelte`: sostituito il refresh manuale con la navigazione `push("/settings?section=stt-server")`. Aggiornati tooltip (`title="Apri impostazioni server"`) e `aria-label` per descrivere chiaramente l'affordance. Rimossa la dipendenza diretta dallo store `servers`.
- In `SettingsPanel.svelte`: importato `router` da `svelte-spa-router`, introdotto `$effect` reattivo che verifica il parametro `section` nella querystring e, a caricamento completato delle impostazioni, esegue `scrollIntoView({ behavior: "smooth", block: "start" })` sull'ID di destinazione.
- Aggiunti gli ID ancora `id="stt-server"` e `id="llm-server"` con classe Tailwind `scroll-mt-4` attorno ai blocchi `ServerControls`.

---

## 2026-10-19 — Ingrandimento REC, timer centrato e stop watermark minimale

**Obiettivo:** Ingrandire il pulsante REC, centrare verticalmente il cronometro aumentandone la dimensione, rimuovere il testo "STOP" e visualizzare il quadrato di stop solo come watermark di background sotto il timer, poco visibile e più stretto della stringa del timer.

**Fatto:**
- Ingrandito il pulsante REC/STOP a `h-32 w-32` (128px), con proporzioni aumentate sia per lo stato idle (`text-lg font-bold tracking-[0.14em]` e dot `size-3`) sia in registrazione.
- Rimossa la label testuale `STOP`, lasciando che sia la forma semantica a comunicare l'affordance.
- Cronometro della durata ingrandito a `text-lg font-bold tabular-nums` e centrato esattamente sia in orizzontale che in verticale (`leading-none`).
- Inserito il quadrato dello STOP in background sull'asse z (`absolute inset-0 m-auto size-16 rounded-lg bg-black/10 aria-hidden="true"`): opacità discreta confermata al 10% e dimensione aumentata a 64px (`size-16`), per un bilanciamento proporzionale ideale rispetto al cerchio da 128px e alle cifre del timer.
- Rimossa la label testuale "REC" e il cerchietto bianco a riposo: il pulsante mostra unicamente il triangolo del PLAY (`Play size={88} fill="currentColor" text-black/10` con compensazione ottica a destra calibrata a `translate-x-0.5` e `scale-y-[1.08]`), offrendo un'estetica hardware pura, minimale e perfettamente bilanciata al centro del cerchio.
- Reso cliccabile `SttIndicator` in alto a sinistra per forzare manualmente il refresh dello stato del server locale (`servers.refreshStt()`), e aggiunto in `App.svelte` un listener sull'evento `focus` della finestra per rieseguire automaticamente il check del server ad ogni ritorno nell'app.
- Rimossi rilievi 3D artificiali, sfumature e bagliori luminosi (glow): uniformato lo stile del pulsante REC a quello pulito e minimale degli altri bottoni dell'app, con transizione a 200ms di colore pieno (`bg-rec` → `enabled:hover:bg-rec-strong`), ombra neutra `shadow-raised`, micro-scale (`scale-[1.02]`) e depressione al click (`active:scale-[0.96]`).


## 2026-10-19 — Timer integrato nel pulsante REC su tasto stop opaco

**Obiettivo:** Mostrare il cronometro di registrazione direttamente dentro il pulsante REC/STOP, sovrapposto a un tasto stop opaco.

**Fatto:**
- In stato di registrazione, il pulsante REC sfuma su un tasto stop quadrato opaco (`size-16 rounded-xl bg-black/35 border border-black/20 shadow-inner`) posizionato al centro del cerchio rosso.
- Sopra al tasto stop quadrato compaiono la micro-label `STOP` e il cronometro monospazio in tempo reale (`tabular-nums font-mono text-xs font-bold text-ink-primary`).
- Rimosso il cronometro dal dock sottostante, che ora rimane pulito e ospita solo badge di trascrizione ed eventuali errori a zero layout shift.

---

## 2026-10-19 — Pulsante import file a icona tonda a destra del REC

**Obiettivo:** Trasformare l'azione di caricamento file in un pulsante circolare allineato a destra del REC, che non sparisce quando compare il cronometro ma rimane visibile disabilitato.

**Fatto:**
- Creato un wrapper relativo attorno al REC: il pulsante REC rimane esattamente centrato sullo schermo, mentre il pulsante import (`Button variant="icon"` con icona `Upload`) è posizionato a destra (`absolute left-full top-1/2 -translate-y-1/2 ml-5`).
- Il pulsante import rimane visibile durante la registrazione e la trascrizione, rimanendo disabilitato (`disabled={session.locked || !session.canRecord}`, opacità al 40% e cursore standard).
- Il dock inferiore è stato compattato ad altezza fissa `h-8`: quando parte la registrazione, il cronometro entra in dissolvenza incrociata senza causare alcun layout shift.

---

## 2026-10-19 — Differenziazione affordance cliccabile vs badge passivo e bordo icon button

**Obiettivo:** Il pulsante impostazioni (`variant="icon"`) mancava del bordo coerente con il design system, e l'indicatore di stato rischiava di sembrare anch'esso un pulsante interattivo a causa della stessa elevazione.

**Fatto:**
- Aggiornata la variante `icon` in `Button.svelte`: aggiunto il bordo coordinato `border border-brand-cream/15` (con hover `hover:border-brand-cream/30`), feedback tattile `active:scale-[0.95]` e colore `text-ink-secondary`.
- `SttIndicator` in alto a sinistra (`fixed left-4 top-4 z-10`) ripulito da sfondo, bordo e badge: si presenta come semplice dot semantico affiancato alla label (`text-xs text-ink-muted`, altezza `h-8`), allineato al pulsante impostazioni.
- Limitata la visibilità alla sola home (`router.location === "/"` in `App.svelte`) per non interferire con il pulsante Indietro dell'header nelle pagine interne o duplicare lo stato in `/settings`.

---

## 2026-10-19 — Restyle bottone REC ed eliminazione del layout shift

**Obiettivo:** Il bottone REC e i suoi controlli annessi (import file, cronometro, stato di trascrizione ed errori) causavano un vistoso layout shift spingendo in alto e in basso la lista "Registrazioni". Inoltre il pulsante REC era privo di incorniciatura hardware e l'azione di caricamento file era un semplice testo sottolineato.

**Fatto:**
- Inserito un bezel esterno tattile concentrico (`p-2.5 rounded-full border border-brand-cream/10 bg-brand-darker/60 shadow-raised`) attorno al bottone REC/STOP da `112px` (`h-28 w-28`), che in registrazione riscalda bordo ed alone.
- Creato un dock di stato ad altezza rigidamente bloccata (`h-14 w-full max-w-md`) che mantiene costante l'altezza complessiva della hero section a 268px in qualsiasi stato: idle, registrazione, trascrizione, gate reason o errore.
- Introdotte transizioni fluide di cross-fade (`transition:fade={{ duration: 150 }}`) sia per le label interne del bottone (REC ↔ STOP) sia per gli stati del dock, impilati in CSS Grid (`col-start-1 row-start-1`) per evitare sovrapposizioni e scatti visivi.
- Sostituito il link ipertestuale "o carica un file" con una pillola dedicata rifinita con icona `Upload` di `@lucide/svelte`.
- Aggiunto `tabular-nums` e micro-dot rosso pulsante al cronometro di registrazione per azzerare lo jitter orizzontale durante l'avanzamento dei secondi.
- Sostituito il testo grezzo di trascrizione con un badge centrato con icona `LoaderCircle` rotante e cronometro monospazio.

**Decisioni:**
- Risolto il layout shift riservando uno slot verticale (`h-14`) con CSS Grid stack e `transition:fade`: durante l'animazione di uscita e ingresso i due stati occupano la stessa cella di griglia, garantendo una dissolvenza incrociata perfetta a zero layout shift.

---

## 2026-10-19 — GitHub Actions CI e Release automatica su merge a main

**Obiettivo:** Configurare CI per validare pull request e rami di feature/fix, e automatizzare build e release di Earwig ad ogni merge sul branch `main`.

**Fatto:**
- Creato `.github/workflows/ci.yml`: esegue linting (Biome) e typecheck (Svelte) per il frontend, e `cargo check` su runner `macos-14` (Apple Silicon M1) con cache Cargo (`Swatinem/rust-cache`).
- Creato `.github/workflows/release.yml`: si attiva ad ogni push/merge su `main` (o manualmente via `workflow_dispatch`).
- Caching intelligente dei binari server: `whisper-server` e `llama-server` in `src-tauri/binaries/` vengono cachati con chiave hash sugli script `build-whisper-server.sh` e `build-llama-server.sh` (evitando minuti superflui di compilazione CMake a ogni merge).
- Packaging macOS: compilazione bundle Tauri (`--target aarch64-apple-darwin`), estrazione del `.dmg`, generazione dell'archivio `.zip` del bundle `.app` tramite `ditto` (preservando attributi e symlink macOS), e calcolo checksum SHA-256 (`checksums-sha256.txt`).
- Strategia di tag e rilascio robusta: se `v<version>` (da `package.json`) non esiste ancora sul remote, usa `v<version>`; in caso di merge successivi con la stessa versione prima di un bump esplicito, genera un tag univoco `v<version>-b<run_number>` senza conflitti.
- Pubblicazione automatica con note di rilascio tramite `softprops/action-gh-release@v2`.
- Aggiornata la documentazione in `docs/architecture.md`.

**Decisioni:**
- Runner `macos-14` (Apple Silicon M1): nativo per la compilazione Metal e per le librerie Swift/ScreenCaptureKit di macOS.
- Target release principale `aarch64-apple-darwin`: garantisce massima stabilità, velocità e compatibilità nativa su tutti i Mac Apple Silicon.

---

## 2026-10-19 — GitHub Actions CI e Release automatica su merge a main

**Obiettivo:** Configurare CI per validare pull request e rami di feature/fix, e automatizzare build e release di Earwig ad ogni merge sul branch `main`.

**Fatto:**
- Creato `.github/workflows/ci.yml`: esegue linting (Biome) e typecheck (Svelte) per il frontend, e `cargo check` su runner `macos-14` (Apple Silicon M1) con cache Cargo (`Swatinem/rust-cache`).
- Creato `.github/workflows/release.yml`: si attiva ad ogni push/merge su `main` (o manualmente via `workflow_dispatch`).
- Caching intelligente dei binari server: `whisper-server` e `llama-server` in `src-tauri/binaries/` vengono cachati con chiave hash sugli script `build-whisper-server.sh` e `build-llama-server.sh` (evitando minuti superflui di compilazione CMake a ogni merge).
- Packaging macOS: compilazione bundle Tauri (`--target aarch64-apple-darwin`), estrazione del `.dmg`, generazione dell'archivio `.zip` del bundle `.app` tramite `ditto` (preservando attributi e symlink macOS), e calcolo checksum SHA-256 (`checksums-sha256.txt`).
- Strategia di tag e rilascio robusta: se `v<version>` (da `package.json`) non esiste ancora sul remote, usa `v<version>`; in caso di merge successivi con la stessa versione prima di un bump esplicito, genera un tag univoco `v<version>-b<run_number>` senza conflitti.
- Pubblicazione automatica con note di rilascio tramite `softprops/action-gh-release@v2`.
- Aggiornata la documentazione in `docs/architecture.md`.

**Decisioni:**
- Runner `macos-14` (Apple Silicon M1): nativo per la compilazione Metal e per le librerie Swift/ScreenCaptureKit di macOS.
- Target release principale `aarch64-apple-darwin`: garantisce massima stabilità, velocità e compatibilità nativa su tutti i Mac Apple Silicon.

---

## 2026-10-19 — Ombre a token

**Obiettivo:** 4 `shadow-[...]` inline con rgba hardcoded, e i keyframes `pulse-rec` che duplicavano in rgba il colore di `--color-rec-strong`.

**Decisioni:**
- Token `shadow-raised` (elementi flottanti), `shadow-pop` (logo onboarding), `shadow-rec`/`shadow-rec-active` (alone del REC) in `@theme`
- `shadow-rec-active` usa `color-mix` su `var(--color-rec-strong)`: cambiare il rosso del bottone aggiorna anche il suo alone
- I keyframes ora referenziano il token invece di ripetere il color-mix

---

## 2026-10-19 — Token di inchiostro per la gerarchia del testo

**Obiettivo:** il testo usava 13 alpha diverse di `brand-cream` (`/5` → `/90`), lo stesso male dei font size letterali: a occhio `/45` e `/50` coincidono.

**Decisioni:**
- Quattro token `ink-*` in `@theme` (primary = cream pieno, secondary 70%, muted 50%, faint 40%), definiti con `color-mix` così seguono il brand senza duplicare hex
- Gli alpha bassi di `brand-cream` (5-20) restano bordi e superfici, mai testo: la regola è scritta nel commento dei token
- Badge a 3 stati in `types.ts` compreso nella mappatura

---

## 2026-10-19 — Scala tipografica centralizzata

**Obiettivo:** il markup spargeva 13 font size diversi, 9 dei quali valori letterali (`0.62rem` → `0.85rem`) visivamente indistinguibili: la scala tipografica entra nei token `@theme` accanto a colori e raggi.

**Decisioni:**
- Quattro gradini per la UI: `2xs` (0.7rem, nuovo token: micro-label, badge, duration mono), `xs` (0.75rem: testo secondario, stati, hint), `sm` (0.875rem: titoli di riga e bottoni), sopra restano `base` e `2xl` riservati a REC e al timer
- I bottoni `Button` erano incoerenti fra varianti (solid/danger 0.8rem, primary sm): ora tutti `sm`
- Spacing intoccato: è già sullo scale standard di Tailwind, il "sistema" per i margini esiste ed è quello

---

## 2026-10-19 — Home in verticale e gerarchia dei testi

**Obiettivo:** la home è un pannello da registrazione, non un dashboard: finestra di default verticale (420×640, era 860×640 orizzontale) e gerarchia tipografica che segua l'uso reale.

**Decisioni:**
- "o carica un file" scende a azione terziaria (45% cream, underline solo in hover): prima competeva con il label REGISTRAZIONI e con le righe, ma l'azione secondaria vera è la lista, l'import è l'eccezione
- Ordine di evidenza voluto: REC → righe della lista (contenuto, 100%) → stati transitori (trascrizione, gate, 50%) → import (45%) → label sezione (40%)
- Stato vuoto della lista come card tratteggiata con icona: per un utente nuovo è il primo volto della home, una riga-segnaposto diceva "nessun dato" invece di dire cosa fare

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
