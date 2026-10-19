<script lang="ts">
import type { SttStatus } from "./types";

let { status }: { status: SttStatus } = $props();

// Nessuno stato ha sfondo o ombra: l'indicatore è un dettaglio di servizio,
// la sua importanza si legge solo dal colore del dot e dal lampeggio.
// `checking` e `starting` sono per chi guarda lo stesso momento: una
// transizione in corso senza errori. Il tipo li distingue, la UI no.
const TRANSIENT = {
  label: "Avvio...",
  dot: "bg-warn animate-blink",
};

const STATUS: Record<SttStatus, { label: string; dot: string }> = {
  checking: TRANSIENT,
  starting: TRANSIENT,
  running: {
    label: "Server attivo",
    dot: "bg-ok/60",
  },
  error: {
    label: "Server non disponibile",
    dot: "bg-danger",
  },
  stopped: {
    label: "Server fermo",
    dot: "bg-brand-light",
  },
};
</script>

<div class="fixed left-4 bottom-4 z-10 flex items-center gap-2 text-xs text-ink-muted">
  <span class={`h-2 w-2 shrink-0 rounded-full ${STATUS[status].dot}`}></span>
  {STATUS[status].label}
</div>
