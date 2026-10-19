<script lang="ts">
import { Folder, Mic, MonitorCheck, MonitorX, Save } from "@lucide/svelte";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { pop } from "svelte-spa-router";
import Button from "./Button.svelte";
import DownloadProgressBar from "./DownloadProgressBar.svelte";
import PageHeader from "./PageHeader.svelte";
import ServerControls from "./ServerControls.svelte";
import { servers } from "./stores/servers.svelte";
import { session } from "./stores/session.svelte";
import type {
  DownloadProgress,
  HfGgufFile,
  HfModelDetail,
  HfModelSummary,
  LlmDownloadProgress,
  SttSettings,
} from "./types";

// `pop` invece di una rotta fissa: le impostazioni si aprono da qualsiasi
// schermata (il bottone dell'indicatore è sempre visibile) e chiudendole si
// deve tornare da dove si è arrivati, non sempre alla registrazione.
const close = () => pop();

let settings = $state<SttSettings | null>(null);
let downloading = $state(false);
let dlProgress = $state<DownloadProgress | null>(null);
let llmDownloading = $state(false);
let llmDlProgress = $state<LlmDownloadProgress | null>(null);
let modelPath = $state<string | null>(null);
let recordingsDir = $state<string | null>(null);
let screenRecordingGranted = $state(false);
let sttLoading = $state(false);
let sttError = $state<string | null>(null);
let llmLoading = $state(false);
let llmError = $state<string | null>(null);

let systemRamGb = $state<number | null>(null);
let hfQuery = $state("");
let hfSearching = $state(false);
let hfSearched = $state(false);
let hfResults = $state<HfModelSummary[]>([]);
let expandedRepo = $state<string | null>(null);
let hfFilesLoading = $state(false);
let hfFilesGated = $state(false);
let hfFiles = $state<HfGgufFile[]>([]);
let searchDebounce: ReturnType<typeof setTimeout> | undefined;

let locked = $derived(session.locked);

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
  refreshScreenRecordingStatus();
  invoke<number>("get_system_memory_gb").then((gb) => {
    systemRamGb = gb;
  });

  const unlisten = listen<DownloadProgress>("download-progress", (e) => {
    dlProgress = e.payload;
    if (e.payload.step === "done") {
      downloading = false;
      if (settings) settings = { ...settings, localReady: true };
    }
  });

  const unlistenLlm = listen<LlmDownloadProgress>(
    "llm-download-progress",
    async (e) => {
      llmDlProgress = e.payload;
      if (e.payload.step === "done") {
        llmDownloading = false;
        // Ricarica da disco invece di settare llmReady in ottimismo: la
        // verità è sempre il file, stesso motivo per cui get_stt_settings
        // la ricalcola lato Rust invece di fidarsi del valore persistito.
        settings = await invoke<SttSettings>("get_stt_settings");
        await servers.refreshLlm({ attemptStart: false });
      }
    },
  );

  return () => {
    unlisten.then((fn) => fn());
    unlistenLlm.then((fn) => fn());
  };
});

async function save() {
  if (!settings) return;
  await invoke("save_stt_settings", { settings });
  await servers.refreshStt();
  close();
}

// ── Server STT / LLM ──────────────────────────────────────────────────────────
// Tutti i comandi server hanno la stessa forma: busy flag, invoke, refresh
// dello stato in App, errore mostrato nel box di stato.

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

async function runLlm(cmd: string) {
  llmLoading = true;
  llmError = null;
  try {
    await invoke(cmd);
    await servers.refreshLlm({ attemptStart: false });
  } catch (e) {
    llmError = String(e);
  } finally {
    llmLoading = false;
  }
}

async function handleDeleteModel() {
  await runStt("delete_local_model");
  if (!sttError) {
    await servers.refreshStt();
    close();
  }
}

// ── Ricerca modelli Hugging Face ────────────────────────────────────────────────

// Lazy: la prima fetch parte al primo focus del campo, non all'apertura del
// pannello — chi apre le impostazioni per i permessi non deve costare una
// chiamata a Hugging Face.
function initSearch() {
  if (!hfSearched && !hfSearching) runSearch("instruct");
}

function scheduleSearch() {
  clearTimeout(searchDebounce);
  searchDebounce = setTimeout(
    () => runSearch(hfQuery.trim() || "instruct"),
    400,
  );
}

async function runSearch(query: string) {
  hfSearching = true;
  hfSearched = true;
  llmError = null;
  try {
    hfResults = await invoke<HfModelSummary[]>("search_hf_models", { query });
  } catch (e) {
    llmError = String(e);
  } finally {
    hfSearching = false;
  }
}

async function toggleRepo(id: string) {
  if (expandedRepo === id) {
    expandedRepo = null;
    return;
  }
  expandedRepo = id;
  hfFiles = [];
  hfFilesGated = false;
  hfFilesLoading = true;
  try {
    const detail = await invoke<HfModelDetail>("get_hf_model_files", {
      repoId: id,
    });
    hfFilesGated = detail.gated;
    hfFiles = [...detail.files].sort((a, b) => a.size_bytes - b.size_bytes);
  } catch (e) {
    llmError = String(e);
  } finally {
    hfFilesLoading = false;
  }
}

async function selectModel(repo: string, file: HfGgufFile) {
  if (!settings) return;
  await invoke("set_llm_model", {
    repo,
    file: file.filename,
    sizeBytes: file.size_bytes,
  });
  settings = {
    ...settings,
    llmHfRepo: repo,
    llmHfFile: file.filename,
    llmSizeBytes: file.size_bytes,
    llmReady: false,
  };
  // Il nuovo modello non è ancora sul disco: App deve saperlo, altrimenti il
  // CTA "Avvia il server LLM" del dettaglio resta abilitato su un modello
  // mancante.
  await servers.refreshLlm({ attemptStart: false });
}

async function startLlmDownload() {
  llmDownloading = true;
  llmDlProgress = null;
  try {
    await invoke("download_llm_model");
  } catch (e) {
    llmDownloading = false;
    llmError = String(e);
  }
}

function formatGb(bytes: number): string {
  return `${(bytes / 1024 ** 3).toFixed(2)} GB`;
}

// Euristica, mai bloccante: solo un suggerimento visivo, l'utente può
// comunque scegliere un file "pesante" se la sua macchina ce la fa.
function ramBadge(
  sizeBytes: number,
): { label: string; className: string } | null {
  if (systemRamGb == null) return null;
  const ratio = sizeBytes / 1024 ** 3 / systemRamGb;
  if (ratio < 0.35)
    return { label: "Consigliato per la tua RAM", className: "text-green-400" };
  if (ratio < 0.6)
    return { label: "Ok per la tua RAM", className: "text-amber-400" };
  return { label: "Pesante per la tua RAM", className: "text-red-400" };
}

async function refreshScreenRecordingStatus() {
  screenRecordingGranted = await invoke<boolean>(
    "check_screen_recording_permission",
  );
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

{#snippet pathBox(label: string, value: string)}
  <div
    class="flex flex-col gap-1 rounded-lg border border-brand-cream/10 bg-brand-dark/50 px-2.5 py-2"
  >
    <span
      class="text-2xs font-semibold tracking-wider text-ink-faint uppercase"
      >{label}</span
    >
    <p class="m-0 font-mono text-2xs break-all text-ink-secondary">
      {value}
    </p>
  </div>
{/snippet}

{#snippet actions()}
  <Button variant="icon" class="-mr-2" title="Salva" aria-label="Salva impostazioni" onclick={save}>
    <Save size={16} />
  </Button>
{/snippet}

<div class="flex h-full w-full flex-col">
  <PageHeader title="Impostazioni" onBack={close} actions={settings ? actions : undefined} />

  <div class="flex-1 overflow-y-auto px-6 pt-5 pb-18 text-ink-primary">
    {#if settings}
      <!-- Le due colonne (core e LLM) si allargano con la finestra fino a un
           tetto: oltre, si stirerebbero lasciando una voragine in mezzo. Una
           terza colonna non c'è, i gruppi sono due. -->
      <div
        class="mx-auto grid w-full max-w-400 grid-cols-1 gap-x-8 gap-y-5 md:grid-cols-2 md:items-start"
      >
      <div class="flex flex-col gap-5">
        <div class="flex flex-col gap-3">
          <span class="text-xs font-semibold text-ink-primary"
            >Permessi</span
          >

          <div class="flex flex-col">
            <button
              type="button"
              class="flex items-start gap-2.5 rounded-lg px-2 py-1.5 text-left transition hover:bg-brand-dark/50 cursor-pointer"
              onclick={() => openPermissionSettings("microphone")}
            >
              <Mic size={16} class="mt-0.5 shrink-0 text-ink-secondary" />
              <div class="flex flex-col gap-0.5">
                <span class="text-xs font-medium text-ink-primary"
                  >Microfono</span
                >
                <p
                  class="m-0 text-2xs leading-relaxed text-ink-muted"
                >
                  Necessario per registrare la tua voce.
                </p>
              </div>
            </button>

            <button
              type="button"
              class="flex items-start gap-2.5 rounded-lg px-2 py-1.5 text-left transition hover:bg-brand-dark/50 cursor-pointer"
              onclick={() => openPermissionSettings("screen-recording")}
            >
              {#if screenRecordingGranted}
                <MonitorCheck
                  size={16}
                  class="mt-0.5 shrink-0 text-green-500"
                />
              {:else}
                <MonitorX
                  size={16}
                  class="mt-0.5 shrink-0 text-ink-secondary"
                />
              {/if}
              <div class="flex flex-col gap-0.5">
                <span class="text-xs font-medium text-ink-primary"
                  >Cattura audio sistema</span
                >
                <p
                  class="m-0 text-2xs leading-relaxed text-ink-muted"
                >
                  Necessaria per catturare l'audio di sistema (richiesta da
                  ScreenCaptureKit).
                </p>
              </div>
            </button>
          </div>
        </div>

        <div class="flex flex-col gap-1.5">
          <div class="flex items-center justify-between">
            <span class="text-xs font-semibold text-ink-primary"
              >Modello Whisper</span
            >

            {#if modelPath}
              <button
                class="p-2 cursor-pointer font-semibold text-brand-light transition hover:text-ink-primary"
                onclick={() => revealItemInDir(modelPath!.slice(0, modelPath!.lastIndexOf("/")))}><Folder size={20} /></button
              >
            {/if}
          </div>

          {#if settings?.localReady}
            <p class="m-0 text-xs font-medium text-green-400">
              Modello installato e pronto.
            </p>
          {:else}
            <p class="m-0 text-xs leading-relaxed text-ink-secondary">
              Scarica il modello Whisper large-v3-turbo (~1.5 GB). Necessario
              solo al primo avvio.
            </p>
          {/if}

          {#if modelPath}
            {@render pathBox("Percorso", modelPath)}
          {/if}

          {#if downloading && dlProgress}
            <DownloadProgressBar progress={dlProgress} />
          {/if}

          <Button
            variant="solid"
            class="px-4 py-2.5 text-sm"
            onclick={startDownload}
            disabled={downloading}
          >
            {downloading
              ? "Download in corso..."
              : settings?.localReady
                ? "Scarica di nuovo"
                : "Scarica"}
          </Button>
        </div>

        <ServerControls
          title="Server STT"
          status={servers.sttStatus}
          error={sttError}
          loading={sttLoading}
          {locked}
          dangerLabel="Elimina modello"
          onStart={() => runStt("start_stt_server")}
          onRestart={() => runStt("restart_stt_server")}
          onStop={() => runStt("stop_stt_server", { attemptStart: false })}
          onDanger={handleDeleteModel}
        />

        <div class="flex flex-col gap-1.5">
          <div class="flex items-center justify-between">
            <span class="text-xs font-semibold text-ink-primary"
              >Cartella registrazioni</span
            >

            {#if recordingsDir}
              <button
                class="p-2 cursor-pointer font-semibold text-brand-light transition hover:text-ink-primary"
                onclick={() => revealItemInDir(recordingsDir!)}><Folder size={20} /></button
              >
            {/if}
          </div>

          {#if recordingsDir}
            {@render pathBox("Percorso", recordingsDir)}
          {/if}
        </div>
      </div>

      <div class="flex flex-col gap-5">
        <div class="flex flex-col gap-1.5">
          <span class="text-xs font-semibold text-ink-primary">Modello LLM</span>
          <p class="m-0 text-2xs leading-relaxed text-ink-muted">
            Per il riassunto e la chat sulle trascrizioni. Opzionale: nulla si blocca se non lo scarichi.
          </p>

          {#if systemRamGb != null}
            <p class="m-0 text-2xs text-ink-faint">RAM rilevata: {systemRamGb.toFixed(1)} GB</p>
          {/if}

          {#if settings?.llmHfRepo && settings?.llmHfFile}
            {@render pathBox(
              "Selezionato",
              `${settings.llmHfRepo} · ${settings.llmHfFile}`,
            )}

            {#if llmDownloading && llmDlProgress}
              <DownloadProgressBar progress={llmDlProgress} />
            {/if}

            <Button
              variant="solid"
              class="px-4 py-2.5 text-sm"
              onclick={startLlmDownload}
              disabled={llmDownloading}
            >
              {llmDownloading
                ? "Download in corso..."
                : settings?.llmReady
                  ? `Scarica di nuovo (${formatGb(settings.llmSizeBytes)})`
                  : `Scarica modello (${formatGb(settings.llmSizeBytes)})`}
            </Button>
          {/if}

          <input
            type="text"
            class="rounded-lg border border-brand-cream/15 bg-brand-dark/50 px-2.5 py-1.5 text-xs text-ink-primary placeholder:text-ink-faint focus:outline-none"
            placeholder="Cerca un modello GGUF su Hugging Face..."
            bind:value={hfQuery}
            onfocus={initSearch}
            oninput={scheduleSearch}
          />

          <div class="flex max-h-[46vh] flex-col gap-1 overflow-y-auto">
            {#if hfSearching}
              <p class="m-0 text-xs text-ink-muted">Ricerca in corso...</p>
            {:else if !hfSearched}
              <p class="m-0 text-xs text-ink-muted">
                Clicca sul campo di ricerca per vedere i modelli più scaricati.
              </p>
            {:else if hfResults.length === 0}
              <p class="m-0 text-xs text-ink-muted">Nessun risultato.</p>
            {:else}
              {#each hfResults as result (result.id)}
                <div class="rounded-lg border border-brand-cream/10 bg-brand-dark/50 px-2.5 py-1.5">
                  <button
                    type="button"
                    class="flex w-full cursor-pointer items-center justify-between gap-2 border-none bg-transparent p-0 text-left"
                    onclick={() => toggleRepo(result.id)}
                  >
                    <span class="truncate text-xs font-medium text-ink-primary">{result.id}</span>
                    <span class="shrink-0 text-2xs text-ink-faint">
                      {result.license ?? "?"} · {result.downloads.toLocaleString()} dl
                    </span>
                  </button>

                  {#if expandedRepo === result.id}
                    <div class="mt-1.5 flex flex-col gap-1 border-t border-brand-cream/10 pt-1.5">
                      {#if hfFilesLoading}
                        <p class="m-0 text-2xs text-ink-muted">Carico i file...</p>
                      {:else if hfFilesGated}
                        <p class="m-0 text-2xs text-amber-400">
                          🔒 Richiede autenticazione Hugging Face: non supportato.
                        </p>
                      {:else if hfFiles.length === 0}
                        <p class="m-0 text-2xs text-ink-muted">Nessun file GGUF trovato.</p>
                      {:else}
                        {#each hfFiles as file (file.filename)}
                          {@const badge = ramBadge(file.size_bytes)}
                          <button
                            type="button"
                            class="flex cursor-pointer items-center justify-between gap-2 rounded-md border-none bg-transparent px-1.5 py-1 text-left transition hover:bg-brand-cream/5"
                            onclick={() => selectModel(result.id, file)}
                          >
                            <span class="truncate text-2xs text-ink-secondary">{file.filename}</span>
                            <span class="shrink-0 text-right text-2xs">
                              <span class="text-ink-muted">{formatGb(file.size_bytes)}</span>
                              {#if badge}
                                <span class={`ml-1 ${badge.className}`}>· {badge.label}</span>
                              {/if}
                            </span>
                          </button>
                        {/each}
                      {/if}
                    </div>
                  {/if}
                </div>
              {/each}
            {/if}
          </div>
        </div>

        <ServerControls
          title="Server LLM"
          status={servers.llmStatus}
          error={llmError}
          hint={settings?.llmReady ? null : "Scarica prima il modello."}
          loading={llmLoading}
          {locked}
          canStart={settings?.llmReady ?? false}
          dangerLabel="Elimina modelli scaricati"
          onStart={() => runLlm("start_llm_server")}
          onRestart={() => runLlm("restart_llm_server")}
          onStop={() => runLlm("stop_llm_server")}
          onDanger={() => runLlm("clear_llm_cache")}
        />
      </div>
      </div>
    {/if}
  </div>
</div>
