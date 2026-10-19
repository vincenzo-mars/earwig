import { createOpenAICompatible } from "@ai-sdk/openai-compatible";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { streamText } from "ai";
import {
  type ChatMessage,
  formatSeconds,
  groupSegments,
  type NotesSummary,
  type TranscriptResult,
} from "./types";

const LLM_BASE_URL = "http://127.0.0.1:8081/v1";
const LLM_MODEL_ID = "earwig-llm";

// `fetch` gira lato Rust via @tauri-apps/plugin-http invece della fetch nativa
// del webview: l'App Transport Security di macOS può bloccare richieste HTTP
// semplici da un'app pacchettizzata anche verso 127.0.0.1 (vedi tauri-apps/
// tauri#4722), e earwig non aveva finora nessuna eccezione ATS configurata.
const provider = createOpenAICompatible({
  name: "earwig-llm",
  baseURL: LLM_BASE_URL,
  fetch: tauriFetch as unknown as typeof fetch,
});

const model = provider(LLM_MODEL_ID);

// Il contesto della trascrizione supera facilmente qualche migliaio di
// caratteri: --context-shift è disattivato lato server, quindi un prompt
// troppo lungo è un errore secco, non un troncamento morbido. Il cap qui
// evita di mandare mai quell'errore al modello.
const MAX_TRANSCRIPT_CHARS = 30_000;

export interface TranscriptContext {
  text: string;
  truncated: boolean;
}

// Righe `[m:ss] IO: ...` / `[m:ss] INTERLOCUTORE: ...` quando la
// diarizzazione è disponibile (l'attribuzione per speaker è il valore
// aggiunto di un riassunto di riunione), altrimenti il testo grezzo
// (mono/import, dove `speaker` è sempre null).
export function buildTranscriptContext(
  transcript: TranscriptResult,
): TranscriptContext {
  const hasSpeakers = transcript.segments.some((s) => s.speaker != null);
  let text: string;

  if (hasSpeakers) {
    const groups = groupSegments(transcript.segments);
    text = groups
      .map((g) => {
        const label =
          g.speaker === "0" ? "IO" : g.speaker === "1" ? "INTERLOCUTORE" : "?";
        return `[${formatSeconds(g.start)}] ${label}: ${g.text}`;
      })
      .join("\n");
  } else {
    text = transcript.text;
  }

  const truncated = text.length > MAX_TRANSCRIPT_CHARS;
  return {
    text: truncated ? text.slice(0, MAX_TRANSCRIPT_CHARS) : text,
    truncated,
  };
}

const SUMMARY_INSTRUCTIONS = `Sei un assistente che riassume la trascrizione di una riunione o chiamata.
Rispondi SOLO con queste quattro sezioni, in questo ordine esatto, in italiano:

RIASSUNTO: <2-4 frasi che inquadrano di cosa si è parlato>

PUNTI CHIAVE:
- <decisione o fatto rilevante>

AZIONI:
- <cosa va fatto, con il responsabile se identificabile dal parlante (IO/INTERLOCUTORE)>

DOMANDE APERTE:
- <cosa è rimasto irrisolto o da chiarire>

Se una sezione non ha contenuto, scrivi "Nessuna." sotto l'intestazione invece di ometterla.
Usa solo informazioni presenti nella trascrizione: non inventare nomi, numeri o date.`;

export function streamSummary(
  context: TranscriptContext,
  signal?: AbortSignal,
) {
  return streamText({
    model,
    instructions: SUMMARY_INSTRUCTIONS,
    messages: [{ role: "user", content: context.text }],
    abortSignal: signal,
  });
}

const CHAT_INSTRUCTIONS_PREFIX = `Sei un assistente che risponde a domande su una trascrizione di una riunione o chiamata.
Rispondi solo usando quanto presente nella trascrizione qui sotto. Se qualcosa non c'è, dillo esplicitamente
invece di inventare. Non inventare mai nomi, numeri o date. Sii conciso. Rispondi in italiano.

TRASCRIZIONE:
`;

// Il contesto della trascrizione va nelle instructions (non in un primo
// messaggio sintetico) così il prefisso del prompt resta identico turno dopo
// turno: llama.cpp può riusare la KV cache dello slot invece di riprocessare
// migliaia di token ad ogni domanda.
export function streamChatReply(
  context: TranscriptContext,
  history: ChatMessage[],
  question: string,
  signal?: AbortSignal,
) {
  return streamText({
    model,
    instructions: CHAT_INSTRUCTIONS_PREFIX + context.text,
    messages: [
      ...history.map((m) => ({ role: m.role, content: m.content })),
      { role: "user" as const, content: question },
    ],
    abortSignal: signal,
  });
}

const SECTION_HEADERS = [
  { key: "text", pattern: /^riassunto:?/i },
  { key: "key_points", pattern: /^punti chiave:?/i },
  { key: "actions", pattern: /^azioni:?/i },
  { key: "open_questions", pattern: /^domande aperte:?/i },
] as const;

const BULLET_PATTERN = /^\s*[-•*]\s+(.*)$/;

// Tollerante a un formato disatteso (succede coi modelli scelti dal campo
// di ricerca libero, non curati da noi): se non trova le intestazioni, il
// testo grezzo finisce tutto in `text` e le liste restano vuote, mai un
// errore bloccante.
export function parseSummary(
  raw: string,
): Omit<NotesSummary, "model" | "generated_at"> {
  const lines = raw.split("\n");
  const sections: Record<string, string[]> = {
    text: [],
    key_points: [],
    actions: [],
    open_questions: [],
  };
  let current: string = "text";
  let matchedAny = false;

  for (const line of lines) {
    const header = SECTION_HEADERS.find((h) => h.pattern.test(line.trim()));
    if (header) {
      current = header.key;
      matchedAny = true;
      const rest = line.trim().replace(header.pattern, "").trim();
      if (rest) sections[current].push(rest);
      continue;
    }
    if (!line.trim()) continue;
    sections[current].push(line.trim());
  }

  if (!matchedAny) {
    return {
      text: raw.trim(),
      key_points: [],
      actions: [],
      open_questions: [],
    };
  }

  const toBullets = (linesForSection: string[]) =>
    linesForSection
      .map((l) => l.match(BULLET_PATTERN)?.[1] ?? l)
      .filter((l) => l && !/^nessuna\.?$/i.test(l));

  return {
    text: sections.text.join(" ").trim(),
    key_points: toBullets(sections.key_points),
    actions: toBullets(sections.actions),
    open_questions: toBullets(sections.open_questions),
  };
}
