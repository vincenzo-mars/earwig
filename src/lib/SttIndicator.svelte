<script lang="ts">
import type { SttStatus } from "./types";

let { status }: { status: SttStatus } = $props();

const STATUS: Record<SttStatus, { label: string; text: string; dot: string }> =
  {
    checking: {
      label: "Controllo...",
      text: "text-brand-cream/50",
      dot: "bg-gray-400 animate-[blink_1.2s_ease-in-out_infinite]",
    },
    starting: {
      label: "Avvio server...",
      text: "text-brand-cream/50",
      dot: "bg-amber-500 animate-[blink_0.8s_ease-in-out_infinite]",
    },
    running: {
      label: "Server attivo",
      // Stato sano: deve sparire, non decorare. Gli stati anomali
      // (errore, avvio) tengono la pillola e il testo pieno.
      text: "text-brand-cream/45",
      dot: "bg-green-500/70",
    },
    error: {
      label: "Server non disponibile",
      text: "text-brand-cream",
      dot: "bg-red-400",
    },
    stopped: {
      label: "Server fermo",
      text: "text-brand-cream",
      dot: "bg-gray-400",
    },
    // whisper-server non passa mai da questo stato (a differenza di
    // llama-server, apre la porta solo a modello già caricato): presente
    // solo perché SttStatus è un alias di ServerStatus.
    loading: {
      label: "Caricamento...",
      text: "text-brand-cream/50",
      dot: "bg-amber-500 animate-[blink_0.8s_ease-in-out_infinite]",
    },
  };
</script>

<!-- La pillola (sfondo + ombra) esiste solo per gli stati che chiedono
     attenzione: il running sano galleggia senza contenitore, come un
     dettaglio di servizio e non come un elemento UI da guardare. -->
<div
  class={`fixed left-4 bottom-4 z-10 flex items-center rounded-full py-2 px-3 ${
    status === "running" ? "" : "bg-brand-darker/85 shadow-[0_2px_12px_rgba(0,0,0,0.3)]"
  }`}
>
  <div
    class={`flex items-center gap-2 text-sm font-medium ${STATUS[status].text}`}
  >
    <span class={`h-2.5 w-2.5 shrink-0 rounded-full ${STATUS[status].dot}`}
    ></span>
    {STATUS[status].label}
  </div>
</div>
