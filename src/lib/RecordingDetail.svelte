<script lang="ts">
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { push, replace } from "svelte-spa-router";
import Button from "./Button.svelte";
import PageHeader from "./PageHeader.svelte";
import { recordings } from "./stores/recordings.svelte";
import { servers } from "./stores/servers.svelte";
import { session } from "./stores/session.svelte";
import TranscriptNotes from "./TranscriptNotes.svelte";
import TranscriptView from "./TranscriptView.svelte";

let { params }: { params: { id: string } } = $props();

let id = $derived(decodeURIComponent(params.id));
let entry = $derived(recordings.byName(id));
let resolving = $state(true);

// La store può essere vuota se si arriva qui direttamente dall'URL (reload
// con l'hash già su /detail, o HMR in dev): finché `load()` non ha risposto
// non si può concludere che la registrazione non esista. Dopo, un id senza
// riscontro è una registrazione cancellata e si torna alla home (dove sta la
// lista) con `replace`, per non lasciare in history una route morta.
$effect(() => {
  const wanted = id;
  resolving = true;
  recordings.load().then(() => {
    if (wanted !== id) return;
    resolving = false;
    if (!recordings.byName(wanted)) replace("/");
  });
});

async function handleRetry() {
  if (session.locked || !entry) return;
  await recordings.retryTranscription(entry.folder_path);
}
</script>

{#snippet actions()}
  <Button
    onclick={handleRetry}
    disabled={session.locked}
    title={session.locked ? "Attendi la fine della registrazione o della trascrizione in corso" : undefined}
  >
    Rilancia trascrizione
  </Button>
  <Button onclick={() => entry && revealItemInDir(entry.folder_path)}>
    Apri cartella
  </Button>
{/snippet}

<div class="flex h-full w-full flex-col">
  <PageHeader
    title={entry?.name ?? "Registrazione"}
    onBack={() => push("/")}
    actions={entry ? actions : undefined}
  />

  <div class="flex flex-1 flex-col overflow-y-auto px-6 pt-5 pb-18">
    {#if !entry}
      <p class="text-sm text-ink-muted">
        {resolving ? "Caricamento..." : "Registrazione non trovata."}
      </p>
    {:else}
      <!-- La pagina è a tutta larghezza, il testo no: oltre una settantina di
           caratteri per riga l'occhio perde il capo della riga successiva. -->
      <div class="mx-auto flex w-full max-w-[72ch] flex-col gap-4">
        <div class="rounded-xl border border-brand-cream/10 bg-brand-darker px-5 py-4">
          {#if entry.transcript}
            <TranscriptView transcript={entry.transcript} />
          {:else if entry.error}
            <p class="m-0 text-sm text-danger">Trascrizione fallita: {entry.error}</p>
          {:else}
            <p class="m-0 text-sm text-ink-muted">
              Trascrizione non ancora disponibile.
            </p>
          {/if}
        </div>

        <TranscriptNotes
          {entry}
          llmStatus={servers.llmStatus}
          llmReady={servers.llmReady}
          onLlmRefresh={servers.refreshLlm}
        />
      </div>
    {/if}
  </div>
</div>
