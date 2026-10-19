<script lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Button from "./Button.svelte";
import DownloadProgressBar from "./DownloadProgressBar.svelte";
import type { DownloadProgress } from "./types";

let { onContinue }: { onContinue: () => void } = $props();

let downloading = $state(false);
let downloaded = $state(false);
let dlProgress = $state<DownloadProgress | null>(null);
let error = $state<string | null>(null);

$effect(() => {
  const unlisten = listen<DownloadProgress>("download-progress", (e) => {
    dlProgress = e.payload;
    if (e.payload.step === "done") {
      downloading = false;
      downloaded = true;
    }
  });

  return () => {
    unlisten.then((fn) => fn());
  };
});

async function startDownload() {
  error = null;
  downloading = true;
  dlProgress = null;
  try {
    await invoke("download_local_model");
  } catch (e) {
    downloading = false;
    error = String(e);
  }
}
</script>

<div
  class="fixed inset-0 z-[200] flex flex-col items-center justify-center gap-6 bg-brand-dark px-6 text-center"
>
  <img
    src="/icon.png"
    alt="Earwig"
    class="h-20 w-20 rounded-2xl shadow-pop"
  />

  <div class="flex flex-col gap-2">
    <h1 class="m-0 text-2xl font-bold text-ink-primary">Benvenuto in Earwig</h1>
    <p class="m-0 max-w-100 text-sm leading-relaxed text-ink-secondary">
      Per trascrivere le tue registrazioni serve un modello vocale che gira
      interamente sul tuo Mac, senza inviare nulla a server esterni.
    </p>
  </div>

  {#if downloaded}
    <p class="m-0 max-w-100 text-sm leading-relaxed text-ink-secondary">
      Modello pronto. Potrai eliminarlo o scaricarlo di nuovo in qualsiasi
      momento dalle impostazioni.
    </p>
    <Button variant="primary" onclick={onContinue}>Continua</Button>
  {:else if downloading}
    <div class="flex w-full max-w-80 flex-col gap-1.5">
      {#if dlProgress}
        <DownloadProgressBar progress={dlProgress} trackClass="bg-brand-darker" />
      {/if}
    </div>
    <p class="m-0 text-xs text-ink-muted">
      Nel mentre puoi anche prenderti un caffè, ma non uscire dall'app!
    </p>
  {:else}
    <Button variant="solid" class="px-6 py-2.5 text-sm" onclick={startDownload}>
      Scarica il modello
    </Button>
    {#if error}
      <p class="m-0 max-w-100 text-xs text-danger">{error}</p>
    {/if}
  {/if}
</div>
