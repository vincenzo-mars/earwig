<script lang="ts">
// Blocco stato + controlli di un server locale (whisper o llama), identico
// per i due: cambia solo cosa fa il bottone rosso e le condizioni di gate.
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
  title: string;
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
  <span class="text-xs font-semibold text-ink-primary">{title}</span>

  <div
    class="flex flex-col gap-2 rounded-lg border border-brand-cream/10 bg-brand-dark/50 px-2.5 py-2"
  >
    <div class="flex items-center justify-between">
      <span class="text-xs text-ink-secondary">Stato:</span>
      <span
        class={`text-xs font-medium ${
          status === "running" ? "text-green-400" : "text-ink-muted"
        }`}
      >
        {status === "running"
          ? "Attivo"
          : status === "loading"
            ? "Caricamento..."
            : "Fermo"}
      </span>
    </div>

    {#if error}
      <p class="m-0 text-xs text-red-400 leading-tight">{error}</p>
    {:else if hint}
      <p class="m-0 text-xs text-ink-muted leading-tight">{hint}</p>
    {/if}
  </div>

  <div class="grid grid-cols-2 gap-1.5">
    <Button variant="solid" onclick={onStart} disabled={loading || locked || !canStart}>
      {loading ? "..." : "Avvia"}
    </Button>
    <Button variant="solid" onclick={onRestart} disabled={loading || locked || !canStart}>
      {loading ? "..." : "Riavvia"}
    </Button>
    <Button variant="solid" onclick={onStop} disabled={loading || locked}>
      {loading ? "..." : "Spegni"}
    </Button>
    <Button variant="danger" onclick={onDanger} disabled={loading || locked}>
      {loading ? "..." : dangerLabel}
    </Button>
  </div>
</div>
