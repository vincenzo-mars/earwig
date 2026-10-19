<script lang="ts">
import type { Snippet } from "svelte";
import type { HTMLButtonAttributes } from "svelte/elements";
import { cn } from "$lib/utils";

const VARIANTS = {
  ghost:
    "cursor-pointer rounded-lg border border-brand-cream/15 bg-brand-darker px-3 py-1.5 text-xs text-ink-secondary transition-colors hover:text-ink-primary",
  icon: "cursor-pointer rounded-full bg-brand-darker/85 p-2 shadow-[0_2px_12px_rgba(0,0,0,0.3)] backdrop-blur-sm transition-all hover:bg-brand-cream hover:text-brand-darker",
  solid:
    "cursor-pointer rounded-lg bg-brand-cream px-3 py-2 text-sm font-semibold text-brand-ink transition hover:bg-brand-cream-dim disabled:cursor-not-allowed disabled:bg-brand-lighter disabled:text-ink-faint",
  danger:
    "cursor-pointer rounded-lg bg-red-600 px-3 py-2 text-sm font-semibold text-ink-primary transition hover:bg-red-700 disabled:cursor-not-allowed disabled:bg-red-600/30",
  primary:
    "cursor-pointer rounded-lg bg-brand-cream px-6 py-2.5 text-sm font-semibold text-brand-ink transition hover:bg-brand-cream-dim",
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
