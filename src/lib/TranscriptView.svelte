<script lang="ts">
import { Check, Copy } from "@lucide/svelte";
import type { TranscriptResult } from "./types";
import { groupSegments } from "./types";

let { transcript }: { transcript: TranscriptResult } = $props();

const groups = $derived(groupSegments(transcript.segments));

let copiedIndex = $state<number | null>(null);
let copyTimeout: ReturnType<typeof setTimeout> | null = null;

async function handleCopyTurn(text: string, index: number) {
  try {
    await navigator.clipboard.writeText(text);
    copiedIndex = index;
    if (copyTimeout) clearTimeout(copyTimeout);
    copyTimeout = setTimeout(() => {
      if (copiedIndex === index) copiedIndex = null;
    }, 1500);
  } catch (e) {
    console.error("Failed to copy turn:", e);
  }
}
</script>

{#if groups.length === 0}
  <div class="py-6 text-xs text-ink-muted">
    Nessun testo rilevato nella registrazione.
  </div>
{:else}
  <div class="flex flex-col gap-5 text-sm leading-relaxed">
    {#each groups as g, i (i)}
      {@const isMe = g.speaker === "0"}
      {@const speakerName = isMe ? "Tu:" : g.speaker === "1" ? "Altri:" : "Voce:"}
      <div
        class={`group relative flex flex-col gap-1 ${
          isMe
            ? "items-end text-right ml-auto max-w-[85%]"
            : "items-start text-left mr-auto max-w-[85%]"
        }`}
      >
        <div
          class={`flex items-center gap-2 ${
            isMe ? "flex-row-reverse" : "flex-row"
          }`}
        >
          <span
            class={`font-semibold tracking-wide ${
              isMe ? "text-red-400" : "text-purple-400"
            }`}
          >
            {speakerName}
          </span>

          <button
            type="button"
            class="opacity-0 group-hover:opacity-100 transition-opacity duration-150 inline-flex items-center gap-1 text-2xs text-ink-faint hover:text-ink-primary cursor-pointer border-none bg-transparent p-0"
            onclick={() => handleCopyTurn(g.text, i)}
            title="Copia intervento"
          >
            {#if copiedIndex === i}
              <Check size={11} class="text-ok" />
              <span class="text-ok text-2xs font-medium">Copiato</span>
            {:else}
              <Copy size={11} />
            {/if}
          </button>
        </div>

        <p
          class="m-0 select-text text-ink-primary whitespace-pre-wrap leading-relaxed font-normal"
        >
          {g.text}
        </p>
      </div>
    {/each}
  </div>
{/if}
