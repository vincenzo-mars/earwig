<script lang="ts">
import {
  AlertCircle,
  Check,
  Clock,
  Copy,
  Folder,
  LoaderCircle,
  RotateCcw,
} from "@lucide/svelte";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { replace } from "svelte-spa-router";
import Button from "./Button.svelte";
import Modal from "./Modal.svelte";
import { recordings } from "./stores/recordings.svelte";
import { session } from "./stores/session.svelte";
import TranscriptView from "./TranscriptView.svelte";
import {
  formatRecordingDate,
  formatSeconds,
  groupSegments,
  speakerInfo,
} from "./types";

let {
  id: propId,
  params,
  onclose,
}: {
  id?: string;
  params?: { id: string };
  onclose?: () => void;
} = $props();

let id = $derived(propId ?? (params?.id ? decodeURIComponent(params.id) : ""));
let entry = $derived(recordings.byName(id));
let resolving = $state(true);

const title = $derived(
  entry ? formatRecordingDate(entry.name) : "Dettaglio registrazione",
);

function close() {
  if (onclose) {
    onclose();
  } else {
    replace("/");
  }
}

let isProcessing = $derived(session.processingItem?.name === entry?.name);

let copiedAll = $state(false);
let copiedAllTimeout: ReturnType<typeof setTimeout> | null = null;

// La registrazione è accessibile solo se già trascritta:
// un id senza riscontro o non ancora trascritto torna alla home con `close()`.
$effect(() => {
  const wanted = id;
  if (!wanted) {
    close();
    return;
  }
  resolving = true;
  recordings.load().then(() => {
    if (wanted !== id) return;
    resolving = false;
    const found = recordings.byName(wanted);
    if (!found?.transcript) close();
  });
});

async function handleRetry() {
  if (session.locked || isProcessing || !entry) return;
  await recordings.retryTranscription(entry.folder_path);
}

async function handleCopyAll() {
  if (!entry?.transcript) return;
  const currentGroups = groupSegments(entry.transcript.segments);
  let text = "";
  if (currentGroups.length > 0) {
    text = currentGroups
      .map((g) => {
        const { label } = speakerInfo(g.speaker);
        return `${label}:\n${g.text}`;
      })
      .join("\n\n");
  } else {
    text = entry.transcript.text;
  }

  try {
    await navigator.clipboard.writeText(text);
    copiedAll = true;
    if (copiedAllTimeout) clearTimeout(copiedAllTimeout);
    copiedAllTimeout = setTimeout(() => {
      copiedAll = false;
    }, 2000);
  } catch (e) {
    console.error("Failed to copy transcript:", e);
  }
}
</script>

<Modal {title} onclose={close} maxWidth="max-w-5xl" class="h-[85vh]">
  {#snippet headerActions()}
    {#if entry?.transcript}
      <!-- Durata con icona tempo (senza badge, solo icona e testo) -->
      <div
        class="flex items-center gap-1.5 font-mono text-xs text-ink-secondary tabular-nums select-none mr-1"
        title="Durata registrazione"
      >
        <Clock size={13} class="text-ink-muted" />
        <span>{formatSeconds(entry.transcript.duration)}</span>
      </div>

      <!-- Copia tutta la conversazione -->
      <Button
        variant="icon"
        onclick={handleCopyAll}
        title={copiedAll ? "Copiato!" : "Copia tutta la conversazione"}
        aria-label="Copia tutta la conversazione"
      >
        {#if copiedAll}
          <Check size={15} class="text-ok" />
        {:else}
          <Copy size={15} />
        {/if}
      </Button>

      <!-- Percorso (apri nel Finder con icona cartella) -->
      <Button
        variant="icon"
        onclick={() => entry && revealItemInDir(entry.folder_path)}
        title="Apri cartella nel Finder"
        aria-label="Apri cartella nel Finder"
      >
        <Folder size={15} />
      </Button>

      <!-- Rilancia trascrizione -->
      <Button
        variant="icon"
        onclick={handleRetry}
        disabled={session.locked || isProcessing}
        title={session.locked
          ? "Attendi la fine della registrazione in corso"
          : isProcessing
            ? "Trascrizione in corso..."
            : "Rilancia trascrizione"}
        aria-label="Rilancia trascrizione"
      >
        <RotateCcw size={15} class={isProcessing ? "animate-spin" : ""} />
      </Button>
    {/if}
  {/snippet}

  <div class="flex flex-col flex-1 min-h-0 overflow-hidden text-ink-primary">
    {#if !entry || !entry.transcript}
      <div class="flex flex-1 flex-col items-center justify-center py-16 gap-3 text-center">
        {#if resolving}
          <LoaderCircle size={22} class="animate-spin text-ink-faint" />
          <p class="m-0 text-xs text-ink-muted">Caricamento registrazione...</p>
        {:else}
          <AlertCircle size={22} class="text-ink-faint" />
          <p class="m-0 text-xs text-ink-muted">Registrazione non disponibile.</p>
        {/if}
      </div>
    {:else}
      <!-- Trascrizione a tutta larghezza stile conversazione -->
      <div class="relative flex-1 overflow-y-auto px-4 py-4 md:px-8 md:py-6">
        <TranscriptView transcript={entry.transcript} />
      </div>
    {/if}
  </div>
</Modal>
