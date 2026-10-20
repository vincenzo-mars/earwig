<script lang="ts">
import {
  AudioLines,
  ChevronRight,
  LoaderCircle,
  RotateCcw,
} from "@lucide/svelte";
import { push } from "svelte-spa-router";
import Button from "./Button.svelte";
import { recordings } from "./stores/recordings.svelte";
import { session } from "./stores/session.svelte";
import {
  formatElapsed,
  formatRecordingDate,
  formatSeconds,
  type RecordingEntry,
  transcriptStatus,
} from "./types";

$effect(() => {
  recordings.load();
});

// Solo la registrazione blocca l'apertura del dettaglio: navigare via dalla
// home mentre si registra nasconderebbe stop e cronometro, che vivono lì.
const rowsLocked = $derived(session.isRecording);

async function handleRetry(entry: RecordingEntry) {
  if (session.locked) return;
  await recordings.retryTranscription(entry.folder_path);
}
</script>

<div class="flex flex-col gap-2">
  {#if recordings.loading && recordings.all.length === 0}
    <p class="m-0 py-2 text-xs text-ink-muted">Caricamento...</p>
  {:else if recordings.all.length === 0}
    <div
      class="flex flex-col items-center gap-2.5 rounded-xl border border-dashed border-brand-cream/10 bg-brand-darker/50 px-6 py-9 text-center"
    >
      <span
        class="flex size-9 items-center justify-center rounded-full bg-brand-cream/5 text-ink-faint"
      >
        <AudioLines size={18} />
      </span>
      <p class="m-0 text-xs text-ink-muted">Nessuna registrazione, per ora.</p>
    </div>
  {:else}
    {#each recordings.all as entry (entry.folder_path)}
      {@const isProcessing = session.processingItem?.name === entry.name}
      {@const processingPhase = isProcessing
        ? session.processingItem?.phase
        : null}
      {@const status = transcriptStatus(entry)}

      {#if status === "transcribed"}
        <button
          type="button"
          class="group flex w-full items-center justify-between gap-3 rounded-lg border border-brand-cream/8 bg-brand-dark/40 px-3.5 py-2.5 text-left transition-all duration-150 enabled:cursor-pointer enabled:hover:border-brand-cream/25 enabled:hover:bg-brand-cream/[0.04] enabled:active:scale-[0.995] disabled:cursor-not-allowed disabled:opacity-60"
          onclick={() => push(`/detail/${encodeURIComponent(entry.name)}`)}
          disabled={rowsLocked || isProcessing}
          title={rowsLocked
            ? "Attendi la fine della registrazione in corso"
            : undefined}
        >
          <!-- Sinistra: Titolo formattato + Durata audio -->
          <div class="flex min-w-0 items-baseline gap-2 truncate">
            <span
              class="truncate text-xs font-semibold tracking-wide text-ink-primary transition-colors group-hover:text-brand-cream"
              title={entry.name}
            >
              {formatRecordingDate(entry.name)}
            </span>
            {#if entry.transcript?.duration != null}
              <span class="shrink-0 font-mono text-2xs text-ink-muted">
                · {formatSeconds(entry.transcript.duration)}
              </span>
            {/if}
          </div>

          <!-- Destra: Durata STT / Badge Trascritto / Chevron -->
          <div class="flex shrink-0 items-center gap-2">
            {#if entry.transcript?.transcription_ms != null}
              <span
                class="font-mono text-2xs text-ink-faint"
                title="Tempo di trascrizione"
              >
                {formatElapsed(entry.transcript.transcription_ms)}
              </span>
            {/if}

            <span
              class="inline-flex items-center gap-1.5 rounded-full border border-ok/25 bg-ok/10 px-2 py-0.5 text-2xs font-medium text-ok"
            >
              <span class="size-1.5 rounded-full bg-ok"></span>
              <span>Trascritto</span>
            </span>

            <ChevronRight
              size={14}
              class="text-ink-faint transition-all duration-150 group-hover:translate-x-0.5 group-hover:text-ink-primary"
            />
          </div>
        </button>
      {:else}
        <div
          class={`group flex w-full items-center justify-between gap-3 rounded-lg border border-brand-cream/8 bg-brand-dark/40 px-3.5 py-2.5 transition-all duration-150 ${
            isProcessing ? "opacity-90" : "opacity-75"
          }`}
        >
          <!-- Sinistra: Titolo formattato + Durata audio -->
          <div class="flex min-w-0 items-baseline gap-2 truncate">
            <span
              class="truncate text-xs font-semibold tracking-wide text-ink-primary"
              title={entry.name}
            >
              {formatRecordingDate(entry.name)}
            </span>
            {#if entry.transcript?.duration != null}
              <span class="shrink-0 font-mono text-2xs text-ink-muted">
                · {formatSeconds(entry.transcript.duration)}
              </span>
            {/if}
          </div>

          <!-- Destra: Stato / Processing / Rilancia -->
          <div class="flex shrink-0 items-center gap-2">
            {#if isProcessing}
              {#if processingPhase === "noise_cancellation"}
                <span
                  class="inline-flex items-center gap-1.5 rounded-full border border-warn/30 bg-warn/10 px-2 py-0.5 text-2xs font-medium text-warn"
                >
                  <LoaderCircle size={10} class="animate-spin text-warn" />
                  <span>cancellazione rumore...</span>
                </span>
              {:else if processingPhase === "transcribing"}
                <span class="font-mono text-2xs text-ink-faint">
                  {formatElapsed(session.transcribeMs)}
                </span>
                <span
                  class="inline-flex items-center gap-1.5 rounded-full border border-brand-cream/20 bg-brand-cream/5 px-2 py-0.5 text-2xs font-medium text-ink-secondary"
                >
                  <LoaderCircle size={10} class="animate-spin text-ink-faint" />
                  <span>trascrizione...</span>
                </span>
              {/if}
            {:else}
              {#if status === "failed"}
                <span
                  class="inline-flex items-center gap-1.5 rounded-full border border-danger/30 bg-danger/10 px-2 py-0.5 text-2xs font-medium text-danger"
                >
                  <span class="size-1.5 rounded-full bg-danger"></span>
                  <span>Fallito</span>
                </span>
              {:else}
                <span
                  class="inline-flex items-center rounded-full border border-brand-cream/20 bg-transparent px-2 py-0.5 text-2xs font-medium text-ink-faint"
                >
                  In attesa
                </span>
              {/if}

              <Button
                variant="icon"
                class="size-7 shrink-0 text-ink-secondary hover:text-ink-primary"
                onclick={() => handleRetry(entry)}
                disabled={session.locked}
                title="Rilancia trascrizione"
                aria-label="Rilancia trascrizione"
              >
                <RotateCcw size={13} />
              </Button>
            {/if}
          </div>
        </div>
      {/if}
    {/each}
  {/if}
</div>
