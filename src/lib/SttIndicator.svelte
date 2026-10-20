<script lang="ts">
import { Server } from "@lucide/svelte";
import { push } from "svelte-spa-router";
import type { SttStatus } from "./types";

let { status }: { status: SttStatus } = $props();

// Indicatore di stato in alto a sinistra: dot semantico a riposo,
// label ed icona server visibili al passaggio del mouse con transizione fluida.
// Cliccabile per aprire la pagina impostazioni direttamente sulla sezione del server.
const TRANSIENT = {
  label: "Avvio...",
  dot: "bg-warn animate-blink",
};

const STATUS: Record<SttStatus, { label: string; dot: string }> = {
  checking: TRANSIENT,
  starting: TRANSIENT,
  running: {
    label: "STT pronto",
    dot: "bg-ok",
  },
  error: {
    label: "STT non disponibile",
    dot: "bg-danger",
  },
  stopped: {
    label: "STT non scaricato",
    dot: "bg-brand-light",
  },
};

function handleClick() {
  push("/settings?section=stt-server");
}
</script>

<button
  type="button"
  onclick={handleClick}
  class="group fixed left-6 top-6 z-10 flex h-8 select-none items-center gap-1.5 border-none bg-transparent p-0 text-xs text-ink-muted transition-colors hover:text-ink-primary cursor-pointer focus:outline-none focus-visible:ring-1 focus-visible:ring-brand-light/40 rounded before:absolute before:-inset-2 before:content-['']"
  title="Apri impostazioni server"
  aria-label={`Stato server: ${STATUS[status].label}. Apri impostazioni server`}
>
  <span class={`h-2 w-2 shrink-0 rounded-full ${STATUS[status].dot}`}></span>
  <Server size={14} class="shrink-0 text-ink-secondary transition-colors group-hover:text-ink-primary" />
  <span
    class="overflow-hidden whitespace-nowrap max-w-0 opacity-0 transition-all duration-200 ease-out group-hover:max-w-48 group-hover:opacity-100 group-focus-visible:max-w-48 group-focus-visible:opacity-100"
  >
    <span>{STATUS[status].label}</span>
  </span>
</button>

