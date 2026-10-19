---
name: run-heedm
description: >
  Launch, drive, and verify the heedm app: dev run, screenshot for UI
  verification, whisper-server and llama-server lifecycle (start/health/kill/
  orphans), the record→transcribe STT smoke test, and the local LLM chat
  completions smoke test. Use when asked to run/avvia the app, screenshot the
  UI, debug whisper-server or llama-server, or verify a recording/STT/chat
  feature end-to-end before declaring it done.
---

## Prerequisites

- Rust env: `source ~/.cargo/env` (Rust lives in `~/.cargo`).
- `src-tauri/binaries/whisper-server` must exist or `tauri dev` fails with
  `resource path 'binaries/whisper-server' doesn't exist`. If missing:
  `bash scripts/build-whisper-server.sh`.
- `src-tauri/binaries/llama-server` must exist too (same failure mode, for the
  `binaries/llama-server` resource). If missing: `bash scripts/build-llama-server.sh`.
- Screenshots from the terminal need the terminal app granted
  **System Settings → Privacy & Security → Screen Recording** (one-time, manual).
  Do NOT try osascript/System Events UI scripting — assistive access is denied
  on this machine and has failed in every past session.

## Launch (dev)

```sh
source ~/.cargo/env && npm run tauri dev
```

Run in background; first compile takes minutes, later runs seconds. Ready when
the log shows the Vite server + app window opens.

## Screenshot verification

```sh
screencapture -x /tmp/heedm-shot.png
```

Then Read the PNG. Full-screen capture — no window ID needed, no assistive
access. If it produces a black/empty image, the terminal lacks the Screen
Recording permission (see prerequisites); tell the user, don't retry.

## whisper-server lifecycle

The app manages the server itself (spawn on `start_local_server`, kill on app
exit). For manual debugging:

```sh
lsof -i :8080                          # who owns the port
curl -s http://127.0.0.1:8080/health   # server alive?
pkill -f whisper-server                # kill orphans (check lsof again after)
# manual start (default model dir):
src-tauri/binaries/whisper-server \
  --model "$HOME/Library/Application Support/com.vincenzomars.heedm/models/ggml-large-v3-turbo.bin" \
  --host 127.0.0.1 --port 8080
```

Orphan gotcha: a `whisper-server` still alive after app quit may predate the
current fix — check its start time (`ps -o lstart -p <pid>`) before concluding
the kill-on-exit code is broken.

## STT smoke test (record→transcribe)

Run this before declaring any recorder/STT feature done. Generates a spoken
fixture with native tools — no stored fixture file:

```sh
say -v Alice -o /tmp/heedm-fix.aiff "questa è una prova di trascrizione"
afconvert -f WAVE -d LEI16@16000 -c 1 /tmp/heedm-fix.aiff /tmp/heedm-fix.wav
curl -s http://127.0.0.1:8080/inference \
  -F file=@/tmp/heedm-fix.wav -F language=it -F response_format=verbose_json \
  | jq -e '.text | length > 0'
```

Pass = jq exits 0 and `.text` contains something like the spoken phrase.
Known failure modes (both shipped as bugs in the past):
- `{"error":"failed to read WAV file"}` → input not mono/16kHz/16-bit PCM
- 404 → wrong endpoint; whisper.cpp exposes only `/inference`, `/load`, `/health`
  (no OpenAI-style `/v1/audio/transcriptions`)

For a full-loop check, also record from the real app (user does this part —
TCC mic/screen permissions belong to the app, not the terminal) and transcribe
the produced WAV the same way.

## llama-server lifecycle (local LLM: summary + chat)

Independent process/port from whisper-server (8081 vs 8080) — starting/stopping
one never touches the other. Model download is handled by heedm itself
(`download_llm_model`, plain `reqwest` streamed to `<model_dir>/llm-models/`),
**not** by llama-server's own `--hf-repo`/`--hf-file` downloader: its progress
bar is gated on `isatty(stdout)` in llama.cpp's `common/download.cpp` and
prints nothing under `Stdio::piped()` (see DEVLOG). `start_llm_server` always
passes `--model <local path>` and fails fast if that file isn't there yet.
The port still opens **before** the model is loaded into RAM/VRAM (`/health`
is 503 while loading, 200 once ready — a bare port check is not a readiness
check here).

```sh
lsof -i :8081                          # who owns the port
curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8081/health   # 503 loading → 200 ready
pkill -f llama-server                  # kill orphans (check lsof again after)
# manual start (pass the path of an already-downloaded GGUF):
src-tauri/binaries/llama-server \
  --model ~/Library/Application\ Support/<bundle-id>/llm-models/<org>--<repo>/<file>.gguf \
  --host 127.0.0.1 --port 8081 --alias heedm-llm --ctx-size 16384 --no-webui
```

## Chat completions smoke test

Run this before declaring any summary/chat feature done, in addition to the
STT smoke test above:

```sh
curl -s http://127.0.0.1:8081/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{"model":"heedm-llm","messages":[{"role":"user","content":"Rispondi solo con: ok"}],"stream":true}'
```

Pass = a stream of `data: {...}` SSE chunks with `choices[0].delta.content`,
ending in `data: [DONE]`. This confirms the endpoint before any TypeScript
(AI SDK, `llm.ts`) is involved — if this fails, the bug is not in the frontend.

For the full in-app path (not just the raw endpoint): open a transcribed
recording's detail view, generate a summary, ask a follow-up question, quit
and relaunch the app, confirm the summary/chat history is still there (read
from `notes.json` next to the recording, not re-generated).
