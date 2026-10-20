import { invoke } from "@tauri-apps/api/core";
import { formatTimestamp, type ProcessingItem } from "../types";
import { servers } from "./servers.svelte";

let isRecording = $state(false);
let isStarting = $state(false);
let isStopping = $state(false);
let isTranscribing = $state(false);
let processingItem = $state<ProcessingItem | null>(null);
let durationMs = $state(0);
let transcribeMs = $state(0);
let error = $state<string | null>(null);

let durationTimer: ReturnType<typeof setInterval> | undefined;
let transcribeTimer: ReturnType<typeof setInterval> | undefined;

// Il frontend è l'unico a poter avviare registrazione e trascrizione, quindi
// l'istante di inizio lo conosce già: nessun bisogno di chiedere al backend
// via IPC ogni mezzo secondo.
function startTimer(
  tick: (elapsed: number) => void,
  everyMs: number,
): ReturnType<typeof setInterval> {
  const start = Date.now();
  tick(0);
  return setInterval(() => tick(Date.now() - start), everyMs);
}

function stopTimer(id: ReturnType<typeof setInterval> | undefined) {
  if (id) clearInterval(id);
  return undefined;
}

// Gate per registrazione e import: senza server whisper attivo e modello sul
// disco, `transcribe_recording` non può funzionare. Lo stesso controllo è
// applicato anche lato backend (start_recording/import_audio_file), qui serve
// solo a riflettersi in UI.
const canRecord = () => servers.sttStatus === "running" && servers.modelReady;

const recordGateReason = () => {
  if (canRecord()) return null;
  if (!servers.modelReady)
    return "Scarica il modello dalle impostazioni per registrare.";
  return "Avvia il server whisper dalle impostazioni per registrare.";
};

// Lock condiviso da REC, import e rilancio: finché è attivo nessun altro
// flusso di trascrizione può partire. Esposto anche a chi mostra righe
// disabilitate (lista e dettaglio).
const locked = () => isRecording || isStarting || isStopping || isTranscribing;

// Primitiva usata sia dal flusso REC/import sia dal rilancio in
// `recordings.svelte.ts`: tiene il lock e il cronometro in un posto solo.
// Gli errori non vengono ingoiati: `transcribe_recording` li persiste su
// disco (transcript_error.json) e il chiamante decide se mostrarli.
async function runTranscription(path: string): Promise<void> {
  isTranscribing = true;
  transcribeTimer = startTimer((ms) => {
    transcribeMs = ms;
  }, 100);
  try {
    await invoke("transcribe_recording", { path });
  } finally {
    isTranscribing = false;
    transcribeTimer = stopTimer(transcribeTimer);
  }
}

async function startRecording(): Promise<void> {
  error = null;
  if (locked() || !canRecord()) return;
  isStarting = true;
  try {
    await invoke("start_recording");
    isRecording = true;
    durationTimer = startTimer((ms) => {
      durationMs = ms;
    }, 500);
  } catch (e) {
    const err = String(e);
    if (
      err.includes("Already recording") ||
      err.includes("Registrazione già in corso")
    ) {
      isRecording = true;
    } else {
      error = err;
    }
  } finally {
    isStarting = false;
  }
}

async function stopRecording(): Promise<void> {
  if (!isRecording || isStopping) return;
  const name = formatTimestamp();
  isStopping = true;
  isRecording = false;
  durationTimer = stopTimer(durationTimer);
  error = null;
  processingItem = {
    name,
    phase: "noise_cancellation",
  };
  try {
    const path = await invoke<string>("stop_recording", { timestamp: name });
    isStopping = false;
    if (!path) {
      processingItem = null;
      return;
    }
    processingItem = {
      name,
      folderPath: path,
      phase: "transcribing",
    };
    await runTranscription(path);
  } catch (e) {
    isStopping = false;
    processingItem = null;
    const err = String(e);
    if (
      !err.includes("Not recording") &&
      !err.includes("Nessuna registrazione in corso")
    ) {
      error = err;
    }
  } finally {
    processingItem = null;
  }
}

// Ritorna true se un file è stato davvero importato e trascritto: il
// chiamante lo usa per decidere se navigare alla lista. False copre anche
// l'annullamento del dialog, che non è un errore.
async function importFile(): Promise<boolean> {
  error = null;
  if (locked()) return false;
  if (!canRecord()) {
    error = recordGateReason();
    return false;
  }
  isStarting = true;
  let path: string | null = null;
  try {
    path = await invoke<string | null>("import_audio_file");
  } catch (e) {
    error = String(e);
    return false;
  } finally {
    isStarting = false;
  }
  if (!path) return false;
  const name = path.split("/").filter(Boolean).slice(-2, -1)[0] ?? "";
  processingItem = {
    name,
    folderPath: path,
    phase: "transcribing",
  };
  try {
    await runTranscription(path);
    return true;
  } finally {
    processingItem = null;
  }
}

export const session = {
  get isRecording() {
    return isRecording;
  },
  get isStarting() {
    return isStarting;
  },
  get isStopping() {
    return isStopping;
  },
  get isTranscribing() {
    return isTranscribing;
  },
  get processingItem() {
    return processingItem;
  },
  get durationMs() {
    return durationMs;
  },
  get transcribeMs() {
    return transcribeMs;
  },
  get error() {
    return error;
  },
  get locked() {
    return locked();
  },
  get canRecord() {
    return canRecord();
  },
  get recordGateReason() {
    return recordGateReason();
  },
  setProcessingItem(item: ProcessingItem | null) {
    processingItem = item;
  },
  startRecording,
  stopRecording,
  importFile,
  runTranscription,
};
