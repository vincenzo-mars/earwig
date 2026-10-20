<script lang="ts">
import { PanelRightClose, Play, Settings, Upload } from "@lucide/svelte";
import { fade } from "svelte/transition";
import { push } from "svelte-spa-router";
import Button from "./Button.svelte";
import RecordingsList from "./RecordingsList.svelte";
import { recordings } from "./stores/recordings.svelte";
import { session } from "./stores/session.svelte";
import { formatDuration } from "./types";

// La lista vive qui, quindi va riallineata al disco appena una trascrizione
// finisce. Il refresh non può stare dentro `session`: la direzione delle
// dipendenze fra store è `recordings → session`, non il contrario.
async function handleImport() {
  if (await session.importFile()) await recordings.load(true);
}

async function handleRecord() {
  if (session.isRecording) {
    await session.stopRecording();
    await recordings.load(true);
  } else {
    await session.startRecording();
  }
}

// Quando registrare non è disponibile (STT non pronto, trascrizione in corso,
// avvio o stop in volo), il pulsante viene disattivato in modo opaco.
const isRecordDisabled = $derived(
  session.isRecording
    ? session.isStopping
    : !session.canRecord ||
        session.isTranscribing ||
        session.isStarting ||
        session.isStopping,
);

const recordButtonTitle = $derived.by(() => {
  if (session.isStopping) return "Salvataggio audio in corso...";
  if (session.isStarting) return "Avvio registrazione in corso...";
  if (session.isRecording) return "Ferma registrazione";
  if (!session.canRecord) return session.recordGateReason ?? undefined;
  if (session.isTranscribing) return "Trascrizione in corso";
  return "Avvia registrazione";
});

function isAlreadyRecording(err: string) {
  return (
    err.includes("Already recording") ||
    err.includes("Registrazione già in corso")
  );
}
</script>

<!-- L'accesso alle impostazioni vive qui e non nel guscio: le pagine interne
     hanno già il loro header, e un bottone flottante in alto a destra si
     sovrapporrebbe alle loro azioni. -->
<div class="fixed right-6 top-6 z-10 flex items-center gap-2">
  <Button
    variant="icon"
    onclick={() => session.setCompactMode(true)}
    title="Modalità ridotta (bordo schermo)"
    aria-label="Attiva modalità ridotta"
  >
    <PanelRightClose size={16} />
  </Button>

  <Button
    variant="icon"
    class="relative"
    onclick={() => push("/settings")}
    title="Impostazioni"
    aria-label="Apri le impostazioni"
  >
    <Settings size={16} />
  </Button>
</div>

<main class="flex flex-1 flex-col overflow-hidden">
  <!-- Due zone: l'hero REC non scrolla mai, la lista sotto prende quello che
       resta. `pb-18` lascia respiro all'ultima card a fondo pagina. -->
  <div class="flex shrink-0 flex-col items-center gap-4 px-6 pt-10 pb-6 text-center">
    <div class="relative flex items-center justify-center">
      <!-- Bezel esterno che incornicia il pulsante REC/STOP -->
      <div
        class={`relative flex items-center justify-center rounded-full border p-2.5 transition-all duration-200 ${
          session.isRecording
            ? "border-rec-strong/30 bg-rec-strong/5"
            : isRecordDisabled
              ? "border-brand-cream/5 bg-brand-darker/40 shadow-none opacity-40"
              : "border-brand-cream/10 bg-brand-darker/60 shadow-raised"
        }`}
      >
        <button
          class={`flex h-32 w-32 items-center justify-center rounded-full border-none text-lg font-bold text-ink-primary shadow-raised transition-all duration-200 enabled:cursor-pointer enabled:hover:scale-[1.02] enabled:active:scale-[0.96] disabled:cursor-not-allowed disabled:shadow-none ${
            session.isRecording
              ? "animate-pulse-rec bg-rec-strong"
              : "bg-rec enabled:hover:bg-rec-strong"
          }`}
          onclick={handleRecord}
          disabled={isRecordDisabled}
          title={recordButtonTitle}
          aria-label={session.isStopping ? "Salvataggio registrazione" : session.isRecording ? "Ferma registrazione" : "Avvia registrazione"}
        >
          <div class="grid place-items-center">
            {#if session.isRecording}
              <div
                transition:fade={{ duration: 150 }}
                class="col-start-1 row-start-1 relative flex items-center justify-center"
              >
                <!-- Quadrato dello STOP in background: centrato sull'asse z sotto al timer,
                     opacità discreta (10%) e dimensione aumentata (size-16, 64px) -->
                <div
                  class="absolute inset-0 m-auto size-16 rounded-lg bg-black/10"
                  aria-hidden="true"
                ></div>

                <!-- Timer durata ingrandito, centrato sia verticalmente che orizzontalmente -->
                <span
                  class="relative z-10 select-none font-mono text-lg font-bold tabular-nums tracking-tight text-ink-primary leading-none"
                >
                  {formatDuration(session.durationMs)}
                </span>
              </div>
            {:else}
              <div
                transition:fade={{ duration: 150 }}
                class="col-start-1 row-start-1 flex items-center justify-center"
              >
                <!-- Icona PLAY in stato idle: centrata geometricamente ed otticamente nel cerchio REC
                     (compensata a -translate-x-[1.5px] per bilanciare l'asimmetria del path Lucide rispetto alla circonferenza),
                     size 88px, ingrandita verticalmente (scale-y-[1.08]) e opacità 10% -->
                <Play
                  size={88}
                  fill="currentColor"
                  strokeWidth={0}
                  class="-translate-x-[1.5px] scale-y-[1.08] text-black/10 pointer-events-none"
                />
              </div>
            {/if}
          </div>
        </button>
      </div>

      <!-- Bottone rounded "Carica file audio" allineato centralmente a destra del REC -->
      <div class="absolute left-full top-1/2 -translate-y-1/2 ml-5">
        <Button
          variant="icon"
          onclick={handleImport}
          disabled={session.locked || !session.canRecord}
          title={!session.canRecord
            ? session.recordGateReason
            : session.isStopping
              ? "Salvataggio audio in corso"
              : session.isRecording
                ? "Registrazione in corso"
                : session.isTranscribing
                  ? "Trascrizione in corso"
                  : "Carica file audio"}
          aria-label="Carica file audio"
        >
          <Upload size={16} />
        </Button>
      </div>
    </div>

    <!-- Dock di stato ad altezza bloccata (h-8): ospita avvisi/errori azzerando il layout shift. -->
    <div class="grid h-8 w-full max-w-md place-items-center text-center">
      {#if session.error && !isAlreadyRecording(session.error)}
        <div
          transition:fade={{ duration: 150 }}
          class="col-start-1 row-start-1 flex items-center justify-center"
        >
          <p class="m-0 max-w-95 text-xs text-danger line-clamp-2 leading-relaxed" title={session.error}>
            {session.error}
          </p>
        </div>
      {:else if !session.canRecord}
        <div
          transition:fade={{ duration: 150 }}
          class="col-start-1 row-start-1 flex items-center justify-center"
        >
          <p class="m-0 max-w-95 text-xs text-ink-muted leading-relaxed">
            {session.recordGateReason}
          </p>
        </div>
      {/if}
    </div>
  </div>

  <div class="flex flex-1 flex-col gap-3 overflow-y-auto px-6 pb-18">
    <div class="mx-auto flex w-full max-w-200 flex-col gap-3">
      <h2
        class="m-0 text-2xs font-semibold uppercase tracking-[0.12em] text-ink-faint"
      >
        Registrazioni
      </h2>
      <RecordingsList />
    </div>
  </div>
</main>
