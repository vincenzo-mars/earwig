<script lang="ts">
import type { Snippet } from "svelte";
import type { HTMLButtonAttributes } from "svelte/elements";
import { cn } from "$lib/utils";

const VARIANTS = {
  ghost:
    "inline-flex items-center justify-center cursor-pointer rounded-lg border border-brand-cream/15 bg-brand-darker px-3 py-1.5 text-xs text-ink-secondary transition-colors hover:text-ink-primary",
  icon: "inline-flex items-center justify-center shrink-0 cursor-pointer rounded-full border border-brand-cream/15 bg-brand-darker/85 size-8 p-0 text-ink-secondary shadow-raised backdrop-blur-sm transition-all enabled:hover:border-brand-cream/30 enabled:hover:bg-brand-cream enabled:hover:text-brand-darker enabled:active:scale-[0.95] disabled:cursor-default disabled:opacity-40",
  solid:
    "inline-flex items-center justify-center cursor-pointer rounded-lg bg-brand-cream px-3 py-2 text-sm font-semibold text-brand-ink transition hover:bg-brand-cream-dim disabled:cursor-not-allowed disabled:bg-brand-lighter disabled:text-ink-faint",
  danger:
    "inline-flex items-center justify-center cursor-pointer rounded-lg bg-danger-strong px-3 py-2 text-sm font-semibold text-ink-primary transition hover:bg-danger-strong/90 disabled:cursor-not-allowed disabled:bg-danger-strong/30",
  primary:
    "inline-flex items-center justify-center cursor-pointer rounded-lg bg-brand-cream px-6 py-2.5 text-sm font-semibold text-brand-ink transition hover:bg-brand-cream-dim",
} as const;

let {
  variant = "ghost",
  class: className,
  children,
  ...restProps
}: HTMLButtonAttributes & {
  variant?: keyof typeof VARIANTS;
  children?: Snippet;
} = $props();
</script>

<button class={cn(VARIANTS[variant], className)} {...restProps}>
  {@render children?.()}
</button>
