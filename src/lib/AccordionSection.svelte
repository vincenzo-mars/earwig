<script lang="ts">
import { ChevronDown } from "@lucide/svelte";
import type { Component, Snippet } from "svelte";
import { slide } from "svelte/transition";
import { cn } from "./utils";

let {
  id,
  title,
  icon: Icon,
  badge,
  isOpen = $bindable(true),
  class: className = "",
  children,
}: {
  id?: string;
  title: string;
  icon: Component<{ size?: number; class?: string }>;
  badge?: string;
  isOpen?: boolean;
  class?: string;
  children: Snippet;
} = $props();
</script>

<div
  {id}
  class={cn(
    "flex flex-col rounded-lg border border-brand-cream/8 bg-brand-dark/40 transition-all duration-150",
    !isOpen && "hover:border-brand-cream/25 hover:bg-brand-cream/[0.04] active:scale-[0.995]",
    isOpen && "hover:border-brand-cream/15",
    className,
  )}
>
  <button
    type="button"
    class={cn(
      "group flex w-full cursor-pointer select-none items-center justify-between gap-3 px-3.5 py-2.5 text-left text-ink-primary focus:outline-none transition-colors",
      isOpen ? "rounded-t-lg hover:bg-brand-cream/[0.04]" : "rounded-lg",
    )}
    onclick={() => (isOpen = !isOpen)}
    aria-expanded={isOpen}
  >
    <div class="flex items-center gap-2.5">
      <Icon
        size={17}
        class="text-ink-secondary transition-colors group-hover:text-brand-cream"
      />
      <h2
        class="text-sm font-semibold tracking-wide text-ink-primary transition-colors group-hover:text-brand-cream"
      >
        {title}
      </h2>
      {#if badge}
        <span
          class="rounded-full border border-brand-cream/15 bg-brand-cream/5 px-2 py-0.5 text-2xs font-medium text-ink-muted"
        >
          {badge}
        </span>
      {/if}
    </div>
    <ChevronDown
      size={15}
      class={`text-ink-faint transition-all duration-200 group-hover:text-ink-primary ${isOpen ? "rotate-180" : ""}`}
    />
  </button>

  {#if isOpen}
    <div transition:slide={{ duration: 200 }} class="overflow-hidden border-t border-brand-cream/6">
      {@render children()}
    </div>
  {/if}
</div>
