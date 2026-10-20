// ── Types ─────────────────────────────────────────────────────────────────────

export interface SttSettings {
  localReady: boolean;
  modelDir: string | null;
  recordingsDir: string | null;
  transcriptsDir: string | null;
  llmHfRepo: string;
  llmHfFile: string;
  llmSizeBytes: number;
  llmReady: boolean;
}

export interface DownloadProgress {
  step: "model" | "done";
  pct: number;
}

// Evento separato da DownloadProgress: quello ha già due listener
// indipendenti (SettingsPanel e Onboarding), e lo step "done" condiviso
// farebbe scattare per errore il codice "modello whisper pronto" su un
// download dell'LLM.
export interface LlmDownloadProgress {
  step: "llm" | "done";
  pct: number;
}

export interface TranscriptSegment {
  id: number;
  start: number;
  end: number;
  text: string;
  speaker: string | null;
}

export interface TranscriptResult {
  task: string;
  language: string;
  duration: number;
  text: string;
  segments: TranscriptSegment[];
  transcription_ms: number | null;
}

export interface RecordingEntry {
  folder_path: string;
  name: string;
  transcript: TranscriptResult | null;
  error: string | null;
}

// Stato di un server locale gestito dall'app (whisper o llm). "loading" è
// specifico dell'LLM: llama-server apre la porta prima di aver caricato il
// modello, whisper-server no, quindi whisper non lo usa mai.
export type ServerStatus =
  | "checking"
  | "starting"
  | "running"
  | "error"
  | "stopped"
  | "loading";

// whisper-server non passa mai da "loading" (apre la porta solo a modello già
// caricato, a differenza di llama-server): lo stato non esiste per l'STT.
export type SttStatus = Exclude<ServerStatus, "loading">;

// ── Note per registrazione (riassunto + chat locale) ───────────────────────────

export interface NotesSummary {
  text: string;
  key_points: string[];
  actions: string[];
  open_questions: string[];
  model: string;
  generated_at: string;
}

export type ChatRole = "user" | "assistant";

export interface ChatMessage {
  id: string;
  role: ChatRole;
  content: string;
  at: string;
}

export interface RecordingNotes {
  version: number;
  summary: NotesSummary | null;
  messages: ChatMessage[];
}

// ── Ricerca modelli Hugging Face ────────────────────────────────────────────────

export interface HfModelSummary {
  id: string;
  downloads: number;
  license: string | null;
}

export interface HfGgufFile {
  filename: string;
  size_bytes: number;
}

export interface HfModelDetail {
  gated: boolean;
  files: HfGgufFile[];
}

// ── Helpers ───────────────────────────────────────────────────────────────────

// Diarizzazione fisica canali stereo (L = microfono/Tu, R = sistema/Altri).
// Segmenti mono o ambigui usano il fallback "Voce".
export interface SpeakerInfo {
  label: string;
  color: string;
  badgeClass: string;
}

const SPEAKER_INFO: Record<string, SpeakerInfo> = {
  "0": {
    label: "Tu",
    color: "#f87171", // red-400
    badgeClass: "border-red-500/25 bg-red-500/10 text-red-400",
  },
  "1": {
    label: "Altri",
    color: "#c084fc", // purple-400
    badgeClass: "border-purple-500/25 bg-purple-500/10 text-purple-400",
  },
};

const UNKNOWN_SPEAKER: SpeakerInfo = {
  label: "Voce",
  color: "#94908a",
  badgeClass: "border-brand-cream/15 bg-brand-cream/5 text-ink-secondary",
};

export function speakerInfo(speaker: string): SpeakerInfo {
  return SPEAKER_INFO[speaker] ?? UNKNOWN_SPEAKER;
}

// Stato derivato di una entry, non persistito come tale: il backend scrive solo
// `transcript`/`error` su disco (vedi `transcribe_recording`), qui si ricava la
// terza etichetta per il badge.
export type TranscriptStatus = "transcribed" | "pending" | "failed";

export function transcriptStatus(entry: RecordingEntry): TranscriptStatus {
  if (entry.transcript) return "transcribed";
  if (entry.error) return "failed";
  return "pending";
}

const TRANSCRIPT_STATUS_INFO: Record<
  TranscriptStatus,
  { label: string; className: string }
> = {
  transcribed: {
    label: "trascritto",
    className: "border border-ok/30 bg-ok/10 text-ok",
  },
  pending: {
    label: "in attesa",
    className: "border border-brand-cream/20 bg-transparent text-ink-faint",
  },
  failed: {
    label: "fallito",
    className: "border border-danger/30 bg-danger/10 text-danger",
  },
};

export function transcriptStatusInfo(entry: RecordingEntry): {
  label: string;
  className: string;
} {
  return TRANSCRIPT_STATUS_INFO[transcriptStatus(entry)];
}

export function formatDuration(ms: number): string {
  const s = Math.floor(ms / 1000);
  const m = Math.floor(s / 60);
  const h = Math.floor(m / 60);
  return [h, m % 60, s % 60].map((v) => String(v).padStart(2, "0")).join(":");
}

export function formatSeconds(s: number): string {
  const m = Math.floor(s / 60);
  const sec = Math.floor(s % 60);
  return `${m}:${String(sec).padStart(2, "0")}`;
}

// Tempo di trascrizione: sotto il minuto mostra i secondi con un decimale
// ("12.4s"), oltre passa a "m:ss" per restare leggibile.
export function formatElapsed(ms: number): string {
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  return formatSeconds(ms / 1000);
}

// Timestamp formattato per il nome della cartella di registrazione (es. "2026-10-19 17.44.15")
// Identico al formato prodotto da chrono lato Rust.
export function formatTimestamp(d = new Date()): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}.${pad(d.getMinutes())}.${pad(d.getSeconds())}`;
}

const MONTHS_IT = [
  "Gen",
  "Feb",
  "Mar",
  "Apr",
  "Mag",
  "Giu",
  "Lug",
  "Ago",
  "Set",
  "Ott",
  "Nov",
  "Dic",
];

// Formatta il timestamp di registrazione (es. "2026-10-19 17.44.15") in forma umana leggibile
// ("Oggi, 17:44", "Ieri, 17:44" o "19 Ott, 17:44").
export function formatRecordingDate(name: string): string {
  const match = name.match(
    /^(\d{4})-(\d{2})-(\d{2})[ T](\d{2})[.:](\d{2})(?:[.:](\d{2}))?/,
  );
  if (!match) return name;

  const year = Number.parseInt(match[1], 10);
  const month = Number.parseInt(match[2], 10) - 1;
  const day = Number.parseInt(match[3], 10);
  const hours = match[4];
  const minutes = match[5];

  const recDate = new Date(year, month, day);
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const yesterday = new Date(today);
  yesterday.setDate(yesterday.getDate() - 1);

  if (recDate.getTime() === today.getTime()) {
    return `Oggi, ${hours}:${minutes}`;
  }
  if (recDate.getTime() === yesterday.getTime()) {
    return `Ieri, ${hours}:${minutes}`;
  }
  const monthName = MONTHS_IT[month] ?? "";
  if (year === now.getFullYear()) {
    return `${day} ${monthName}, ${hours}:${minutes}`;
  }
  return `${day} ${monthName} ${year}, ${hours}:${minutes}`;
}

export type ProcessingPhase = "noise_cancellation" | "transcribing";

export interface ProcessingItem {
  name: string;
  folderPath?: string;
  phase: ProcessingPhase;
}

export function groupSegments(segments: TranscriptSegment[]) {
  const groups: { speaker: string; text: string; start: number }[] = [];
  for (const seg of segments) {
    const speaker = seg.speaker ?? "unknown";
    const last = groups[groups.length - 1];
    if (last && last.speaker === speaker) {
      last.text += ` ${seg.text.trim()}`;
    } else {
      groups.push({ speaker, text: seg.text.trim(), start: seg.start });
    }
  }
  return groups;
}
