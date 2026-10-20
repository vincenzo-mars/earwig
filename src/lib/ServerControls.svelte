<script lang="ts">
import { Play, RotateCcw, Square, Trash2 } from "@lucide/svelte";
import Button from "./Button.svelte";
import type { ServerStatus } from "./types";

let {
  title,
  status,
  error,
  hint = null,
  loading,
  locked,
  canStart = true,
  dangerLabel,
  onStart,
  onRestart,
  onStop,
  onDanger,
}: {
  title?: string;
  status: ServerStatus;
  error: string | null;
  hint?: string | null;
  loading: boolean;
  locked: boolean;
  canStart?: boolean;
  dangerLabel: string;
  onStart: () => void;
  onRestart: () => void;
  onStop: () => void;
  onDanger: () => void;
} = $props();
</script>

<div class="flex flex-col gap-1.5">
  {#if title}
    <span class="text-xs font-semibold text-ink-primary">{title}</span>
  {/if}

  <div
    class="flex flex-col gap-2 rounded-lg border border-brand-cream/8 bg-brand-dark/40 px-3 py-2.5"
  >
    <div class="flex items-center justify-between gap-3">
      <div class="flex items-center gap-2">
        <span class="text-xs text-ink-secondary">Stato:</span>
        <div class="flex items-center gap-1.5">
          <span
            class={`size-2 shrink-0 rounded-full ${
              status === "running"
                ? "bg-ok"
                : status === "loading"
                  ? "bg-warn animate-blink"
                  : "bg-brand-light"
            }`}
          ></span>
          <span
            class={`text-xs font-medium ${
              status === "running"
                ? "text-ok"
                : status === "loading"
                  ? "text-warn"
                  : "text-ink-muted"
            }`}
          >
            {status === "running"
              ? "Attivo"
              : status === "loading"
                ? "Caricamento..."
                : "Fermo"}
          </span>
        </div>
      </div>

      <div class="flex items-center gap-1">
        <Button
          variant="icon"
          class="size-7 text-ink-secondary hover:text-ink-primary"
          onclick={onStart}
          disabled={loading || locked || !canStart || status === "running"}
          title="Avvia server"
          aria-label="Avvia server"
        >
          <Play size={13} fill="currentColor" class="translate-x-px" />
        </Button>

        <Button
          variant="icon"
          class="size-7 text-ink-secondary hover:text-ink-primary"
          onclick={onRestart}
          disabled={loading || locked || !canStart}
          title="Riavvia server"
          aria-label="Riavvia server"
        >
          <RotateCcw size={13} />
        </Button>

        <Button
          variant="icon"
          class="size-7 text-ink-secondary hover:text-ink-primary"
          onclick={onStop}
          disabled={loading || locked || status === "stopped"}
          title="Ferma server"
          aria-label="Ferma server"
        >
          <Square size={13} fill="currentColor" />
        </Button>

        <Button
          variant="icon"
          class="size-7 text-danger hover:border-danger/30 hover:bg-danger/10 hover:text-danger"
          onclick={onDanger}
          disabled={loading || locked}
          title={dangerLabel}
          aria-label={dangerLabel}
        >
          <Trash2 size={13} />
        </Button>
      </div>
    </div>

    {#if error}
      <p class="m-0 text-xs text-danger leading-tight">{error}</p>
    {:else if hint}
      <p class="m-0 text-2xs text-ink-muted leading-tight">{hint}</p>
    {/if}
  </div>
</div>
