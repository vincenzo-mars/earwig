<script lang="ts">
import { AudioLines } from "@lucide/svelte";
import { push } from "svelte-spa-router";
import Button from "./Button.svelte";
import { recordings } from "./stores/recordings.svelte";
import { session } from "./stores/session.svelte";
import {
  formatElapsed,
  type RecordingEntry,
  transcriptStatus,
  transcriptStatusInfo,
} from "./types";

$effect(() => {
  recordings.load();
});

// Solo la registrazione blocca l'apertura del dettaglio: navigare via dalla
// home mentre si registra nasconderebbe stop e cronometro, che vivono lì.
// Durante la sola trascrizione aprire una registrazione vecchia non
// interferisce, quindi le righe restano cliccabili.
const rowsLocked = $derived(session.isRecording);

async function handleRetry(entry: RecordingEntry) {
  if (session.locked) return;
  await recordings.retryTranscription(entry.folder_path);
}
</script>

<div class="flex flex-col gap-3">
  {#if recordings.loading && recordings.all.length === 0}
    <p class="m-0 text-sm text-ink-muted">Caricamento...</p>
  {:else if recordings.all.length === 0}
    <div
      class="flex flex-col items-center gap-3 rounded-xl border border-dashed border-brand-cream/10 bg-brand-darker/50 px-6 py-10 text-center"
    >
      <span
        class="flex size-10 items-center justify-center rounded-full bg-brand-cream/5 text-ink-faint"
      >
        <AudioLines size={20} />
      </span>
      <p class="m-0 text-sm font-semibold text-ink-muted">Nessuna registrazione, per ora.</p>
      <p class="m-0 text-xs text-ink-faint">
        Premi REC per catturare microfono e audio di sistema.
      </p>
    </div>
  {:else}
    {#each recordings.all as entry (entry.folder_path)}
      {@const status = transcriptStatusInfo(entry)}
      <div
        class="flex items-center justify-between gap-3 rounded-xl border border-brand-cream/10 bg-brand-darker px-5 py-4 transition-colors hover:border-brand-cream/20"
      >
        <button
          class="flex flex-1 cursor-pointer items-center justify-between gap-3 border-none bg-transparent p-0 text-left disabled:cursor-default"
          onclick={() => push(`/detail/${encodeURIComponent(entry.name)}`)}
          disabled={rowsLocked}
          title={rowsLocked ? "Attendi la fine della registrazione in corso" : undefined}
        >
          <span class="text-sm font-semibold text-ink-primary">{entry.name}</span>
          <span class="flex items-center gap-2">
            {#if entry.transcript?.transcription_ms != null}
              <span class="whitespace-nowrap font-mono text-2xs text-ink-faint">
                {formatElapsed(entry.transcript.transcription_ms)}
              </span>
            {/if}
            <span
              class={`whitespace-nowrap rounded-full px-2 py-0.5 text-2xs font-semibold ${status.className}`}
            >
              {status.label}
            </span>
          </span>
        </button>
        <!-- Il rilancio compare solo dove serve davvero: su una registrazione
             già trascritta resta comunque disponibile dal dettaglio. -->
        {#if transcriptStatus(entry) !== "transcribed"}
          <Button
            onclick={() => handleRetry(entry)}
            disabled={session.locked}
            title={session.locked ? "Attendi la fine della registrazione o della trascrizione in corso" : undefined}
          >
            Rilancia
          </Button>
        {/if}
      </div>
    {/each}
  {/if}
</div>
