# TODO: Earwig

Coda di lavoro ordinata per priorità. Ogni voce cita dove vive il codice oggi.
Muovere la voce in `Fatto` quando chiude, non cancellarla.

## 1. Switch modalità ridotta ↔ full: crash e menu bar rotta

- [ ] Switch ripetuti tra linguetta compatta e finestra full fanno crashare l'app. Stesso code path dei due sintomi.
- [ ] Dopo il ritorno in full la barra del menu di macOS non ha il colore (sembra una finestra senza title bar corretta), quindi lo stato full è visivamente rotto.

Punto di partenza: `src-tauri/src/commands/window.rs:62` (`set_compact_mode`) che toglie/rimette `decorations`, `shadow`, `always_on_top`, `resizable` e min size; listener finestra in `src-tauri/src/lib.rs:95`; layer DOM della linguetta `src/lib/CompactStrip.svelte`.

## 2. Mix microfono vs system audio: il mic è troppo basso

- [ ] Aumentare il volume del microfono rispetto alla voce del sistema nella registrazione. Oggi il mic arriva più basso del system audio e questo peggiora ricezione e trascrizione.

Punto di partenza: mix e normalizzazione in `src-tauri/src/recorder/audio.rs` (gain adattivo su `target_rms`, clamp peak), cattura mic in `src-tauri/src/recorder/mic.rs`, AEC in `src-tauri/src/recorder/aec.rs:34` (li togli il system audio rientrato nel mic: alzare il gain qui ha conseguenze sull'echo cancellation).

## 3. Qualità della voce in trascrizione

- [ ] Capire se e come si può migliorare la qualità percepita del vocale prima della trascrizione.

Dipende in parte dalla voce 2. Punti da valutare: pre-processing sul canale mic (gate, EQ, loudness), VAD, cosa passa a parakeet (`src-tauri/src/commands/stt.rs`).

## 4. UI lista registrazioni: snellire l'item

- [ ] Rimuovere il tag `Trascritto` / non trascritto dall'item della lista.
- [ ] Rimuovere i secondi di trascrizione dall'item.
- [ ] Durante la trascrizione in corso mostrare solo un'UI di attesa.
- [ ] Appena la trascrizione è finita non mostrare più alcun indicatore.

Punto di partenza: `src/lib/RecordingsList.svelte:81` (blocco "Durata STT / Badge Trascritto / Chevron") e label in `src/lib/types.ts:160`.

## 5. Animazioni hover della linguetta compatta: scatti

- [ ] Le transizioni di hover della barra minima hanno scatti visibili all'ingresso e all'uscita.

**Causa (due animazioni parallele che non sono mai sincronizzate):** il movimento è animato due volte, in modo indipendente.

1. **CSS nel webview**: `CompactStrip.svelte:125` fa `transition-all duration-200` sulla larghezza `w-[12px] → w-[82px]` (`:127-128`), 200ms con easing.
2. **Risize nativo della finestra**: `handleMouseEnter`/`handleMouseLeave` (`:28`, `:41`) inviano `set_compact_hover` e il backend fa `set_size` **istantaneo e asincrono** (`src-tauri/src/commands/window.rs:160`, `:163`), quindi arriva al fotogramma successivo all'IPC round-trip, con un ritardo variabile rispetto alla partenza della transizione CSS. Il contenuto finisce per essere ritagliato da `overflow-hidden` dentro una finestra già ridotta, mentre la barra CSS sta ancora animando: è lo scatto.

**Concause:**

- **Larghezze diverse tra i due mondi**: CSS 12px/82px, Rust `COMPACT_WIDTH_IDLE`/`COMPACT_WIDTH_EXPANDED` = 18/86 (`window.rs:36-37`). Restano 6px vuoti a riposo e un salto di 4px a fine espansione.
- **`transition-all` + `{#if}`**: i controlli (`:136`, `:150`) si montano istantaneamente al primo fotogramma mentre la larghezza è ancora ~12px, quindi compaiono *prima* che ci sia spazio, poi vengono schiacciati.
- **Ordine invertito**: in hover `set_position` poi `set_size` (`window.rs:159-160`), in uscita `set_size` poi `set_position` (`window.rs:163-164`): asimmetria ingresso/uscita.
- **Invocazioni duplicate**: oltre a `onmouseenter` sul wrapper (`:114`), l'elemento idle ha `onclick={handleMouseEnter}` (`:141`) e l'effetto di sync registrazione (`:55-63`) rilancia `set_compact_hover`: set_size ripetuti a metà transizione.
- **Riecho di eventi**: il backend riemette `compact-stage` (`window.rs:105`, `:161`) che riscrive `stage` sul frontend (`:15-26`); se i due lati divergono lo stato rimbalza.

Scelta da fare: animare il resize nativo lato Rust (CA transaction/window animation) e tenere il CSS solo per il contenuto, oppure disattivare il CSS e affidarsi al solo resize nativo. Finché non si sceglie un solo padrone dell'animazione, lo scatto resta.

## 6. Posizione verticale della modalità ridotta persistente per utente

- [ ] All'avvio la Y della linguetta compatta deve essere quella salvata dell'utente, non recentrata.

Oggi `compact_y` vive solo in memoria (`src-tauri/src/commands/window.rs:14`, `WindowModeState`): a ogni riavvio è `None` e la finestra parte dal centro (`get_screen_bounds`, `window.rs:57`). Se l'utente la sposta verso il basso con `set_compact_y` (`window.rs:172`), la posizione va persistita per utente in `settings.json` (`src-tauri/src/commands/mod.rs:61`) e riletta in setup.
