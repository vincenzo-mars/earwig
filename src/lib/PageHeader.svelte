<script lang="ts">
import { ArrowLeft } from "@lucide/svelte";
import type { Snippet } from "svelte";
import Button from "./Button.svelte";

let {
  title,
  onBack,
  actions,
}: {
  title: string;
  onBack: () => void;
  actions?: Snippet;
} = $props();
</script>

<header
  class="sticky top-0 z-20 flex shrink-0 items-center gap-3 border-b border-brand-cream/10 bg-brand-dark/95 px-6 py-3 backdrop-blur"
>
  <Button
    variant="icon"
    onclick={onBack}
    title="Indietro"
    aria-label="Torna indietro"
  >
    <ArrowLeft size={16} />
  </Button>
  <h2
    class="m-0 text-xs font-semibold tracking-wider text-ink-muted uppercase"
  >
    {title}
  </h2>
  {#if actions}
    <div class="ml-auto flex items-center gap-2">{@render actions()}</div>
  {/if}
</header>
