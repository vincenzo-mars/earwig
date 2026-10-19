<script lang="ts">
import Button from "./Button.svelte";
import type { ChatMessage } from "./types";

let {
  messages,
  streamingText,
  busy,
  disabled,
  retryDraft,
  onSend,
  onStop,
}: {
  messages: ChatMessage[];
  streamingText: string;
  busy: boolean;
  disabled: boolean;
  retryDraft: string | null;
  onSend: (text: string) => void;
  onStop: () => void;
} = $props();

let draft = $state("");

// Un errore di invio ripristina la domanda nel composer per un retry a un
// click, senza dover riscrivere: vedi `retryDraft` in TranscriptNotes.
$effect(() => {
  if (retryDraft != null) draft = retryDraft;
});

function submit() {
  const text = draft.trim();
  if (!text || busy || disabled) return;
  draft = "";
  onSend(text);
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    submit();
  }
}
</script>

<div class="flex flex-col gap-3">
  {#if messages.length > 0 || streamingText}
    <div class="flex flex-col gap-2.5">
      {#each messages as message (message.id)}
        <div
          class={`max-w-[85%] rounded-xl px-3.5 py-2 text-sm leading-relaxed whitespace-pre-wrap ${
            message.role === "user"
              ? "self-end bg-brand-light text-brand-ink"
              : "self-start bg-brand-darker text-ink-primary"
          }`}
        >
          {message.content}
        </div>
      {/each}
      {#if streamingText}
        <div
          class="max-w-[85%] self-start rounded-xl bg-brand-darker px-3.5 py-2 text-sm leading-relaxed whitespace-pre-wrap text-ink-primary"
        >
          {streamingText}
        </div>
      {/if}
    </div>
  {/if}

  <div class="flex items-end gap-2">
    <textarea
      class="min-h-10 flex-1 resize-none rounded-lg border border-brand-cream/15 bg-brand-darker px-3 py-2 text-sm text-ink-primary placeholder:text-ink-faint focus:outline-none focus:border-brand-light/70 focus:ring-2 focus:ring-brand-light/40 disabled:opacity-50"
      rows="1"
      placeholder="Fai una domanda sulla trascrizione..."
      bind:value={draft}
      onkeydown={handleKeydown}
      disabled={disabled || busy}
    ></textarea>
    {#if busy}
      <Button onclick={onStop}>Interrompi</Button>
    {:else}
      <Button onclick={submit} disabled={disabled || !draft.trim()}>Invia</Button>
    {/if}
  </div>
</div>
