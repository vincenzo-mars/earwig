<script lang="ts">
import {
  Check,
  Download,
  FileText,
  Folder,
  LoaderCircle,
  Mic,
  MonitorCheck,
  MonitorX,
  Play,
  Power,
  RotateCcw,
  Server,
  ShieldCheck,
  Square,
  Trash2,
} from "@lucide/svelte";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { pop, router } from "svelte-spa-router";
import AccordionSection from "./AccordionSection.svelte";
import Button from "./Button.svelte";
import DownloadProgressBar from "./DownloadProgressBar.svelte";
import Modal from "./Modal.svelte";
import { servers } from "./stores/servers.svelte";
import { session } from "./stores/session.svelte";
import type { DownloadProgress, SttSettings } from "./types";

let { onclose }: { onclose?: () => void } = $props();

const close = () => {
  if (onclose) {
    onclose();
  } else {
    pop();
  }
};

let settings = $state<SttSettings | null>(null);
let downloading = $state(false);
let dlProgress = $state<DownloadProgress | null>(null);
let modelPath = $state<string | null>(null);
let recordingsDir = $state<string | null>(null);
let effectiveTranscriptsDir = $state<string | null>(null);
let microphoneGranted = $state(false);
let screenRecordingGranted = $state(false);
let sttLoading = $state(false);
let sttError = $state<string | null>(null);

let openStt = $state(true);

let autostartEnabled = $state(false);
let autostartLoading = $state(true);

async function checkAutostart() {
  try {
    autostartEnabled = await isEnabled();
  } catch (e) {
    console.error("Failed to check autostart:", e);
  } finally {
    autostartLoading = false;
  }
}

async function toggleAutostart() {
  autostartLoading = true;
  try {
    if (autostartEnabled) {
      await disable();
      autostartEnabled = false;
    } else {
      await enable();
      autostartEnabled = true;
    }
  } catch (err) {
    console.error("Failed to toggle autostart:", err);
  } finally {
    autostartLoading = false;
  }
}

let locked = $derived(session.locked);

async function refreshPermissions() {
  screenRecordingGranted = await invoke<boolean>(
    "check_screen_recording_permission",
  );
  microphoneGranted = await invoke<boolean>("check_microphone_permission");
}

$effect(() => {
  invoke<SttSettings>("get_stt_settings").then((s) => {
    settings = s;
  });
  invoke<string>("get_local_model_path").then((p) => {
    modelPath = p;
  });
  invoke<string>("get_recordings_dir").then((d) => {
    recordingsDir = d;
  });
  invoke<string>("get_transcripts_dir").then((t) => {
    effectiveTranscriptsDir = t;
  });
  refreshPermissions();
  checkAutostart();

  const onFocus = () => {
    refreshPermissions();
    checkAutostart();
  };
  window.addEventListener("focus", onFocus);

  const unlisten = listen<DownloadProgress>("download-progress", (e) => {
    dlProgress = e.payload;
    if (e.payload.step === "done") {
      downloading = false;
      if (settings) settings = { ...settings, localReady: true };
    }
  });

  return () => {
    window.removeEventListener("focus", onFocus);
    unlisten.then((fn) => fn());
  };
});

let scrolledSection = $state<string | null>(null);
$effect(() => {
  const section = router.querystring
    ? new URLSearchParams(router.querystring).get("section")
    : null;
  if (settings && section && scrolledSection !== section) {
    scrolledSection = section;
    if (section === "stt-server") openStt = true;
    setTimeout(() => {
      const el = document.getElementById(section);
      el?.scrollIntoView({ behavior: "smooth", block: "start" });
    }, 50);
  }
});

// ── Server STT ───────────────────────────────────────────────────────────────

async function runStt(cmd: string, opts?: { attemptStart?: boolean }) {
  sttLoading = true;
  sttError = null;
  try {
    await invoke(cmd);
    await servers.refreshStt(opts);
  } catch (e) {
    sttError = String(e);
  } finally {
    sttLoading = false;
  }
}

async function handleDeleteModel() {
  await runStt("delete_local_model");
  if (!sttError) {
    await servers.refreshStt();
    close();
  }
}

async function chooseTranscriptsDir() {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Seleziona la cartella per le note Markdown",
  });
  if (selected && typeof selected === "string") {
    effectiveTranscriptsDir = await invoke<string>("set_transcripts_dir", {
      path: selected,
    });
    settings = await invoke<SttSettings>("get_stt_settings");
  }
}

async function resetTranscriptsDir() {
  effectiveTranscriptsDir = await invoke<string>("set_transcripts_dir", {
    path: null,
  });
  settings = await invoke<SttSettings>("get_stt_settings");
}

function openPermissionSettings(pane: "microphone" | "screen-recording") {
  invoke("open_permission_settings", { pane });
}

async function startDownload() {
  downloading = true;
  dlProgress = null;
  try {
    await invoke("download_local_model");
  } catch (e) {
    downloading = false;
    throw e;
  }
}
</script>

<Modal title="Impostazioni" onclose={close} maxWidth="max-w-xl">
  <div class="flex-1 overflow-y-auto p-4 sm:p-6 text-ink-primary">
    {#if settings}
      <div class="mx-auto flex w-full max-w-xl flex-col gap-3">
        <!-- 0. Avvio di sistema -->
        <AccordionSection title="Avvio di sistema" icon={Power}>
          <div class="flex items-center justify-between gap-3 px-3.5 py-3">
            <div class="flex flex-col gap-0.5">
              <span class="text-xs font-semibold text-ink-primary">
                Esegui in background all'avvio del Mac
              </span>
              <p class="m-0 text-2xs leading-relaxed text-ink-muted">
                Earwig si avvia silenziosamente nella barra dei menu in alto a destra, senza occupare il Dock.
              </p>
            </div>
            <button
              type="button"
              role="switch"
              aria-label="Esegui in background all'avvio del Mac"
              title="Esegui in background all'avvio del Mac"
              aria-checked={autostartEnabled}
              disabled={autostartLoading}
              onclick={toggleAutostart}
              class="relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none disabled:opacity-50 {autostartEnabled ? 'bg-brand-red' : 'bg-brand-cream/15'}"
            >
              <span
                class="pointer-events-none inline-block h-4 w-4 transform rounded-full bg-brand-cream shadow-sm ring-0 transition duration-200 ease-in-out {autostartEnabled ? 'translate-x-4' : 'translate-x-0'}"
              ></span>
            </button>
          </div>
        </AccordionSection>

        <!-- 1. Permessi -->
        <AccordionSection title="Permessi" icon={ShieldCheck}>
          <div class="flex flex-col divide-y divide-brand-cream/6">
            <button
              type="button"
              class="group/item flex w-full items-start justify-between gap-3 px-3.5 py-2.5 text-left transition-colors hover:bg-brand-cream/[0.04] cursor-pointer"
              onclick={() => openPermissionSettings("microphone")}
              title="Clicca per aprire le preferenze di sistema per il microfono"
            >
              <div class="flex items-start gap-3">
                <div class="mt-0.5 shrink-0">
                  <Mic
                    size={16}
                    class="text-ink-secondary transition-colors group-hover/item:text-brand-cream"
                  />
                </div>
                <div class="flex flex-col gap-0.5">
                  <span
                    class="text-xs font-semibold text-ink-primary group-hover/item:text-brand-cream"
                  >
                    Microfono
                  </span>
                  <p class="m-0 text-2xs leading-relaxed text-ink-muted">
                    Necessario per registrare la tua voce.
                  </p>
                </div>
              </div>

              <div class="flex shrink-0 items-center gap-1.5 pt-0.5">
                {#if microphoneGranted}
                  <span
                    class="inline-flex items-center gap-1 rounded-full border border-ok/25 bg-ok/10 px-2 py-0.5 text-2xs font-medium text-ok"
                  >
                    <Check size={11} /> Concesso
                  </span>
                {:else}
                  <span
                    class="inline-flex items-center gap-1 rounded-full border border-brand-cream/20 bg-brand-cream/5 px-2 py-0.5 text-2xs font-medium text-ink-muted"
                  >
                    Non concesso
                  </span>
                {/if}
              </div>
            </button>

            <button
              type="button"
              class="group/item flex w-full items-start justify-between gap-3 px-3.5 py-2.5 text-left transition-colors hover:bg-brand-cream/[0.04] cursor-pointer"
              onclick={() => openPermissionSettings("screen-recording")}
              title="Clicca per aprire le preferenze di sistema per la cattura audio sistema"
            >
              <div class="flex items-start gap-3">
                <div class="mt-0.5 shrink-0">
                  {#if screenRecordingGranted}
                    <MonitorCheck
                      size={16}
                      class="text-ink-secondary transition-colors group-hover/item:text-brand-cream"
                    />
                  {:else}
                    <MonitorX
                      size={16}
                      class="text-ink-secondary transition-colors group-hover/item:text-brand-cream"
                    />
                  {/if}
                </div>
                <div class="flex flex-col gap-0.5">
                  <span
                    class="text-xs font-semibold text-ink-primary group-hover/item:text-brand-cream"
                  >
                    Cattura audio sistema
                  </span>
                  <p class="m-0 text-2xs leading-relaxed text-ink-muted">
                    Necessaria per catturare l'audio di sistema (richiesta da ScreenCaptureKit).
                  </p>
                </div>
              </div>

              <div class="flex shrink-0 items-center gap-1.5 pt-0.5">
                {#if screenRecordingGranted}
                  <span
                    class="inline-flex items-center gap-1 rounded-full border border-ok/25 bg-ok/10 px-2 py-0.5 text-2xs font-medium text-ok"
                  >
                    <Check size={11} /> Concesso
                  </span>
                {:else}
                  <span
                    class="inline-flex items-center gap-1 rounded-full border border-brand-cream/20 bg-brand-cream/5 px-2 py-0.5 text-2xs font-medium text-ink-muted"
                  >
                    Non concesso
                  </span>
                {/if}
              </div>
            </button>
          </div>
        </AccordionSection>

        <!-- 2. Cartella registrazioni -->
        <AccordionSection title="Cartella registrazioni" icon={Folder}>
          {#if recordingsDir}
            <div class="flex items-center justify-between gap-3 px-3.5 py-2.5">
              <span class="shrink-0 text-xs text-ink-secondary">Percorso:</span>
              <button
                type="button"
                class="min-w-0 flex-1 truncate text-left font-mono text-2xs text-ink-muted transition-all duration-150 rounded-md border border-brand-cream/8 bg-brand-cream/[0.03] px-2.5 py-1.5 hover:border-brand-cream/25 hover:bg-brand-cream/10 hover:text-ink-primary active:scale-[0.99] cursor-pointer"
                onclick={() => revealItemInDir(recordingsDir!)}
                title="Apri nel Finder"
              >
                <span class="block truncate">{recordingsDir}</span>
              </button>
            </div>
          {/if}
        </AccordionSection>

        <!-- 3. Note Markdown -->
        <AccordionSection title="Note Markdown" icon={FileText}>
          <div class="flex flex-col gap-2.5 px-3.5 py-3">
            <p class="m-0 text-2xs text-ink-secondary leading-relaxed">
              Le trascrizioni vengono esportate come note <code>.md</code> con frontmatter YAML.
              Oltre a restare nella cartella della registrazione, puoi salvarle in una cartella a piacere.
            </p>

            <div class="flex items-center justify-between gap-3">
              <span class="shrink-0 text-xs text-ink-secondary">Destinazione:</span>
              <button
                type="button"
                class="min-w-0 flex-1 truncate text-left font-mono text-2xs text-ink-muted transition-all duration-150 rounded-md border border-brand-cream/8 bg-brand-cream/[0.03] px-2.5 py-1.5 hover:border-brand-cream/25 hover:bg-brand-cream/10 hover:text-ink-primary active:scale-[0.99] cursor-pointer"
                onclick={() => effectiveTranscriptsDir && revealItemInDir(effectiveTranscriptsDir)}
                title="Apri nel Finder"
              >
                <span class="block truncate">
                  {settings.transcriptsDir ?? `${effectiveTranscriptsDir ?? recordingsDir} (Predefinita)`}
                </span>
              </button>
            </div>

            <div class="flex items-center justify-between pt-1">
              {#if settings.transcriptsDir}
                <button
                  type="button"
                  class="text-2xs text-ink-faint hover:text-ink-primary underline cursor-pointer border-none bg-transparent p-0"
                  onclick={resetTranscriptsDir}
                >
                  Ripristina predefinita
                </button>
              {:else}
                <span class="text-2xs text-ink-faint">
                  Salvataggio nella cartella di ogni registrazione
                </span>
              {/if}

              <Button
                variant="ghost"
                class="h-7 px-2.5 text-2xs"
                onclick={chooseTranscriptsDir}
              >
                {settings.transcriptsDir ? "Cambia cartella" : "Scegli cartella"}
              </Button>
            </div>
          </div>
        </AccordionSection>

        <!-- 4. Server STT (Modello e Motore unificati) -->
        <AccordionSection
          id="stt-server"
          class="scroll-mt-4"
          title="Server STT"
          icon={Server}
          bind:isOpen={openStt}
        >
          <div class="flex flex-col divide-y divide-brand-cream/6">
            <!-- Riga Modello -->
            <div class="flex items-center justify-between gap-3 px-3.5 py-2.5">
              <span class="text-xs text-ink-secondary">Modello:</span>
              <div class="flex items-center gap-2">
                <span class="text-xs font-medium text-ink-primary">
                  Parakeet TDT 0.6B v3 (~670 MB)
                </span>
                {#if settings?.localReady && !downloading}
                  <Button
                    variant="icon"
                    class="size-7 text-ink-secondary hover:text-ink-primary"
                    onclick={startDownload}
                    disabled={downloading}
                    title="Scarica di nuovo"
                    aria-label="Scarica di nuovo"
                  >
                    <RotateCcw size={13} />
                  </Button>
                {/if}
              </div>
            </div>

            <!-- Riga Stato e Controlli -->
            <div class="flex items-center justify-between gap-3 px-3.5 py-2.5">
              <div class="flex items-center gap-2">
                <span class="text-xs text-ink-secondary">Stato:</span>
                <div class="flex items-center gap-1.5">
                  <span
                    class={`size-2 shrink-0 rounded-full ${
                      downloading
                        ? "bg-warn animate-blink"
                        : servers.sttStatus === "running"
                          ? "bg-ok"
                          : servers.sttStatus === "error"
                            ? "bg-danger"
                            : "bg-brand-light"
                    }`}
                  ></span>
                  <span
                    class={`text-xs font-medium ${
                      downloading
                        ? "text-warn"
                        : servers.sttStatus === "running"
                          ? "text-ok"
                          : servers.sttStatus === "error"
                            ? "text-danger"
                            : "text-ink-muted"
                    }`}
                  >
                    {#if downloading}
                      Download in corso... {dlProgress?.pct ?? 0}%
                    {:else if servers.sttStatus === "running"}
                      Attivo
                    {:else if !settings?.localReady}
                      Non scaricato
                    {:else}
                      Fermo
                    {/if}
                  </span>
                </div>
              </div>

              <!-- Toolbar controlli -->
              <div class="flex items-center gap-1">
                {#if !settings?.localReady || downloading}
                  <Button
                    variant="icon"
                    class="size-7 text-ink-secondary hover:text-ink-primary"
                    onclick={startDownload}
                    disabled={downloading}
                    title={downloading ? "Download in corso..." : "Scarica modello"}
                    aria-label={downloading ? "Download in corso..." : "Scarica modello"}
                  >
                    {#if downloading}
                      <LoaderCircle size={14} class="animate-spin text-ink-primary" />
                    {:else}
                      <Download size={13} />
                    {/if}
                  </Button>
                {:else}
                  <Button
                    variant="icon"
                    class="size-7 text-ink-secondary hover:text-ink-primary"
                    onclick={() => runStt("start_stt_server")}
                    disabled={sttLoading || locked || servers.sttStatus === "running"}
                    title="Avvia server"
                    aria-label="Avvia server"
                  >
                    <Play size={13} fill="currentColor" class="translate-x-px" />
                  </Button>

                  <Button
                    variant="icon"
                    class="size-7 text-ink-secondary hover:text-ink-primary"
                    onclick={() => runStt("restart_stt_server")}
                    disabled={sttLoading || locked}
                    title="Riavvia server"
                    aria-label="Riavvia server"
                  >
                    <RotateCcw size={13} />
                  </Button>

                  <Button
                    variant="icon"
                    class="size-7 text-ink-secondary hover:text-ink-primary"
                    onclick={() => runStt("stop_stt_server", { attemptStart: false })}
                    disabled={sttLoading || locked || servers.sttStatus === "stopped"}
                    title="Ferma server"
                    aria-label="Ferma server"
                  >
                    <Square size={13} fill="currentColor" />
                  </Button>

                  <Button
                    variant="icon"
                    class="size-7 text-danger hover:border-danger/30 hover:bg-danger/10 hover:text-danger"
                    onclick={handleDeleteModel}
                    disabled={sttLoading || locked}
                    title="Elimina modello"
                    aria-label="Elimina modello"
                  >
                    <Trash2 size={13} />
                  </Button>
                {/if}
              </div>
            </div>

            <!-- Riga Percorso -->
            {#if modelPath}
              <div class="flex items-center justify-between gap-3 px-3.5 py-2.5">
                <span class="shrink-0 text-xs text-ink-secondary">Percorso:</span>
                <button
                  type="button"
                  class="min-w-0 flex-1 truncate text-left font-mono text-2xs text-ink-muted transition-all duration-150 rounded-md border border-brand-cream/8 bg-brand-cream/[0.03] px-2.5 py-1.5 hover:border-brand-cream/25 hover:bg-brand-cream/10 hover:text-ink-primary active:scale-[0.99] cursor-pointer"
                  onclick={() =>
                    revealItemInDir(
                      modelPath!.slice(0, modelPath!.lastIndexOf("/")),
                    )}
                  title="Apri nel Finder"
                >
                  <span class="block truncate">{modelPath}</span>
                </button>
              </div>
            {/if}
          </div>

          {#if downloading && dlProgress}
            <div class="px-3.5 pb-3">
              <DownloadProgressBar progress={dlProgress} />
            </div>
          {/if}

          {#if sttError}
            <div class="px-3.5 pb-2.5">
              <p class="m-0 text-2xs text-danger">{sttError}</p>
            </div>
          {/if}
        </AccordionSection>
      </div>
    {/if}
  </div>
</Modal>
