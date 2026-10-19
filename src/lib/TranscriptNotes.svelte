<script lang="ts">
import { invoke } from "@tauri-apps/api/core";
import Button from "./Button.svelte";
import {
  buildTranscriptContext,
  parseSummary,
  streamChatReply,
  streamSummary,
} from "./llm";
import TranscriptChat from "./TranscriptChat.svelte";
import type {
  ChatMessage,
  RecordingEntry,
  RecordingNotes,
  ServerStatus,
  SttSettings,
} from "./types";

let {
  entry,
  llmStatus,
  llmReady,
  onLlmRefresh,
}: {
  entry: RecordingEntry;
  llmStatus: ServerStatus;
  llmReady: boolean;
  onLlmRefresh: (opts?: {
    attemptStart?: boolean;
  }) => Promise<SttSettings | null>;
} = $props();

let notes = $state<RecordingNotes>({ version: 1, summary: null, messages: [] });
let loadingNotes = $state(true);
let summaryBusy = $state(false);
let summaryError = $state<string | null>(null);
let chatBusy = $state(false);
let chatError = $state<string | null>(null);
let streamingText = $state("");
let pendingQuestion = $state<string | null>(null);

let controller: AbortController | null = null;

let context = $derived(
  entry.transcript ? buildTranscriptContext(entry.transcript) : null,
);

async function persist() {
  await invoke("write_recording_notes", {
    folderPath: entry.folder_path,
    notes,
  });
}

async function loadNotes() {
  loadingNotes = true;
  notes = await invoke<RecordingNotes>("read_recording_notes", {
    folderPath: entry.folder_path,
  });
  loadingNotes = false;

  // Automatico solo se il server è già pronto: mai avviare il server o
  // scaricare un modello a sorpresa per aver aperto un dettaglio (stessa
  // filosofia di ensure_stt_ready lato backend).
  if (!notes.summary && llmStatus === "running") {
    generateSummary();
  }
}

$effect(() => {
  loadNotes();
});

// Uscire dal dettaglio a metà streaming deve interrompere la richiesta,
// altrimenti lo slot del server locale resta occupato dietro le quinte.
$effect(() => {
  return () => {
    controller?.abort();
  };
});

async function generateSummary() {
  if (!context || summaryBusy) return;
  summaryBusy = true;
  summaryError = null;
  streamingText = "";
  controller = new AbortController();
  try {
    const settings = await invoke<SttSettings>("get_stt_settings");
    const result = streamSummary(context, controller.signal);
    let raw = "";
    for await (const delta of result.textStream) {
      raw += delta;
      streamingText = raw;
    }
    const parsed = parseSummary(raw);
    notes = {
      ...notes,
      summary: {
        ...parsed,
        model: settings.llmHfFile,
        generated_at: new Date().toISOString(),
      },
    };
    await persist();
  } catch (e) {
    if (!(e instanceof DOMException && e.name === "AbortError")) {
      summaryError = String(e);
    }
  } finally {
    streamingText = "";
    summaryBusy = false;
    controller = null;
  }
}

async function sendMessage(text: string) {
  if (!context || chatBusy) return;
  pendingQuestion = text;
  chatBusy = true;
  chatError = null;
  streamingText = "";
  controller = new AbortController();
  try {
    const result = streamChatReply(
      context,
      notes.messages,
      text,
      controller.signal,
    );
    let raw = "";
    for await (const delta of result.textStream) {
      raw += delta;
      streamingText = raw;
    }
    const at = new Date().toISOString();
    const userMsg: ChatMessage = {
      id: crypto.randomUUID(),
      role: "user",
      content: text,
      at,
    };
    const assistantMsg: ChatMessage = {
      id: crypto.randomUUID(),
      role: "assistant",
      content: raw,
      at,
    };
    notes = { ...notes, messages: [...notes.messages, userMsg, assistantMsg] };
    pendingQuestion = null;
    await persist();
  } catch (e) {
    if (e instanceof DOMException && e.name === "AbortError") {
      // Abort: si scarta anche la domanda, niente da ripristinare.
      pendingQuestion = null;
    } else {
      chatError = String(e);
      // pendingQuestion resta valorizzato: TranscriptChat lo rimette nel
      // composer per un retry a un click senza dover riscrivere la domanda.
    }
  } finally {
    streamingText = "";
    chatBusy = false;
    controller = null;
  }
}

function stopStreaming() {
  controller?.abort();
}

let hasContent = $derived(notes.summary != null || notes.messages.length > 0);
</script>

{#if entry.transcript}
  <div class="flex flex-col gap-4 rounded-xl border border-brand-cream/10 bg-brand-darker px-5 py-4">
    <div class="flex items-center justify-between">
      <h3 class="m-0 text-xs font-semibold tracking-wider text-ink-muted uppercase">
        Riassunto e note
      </h3>
      {#if notes.summary}
        <Button
          onclick={generateSummary}
          disabled={summaryBusy || llmStatus !== "running"}
          title={llmStatus !== "running" ? "Avvia il server LLM per rigenerare" : undefined}
        >
          {summaryBusy ? "..." : "Rigenera"}
        </Button>
      {/if}
    </div>

    {#if loadingNotes}
      <p class="m-0 text-sm text-ink-muted">Caricamento...</p>
    {:else}
      {#if notes.summary}
        <div class="flex flex-col gap-3 text-sm text-ink-primary">
          <p class="m-0 leading-relaxed">{notes.summary.text}</p>
          <div>
            <h4 class="m-0 mb-1 text-2xs font-semibold tracking-wider text-ink-faint uppercase">
              Punti chiave
            </h4>
            {#if notes.summary.key_points.length > 0}
              <ul class="m-0 list-disc pl-5">
                {#each notes.summary.key_points as point}<li>{point}</li>{/each}
              </ul>
            {:else}
              <p class="m-0 text-ink-muted">Nessuno.</p>
            {/if}
          </div>
          <div>
            <h4 class="m-0 mb-1 text-2xs font-semibold tracking-wider text-ink-faint uppercase">
              Azioni
            </h4>
            {#if notes.summary.actions.length > 0}
              <ul class="m-0 list-disc pl-5">
                {#each notes.summary.actions as action}<li>{action}</li>{/each}
              </ul>
            {:else}
              <p class="m-0 text-ink-muted">Nessuna azione emersa.</p>
            {/if}
          </div>
          <div>
            <h4 class="m-0 mb-1 text-2xs font-semibold tracking-wider text-ink-faint uppercase">
              Domande aperte
            </h4>
            {#if notes.summary.open_questions.length > 0}
              <ul class="m-0 list-disc pl-5">
                {#each notes.summary.open_questions as q}<li>{q}</li>{/each}
              </ul>
            {:else}
              <p class="m-0 text-ink-muted">Nessuna.</p>
            {/if}
          </div>
        </div>
      {:else if summaryBusy}
        <p class="m-0 whitespace-pre-wrap text-sm text-ink-secondary">
          {streamingText || "Generazione in corso..."}
        </p>
      {:else if llmStatus === "running"}
        <Button onclick={generateSummary}>Genera riassunto e note</Button>
      {:else if llmStatus === "loading"}
        <p class="m-0 text-sm text-ink-muted">
          Caricamento modello... può richiedere qualche minuto al primo avvio.
        </p>
      {:else}
        <div class="flex flex-col items-start gap-2">
          <p class="m-0 text-sm text-ink-muted">
            {llmReady
              ? "Genera un riassunto e apri una chat sulla trascrizione con un modello locale."
              : "Scarica un modello LLM dalle impostazioni per generare riassunto e chat sulla trascrizione."}
          </p>
          <Button
            class="disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:text-ink-secondary"
            onclick={() => onLlmRefresh({ attemptStart: true })}
            disabled={!llmReady}
            title={llmReady ? undefined : "Scarica prima il modello dalle impostazioni"}
          >
            Avvia il server LLM
          </Button>
        </div>
      {/if}

      {#if summaryError}
        <p class="m-0 text-sm text-danger">{summaryError}</p>
      {/if}
    {/if}

    {#if !loadingNotes && (llmStatus === "running" || hasContent)}
      <div class="border-t border-brand-cream/10 pt-3">
        <TranscriptChat
          messages={notes.messages}
          streamingText={chatBusy ? streamingText : ""}
          busy={chatBusy}
          disabled={llmStatus !== "running"}
          retryDraft={chatError ? pendingQuestion : null}
          onSend={sendMessage}
          onStop={stopStreaming}
        />
        {#if chatError}
          <p class="m-0 mt-2 text-sm text-danger">{chatError}</p>
        {/if}
      </div>
    {/if}
  </div>
{/if}
