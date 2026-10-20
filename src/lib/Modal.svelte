<script lang="ts">
import { X } from "@lucide/svelte";
import type { Snippet } from "svelte";
import { fade, scale } from "svelte/transition";
import Button from "./Button.svelte";
import { cn } from "./utils";

let {
  title = "",
  onclose,
  maxWidth = "max-w-2xl",
  fullWindow = false,
  class: className,
  children,
  headerActions,
}: {
  title?: string;
  onclose: () => void;
  maxWidth?: string;
  fullWindow?: boolean;
  class?: string;
  children?: Snippet;
  headerActions?: Snippet;
} = $props();

let innerWidth = $state(
  typeof window !== "undefined" ? window.innerWidth : 1024,
);
let innerHeight = $state(
  typeof window !== "undefined" ? window.innerHeight : 768,
);

// Quando la finestra è troppo piccola (larghezza < 640px o altezza < 600px),
// la modale occupa l'intera finestra per sfruttare tutto lo spazio disponibile.
const isCompact = $derived(fullWindow || innerWidth < 640 || innerHeight < 600);

let backdropMousedown = false;

function handleKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.stopPropagation();
    onclose();
  }
}

function handleBackdropMousedown(e: MouseEvent) {
  backdropMousedown = e.target === e.currentTarget;
}

function handleBackdropClick(e: MouseEvent) {
  if (e.target === e.currentTarget && backdropMousedown) {
    onclose();
  }
  backdropMousedown = false;
}
</script>

<svelte:window
  bind:innerWidth
  bind:innerHeight
  onkeydown={handleKeydown}
/>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  role="dialog"
  aria-modal="true"
  aria-label={title || "Finestra modale"}
  tabindex="-1"
  transition:fade={{ duration: 150 }}
  onmousedown={handleBackdropMousedown}
  onclick={handleBackdropClick}
  class={cn(
    "fixed inset-0 z-50 flex items-center justify-center",
    isCompact
      ? "bg-brand-dark p-0"
      : "bg-black/60 p-4 sm:p-6 backdrop-blur-sm",
  )}
>
  <!-- Finestra modale -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    role="document"
    transition:scale={{ start: 0.96, duration: 150 }}
    class={cn(
      "relative flex flex-col w-full bg-brand-dark overflow-hidden transition-all duration-150",
      isCompact
        ? "h-full max-h-none rounded-none border-0 shadow-none"
        : cn(
            "max-h-[88vh] rounded-2xl border border-brand-cream/10 shadow-2xl",
            maxWidth,
            className,
          ),
    )}
    onclick={(e) => e.stopPropagation()}
  >
    <!-- Header modale -->
    <header
      class={cn(
        "flex shrink-0 items-center justify-between border-b border-brand-cream/8 bg-brand-dark/95 backdrop-blur",
        isCompact ? "px-4 py-3" : "px-6 py-4",
      )}
    >
      <div class="flex items-center gap-3 min-w-0">
        {#if title}
          <h2
            class="m-0 text-xs font-semibold tracking-wider text-ink-muted uppercase truncate"
          >
            {title}
          </h2>
        {/if}
      </div>

      <div class="flex items-center gap-2">
        {#if headerActions}
          {@render headerActions()}
        {/if}
        <Button
          variant="icon"
          onclick={onclose}
          title="Chiudi"
          aria-label="Chiudi finestra"
        >
          <X size={16} />
        </Button>
      </div>
    </header>

    <!-- Contenuto scrollabile / strutturato -->
    <div class="flex flex-1 min-h-0 flex-col overflow-hidden">
      {@render children?.()}
    </div>
  </div>
</div>
