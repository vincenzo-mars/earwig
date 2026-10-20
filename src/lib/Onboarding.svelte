<script lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { open } from "@tauri-apps/plugin-dialog";
import Button from "./Button.svelte";
import DownloadProgressBar from "./DownloadProgressBar.svelte";
import type { DownloadProgress, SttSettings } from "./types";

let { onContinue }: { onContinue: () => void } = $props();

let downloading = $state(false);
let downloaded = $state(false);
let dlProgress = $state<DownloadProgress | null>(null);
let error = $state<string | null>(null);
let transcriptsDir = $state<string | null>(null);
let autostartEnabled = $state(true);

$effect(() => {
  isEnabled()
    .then((val) => {
      if (!val) {
        enable()
          .then(() => {
            autostartEnabled = true;
          })
          .catch((err) => console.error(err));
      } else {
        autostartEnabled = true;
      }
    })
    .catch(() => {
      autostartEnabled = true;
    });
});

async function toggleAutostart() {
  try {
    if (autostartEnabled) {
      await disable();
      autostartEnabled = false;
    } else {
      await enable();
      autostartEnabled = true;
    }
  } catch (e) {
    console.error("Failed to toggle autostart:", e);
  }
}

$effect(() => {
  invoke<SttSettings>("get_stt_settings").then((s) => {
    transcriptsDir = s.transcriptsDir;
  });

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

async function chooseTranscriptsDir() {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Seleziona la cartella per le note Markdown",
  });
  if (selected && typeof selected === "string") {
    transcriptsDir = await invoke<string>("set_transcripts_dir", {
      path: selected,
    });
  }
}

async function resetTranscriptsDir() {
  await invoke("set_transcripts_dir", { path: null });
  transcriptsDir = null;
}

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

  <div
    class="flex flex-col gap-1 max-w-100 rounded-lg border border-brand-cream/8 bg-brand-dark/40 p-3 text-left"
  >
    <span class="text-2xs font-semibold text-ink-primary">
      Hai visto "Earwig è danneggiata" al primo avvio?
    </span>
    <span class="text-2xs leading-relaxed text-ink-secondary">
      L'app non è notarizzata da Apple e macOS blocca quelle scaricate dal web.
      Da Terminale esegui
      <code
        class="rounded bg-brand-cream/10 px-1 py-0.5 font-mono text-ink-primary"
      >
        xattr -cr /Applications/Earwig.app
      </code>
      e riaprila: il comando rimuove solo il flag di quarantena.
    </span>
  </div>

  {#if downloaded}
    <div class="flex flex-col items-center gap-4 max-w-sm w-full">
      <div class="flex flex-col gap-1">
        <h2 class="m-0 text-sm font-semibold text-ink-primary">
          Note Markdown
        </h2>
        <p class="m-0 text-xs leading-relaxed text-ink-secondary">
          Le trascrizioni vengono formattate come note <code>.md</code> strutturate con metadati YAML.
          Puoi scegliere subito una cartella dedicata o salvare nella posizione predefinita.
        </p>
      </div>

      <div class="w-full flex flex-col gap-2 rounded-lg border border-brand-cream/8 bg-brand-dark/40 p-3 text-left">
        <div class="flex items-center justify-between gap-2">
          <span class="text-2xs font-medium uppercase tracking-wider text-ink-muted">
            Cartella note .md
          </span>
          {#if transcriptsDir}
            <span class="inline-flex items-center gap-1 rounded-full border border-ok/20 bg-ok/10 px-2 py-0.5 text-2xs font-medium text-ok">
              Personalizzata
            </span>
          {:else}
            <span class="inline-flex items-center rounded-full border border-brand-cream/10 bg-brand-cream/5 px-2 py-0.5 text-2xs font-medium text-ink-muted">
              Predefinita
            </span>
          {/if}
        </div>

        <button
          type="button"
          class="flex w-full items-center justify-between gap-2 rounded-md border border-brand-cream/8 bg-brand-cream/[0.03] px-2.5 py-1.5 text-left font-mono text-2xs text-ink-muted hover:border-brand-cream/25 hover:bg-brand-cream/10 hover:text-ink-primary transition-colors cursor-pointer"
          onclick={chooseTranscriptsDir}
          title="Scegli cartella per le note Markdown"
        >
          <span class="truncate">
            {transcriptsDir ?? "Cartella registrazioni Earwig"}
          </span>
          <span class="shrink-0 text-2xs text-ink-faint font-sans">
            Scegli ↗
          </span>
        </button>

        {#if transcriptsDir}
          <div class="flex justify-end">
            <button
              type="button"
              class="text-2xs text-ink-faint hover:text-ink-primary underline cursor-pointer border-none bg-transparent p-0"
              onclick={resetTranscriptsDir}
            >
              Ripristina predefinita
            </button>
          </div>
        {/if}
      </div>

      <div class="w-full flex items-center justify-between rounded-lg border border-brand-cream/8 bg-brand-dark/40 p-3 text-left">
        <div class="flex flex-col gap-0.5 pr-2">
          <span class="text-xs font-semibold text-ink-primary">Avvia con il Mac</span>
          <span class="text-2xs text-ink-secondary">
            Resta pronta in background nella barra dei menu in alto a destra
          </span>
        </div>
        <button
          type="button"
          role="switch"
          aria-label="Avvia con il Mac"
          title="Avvia con il Mac"
          aria-checked={autostartEnabled}
          onclick={toggleAutostart}
          class="relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none {autostartEnabled ? 'bg-brand-red' : 'bg-brand-cream/15'}"
        >
          <span
            class="pointer-events-none inline-block h-4 w-4 transform rounded-full bg-brand-cream shadow-sm ring-0 transition duration-200 ease-in-out {autostartEnabled ? 'translate-x-4' : 'translate-x-0'}"
          ></span>
        </button>
      </div>

      <Button variant="primary" class="mt-2 w-full" onclick={onContinue}>
        Inizia a usare Earwig
      </Button>
    </div>
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
