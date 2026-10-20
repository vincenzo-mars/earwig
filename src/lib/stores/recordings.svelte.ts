import { invoke } from "@tauri-apps/api/core";
import type { RecordingEntry } from "../types";
import { session } from "./session.svelte";

let entries = $state<RecordingEntry[]>([]);
let loading = $state(false);
let loaded = false;

// Fonte unica per lista e dettaglio: senza, il dettaglio raggiunto per URL
// dovrebbe rifare `list_recordings` per conto suo e le due copie andrebbero
// tenute allineate a mano dopo ogni rilancio.
async function load(force = false): Promise<RecordingEntry[]> {
  if (loaded && !force) return entries;
  loading = true;
  try {
    entries = await invoke<RecordingEntry[]>("list_recordings");
    loaded = true;
    return entries;
  } finally {
    loading = false;
  }
}

// Se una registrazione è in fase di elaborazione (cancellazione rumore o trascrizione),
// viene inserita in cima alla lista per comparire subito come voce in caricamento.
const allEntries = $derived.by(() => {
  const item = session.processingItem;
  if (!item) return entries;
  const existing = entries.some((e) => e.name === item.name);
  if (existing) return entries;
  const pendingEntry: RecordingEntry = {
    name: item.name,
    folder_path: item.folderPath ?? item.name,
    transcript: null,
    error: null,
  };
  return [pendingEntry, ...entries];
});

function byName(name: string): RecordingEntry | undefined {
  return allEntries.find((e) => e.name === name);
}

// Rilancio della trascrizione da lista o dettaglio: riusa il lock di
// `session` (blocca REC e ogni altra riga), poi ricarica dal disco come unica
// fonte di verità. L'esito, riuscito o fallito, è già persistito da
// `transcribe_recording` (transcript.json o transcript_error.json), quindi la
// ricarica lo riflette da sola senza propagare l'errore.
async function retryTranscription(
  folderPath: string,
): Promise<RecordingEntry[]> {
  if (session.locked) return entries;
  const name = folderPath.split("/").filter(Boolean).pop() ?? "";
  session.setProcessingItem({
    name,
    folderPath,
    phase: "transcribing",
  });
  try {
    await session.runTranscription(`${folderPath}/recording.wav`);
  } catch {
    // Esito già sul disco: la load() qui sotto lo rilegge.
  } finally {
    session.setProcessingItem(null);
  }
  return load(true);
}

export const recordings = {
  get all() {
    return allEntries;
  },
  get loading() {
    return loading;
  },
  byName,
  load,
  retryTranscription,
};
