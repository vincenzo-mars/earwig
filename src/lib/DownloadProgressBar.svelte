<script lang="ts">
// Barra di avanzamento download modello, condivisa da Onboarding e
// SettingsPanel (whisper + LLM). `step` distingue solo "done" dal resto:
// gli step attivi ("model", "llm") mostrano entrambi la percentuale.
let {
  progress,
  trackClass = "bg-brand-dark",
}: {
  progress: { step: string; pct: number };
  trackClass?: string;
} = $props();
</script>

<div class="flex flex-col gap-1 pt-1">
  <div class={`h-1.5 overflow-hidden rounded-full ${trackClass}`}>
    <div
      class="h-full rounded-full bg-brand-cream transition-[width] duration-300"
      style={`width: ${progress.pct}%`}
    ></div>
  </div>
  <span class="text-xs text-ink-muted">
    {progress.step === "done" ? "Completato" : `Modello ${progress.pct}%`}
  </span>
</div>
