<script lang="ts">
import { Settings } from "@lucide/svelte";
import { push } from "svelte-spa-router";
import Button from "./Button.svelte";
import RecordingsList from "./RecordingsList.svelte";
import { recordings } from "./stores/recordings.svelte";
import { session } from "./stores/session.svelte";
import { formatDuration, formatElapsed } from "./types";

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
</script>

<!-- L'accesso alle impostazioni vive qui e non nel guscio: le pagine interne
     hanno già il loro header, e un bottone flottante in alto a destra si
     sovrapporrebbe alle loro azioni. -->
<Button
  variant="icon"
  class="fixed right-4 top-4 z-10"
  onclick={() => push("/settings")}
  title="Impostazioni"
  aria-label="Apri le impostazioni"
>
  <Settings size={16} />
</Button>

<main class="flex flex-1 flex-col overflow-hidden">
  <!-- Due zone: l'hero REC non scrolla mai, la lista sotto prende quello che
       resta. `pb-18` tiene l'ultima riga sopra l'indicatore di stato del
       server, che galleggia in basso a sinistra. -->
  <div class="flex shrink-0 flex-col items-center gap-4 px-6 pt-10 pb-6 text-center">
    <button
      class={`h-30 w-30 cursor-pointer rounded-full border-none text-base font-bold text-ink-primary transition-[background,box-shadow,transform] duration-200 active:scale-[0.96] disabled:cursor-default disabled:opacity-50 ${
        session.isRecording
          ? "animate-pulse-rec bg-rec-strong shadow-rec-active"
          : "bg-rec shadow-rec hover:bg-rec-strong"
      }`}
      onclick={handleRecord}
      disabled={!session.isRecording && !session.canRecord}
      title={!session.isRecording && !session.canRecord ? session.recordGateReason : undefined}
      aria-label={session.isRecording ? "Stop recording" : "Start recording"}
    >
      {session.isRecording ? "■ STOP" : "⬤ REC"}
    </button>

    {#if !session.isRecording}
      <button
        class="cursor-pointer border-none bg-transparent text-xs text-ink-faint transition-colors hover:text-ink-secondary hover:underline underline-offset-4 disabled:cursor-default disabled:opacity-50"
        onclick={handleImport}
        disabled={session.isTranscribing || !session.canRecord}
        title={!session.canRecord ? session.recordGateReason : undefined}
      >
        o carica un file
      </button>
    {/if}

    {#if !session.isRecording && !session.canRecord}
      <p class="m-0 max-w-95 text-xs text-ink-muted">{session.recordGateReason}</p>
    {/if}
    {#if session.isRecording}
      <p class="m-0 font-mono text-2xl font-semibold text-rec-strong">
        {formatDuration(session.durationMs)}
      </p>
    {/if}
    {#if session.isTranscribing}
      <p class="m-0 animate-pulse text-xs text-ink-muted">
        Trascrizione in corso... <span class="font-mono">{formatElapsed(session.transcribeMs)}</span>
      </p>
    {/if}
    {#if session.error}
      <p class="m-0 max-w-95 text-xs text-danger">{session.error}</p>
    {/if}
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
