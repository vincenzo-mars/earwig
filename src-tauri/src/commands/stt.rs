//! Motore di trascrizione locale in-process con NVIDIA Parakeet TDT 0.6B v3.
//! Non richiede server esterni né porte aperte: l'inferenza gira direttamente
//! in-process su ONNX Runtime con accelerazione hardware su macOS.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use parakeet_rs::{ExecutionConfig, ParakeetTDT, TimestampMode, Transcriber};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::download::download_to_file;
use super::{get_stt_settings, is_stt_model_ready, local_parakeet_dir, save_stt_settings};

const PARAKEET_BASE_URL: &str =
    "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main";

// ── In-Process Engine State ───────────────────────────────────────────────────

pub struct SttEngineState(pub Mutex<Option<ParakeetTDT>>);

impl SttEngineState {
    pub fn new() -> Self {
        Self(Mutex::new(None))
    }
}

/// Inizializza o recupera l'istanza in-memory di Parakeet TDT.
fn get_or_load_model<'a>(
    app: &AppHandle,
    state: &'a SttEngineState,
) -> Result<std::sync::MutexGuard<'a, Option<ParakeetTDT>>, String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    if guard.is_none() {
        let settings = tauri::async_runtime::block_on(get_stt_settings(app.clone()));
        let dir = local_parakeet_dir(app, &settings);
        if !is_stt_model_ready(app, &settings) {
            return Err("Modello Parakeet TDT v3 non scaricato".to_string());
        }

        let env = parakeet_rs::ort::init()
            .build()
            .map_err(|e| format!("Inizializzazione ONNX runtime fallita: {e}"))?;
        let config = ExecutionConfig::new().with_environment(env);
        let model = ParakeetTDT::from_pretrained(&dir, Some(config))
            .map_err(|e| format!("Caricamento Parakeet TDT fallito: {e}"))?;
        *guard = Some(model);
    }
    Ok(guard)
}

// ── Download ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn download_local_model(app: AppHandle) -> Result<(), String> {
    let settings = get_stt_settings(app.clone()).await;
    let parakeet_dir = local_parakeet_dir(&app, &settings);
    tokio::fs::create_dir_all(&parakeet_dir)
        .await
        .map_err(|e| e.to_string())?;

    let vocab_path = parakeet_dir.join("vocab.txt");
    let decoder_path = parakeet_dir.join("decoder_joint-model.int8.onnx");
    let encoder_path = parakeet_dir.join("encoder-model.int8.onnx");

    // 1. vocab.txt (~94 KB)
    if !vocab_path.exists() {
        download_to_file(
            &app,
            &format!("{PARAKEET_BASE_URL}/vocab.txt"),
            &vocab_path,
            "download-progress",
            "vocab",
        )
        .await?;
    }

    // 2. decoder_joint-model.int8.onnx (~18.2 MB)
    if !decoder_path.exists() {
        download_to_file(
            &app,
            &format!("{PARAKEET_BASE_URL}/decoder_joint-model.int8.onnx"),
            &decoder_path,
            "download-progress",
            "decoder",
        )
        .await?;
    }

    // 3. encoder-model.int8.onnx (~652 MB - download principale con progresso)
    if !encoder_path.exists() {
        download_to_file(
            &app,
            &format!("{PARAKEET_BASE_URL}/encoder-model.int8.onnx"),
            &encoder_path,
            "download-progress",
            "model",
        )
        .await?;
    }

    let mut settings = get_stt_settings(app.clone()).await;
    settings.local_ready = true;
    save_stt_settings(app, settings).await
}

// ── Ciclo di vita motore STT ──────────────────────────────────────────────────

#[tauri::command]
pub async fn check_stt_server(app: AppHandle) -> String {
    let settings = get_stt_settings(app.clone()).await;
    if is_stt_model_ready(&app, &settings) {
        "running".to_string()
    } else {
        "stopped".to_string()
    }
}

#[tauri::command]
pub async fn start_stt_server(app: AppHandle) -> Result<(), String> {
    let app_handle = app.clone();
    tokio::task::spawn_blocking(move || {
        let state = app_handle.state::<SttEngineState>();
        drop(get_or_load_model(&app_handle, &state)?);
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(())
}

#[tauri::command]
pub async fn stop_stt_server(app: AppHandle) -> Result<(), String> {
    let state = app.state::<SttEngineState>();
    if let Ok(mut guard) = state.0.lock() {
        *guard = None;
    }
    Ok(())
}

#[tauri::command]
pub async fn restart_stt_server(app: AppHandle) -> Result<(), String> {
    stop_stt_server(app.clone()).await?;
    start_stt_server(app).await
}

#[tauri::command]
pub async fn delete_local_model(app: AppHandle) -> Result<(), String> {
    stop_stt_server(app.clone()).await?;
    let settings = get_stt_settings(app.clone()).await;
    let dir = local_parakeet_dir(&app, &settings);
    if dir.exists() {
        tokio::fs::remove_dir_all(&dir)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Trascrizione ──────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TranscriptSegment {
    pub id: u32,
    pub start: f64,
    pub end: f64,
    pub text: String,
    pub speaker: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TranscriptResult {
    pub task: String,
    pub language: String,
    pub duration: f64,
    pub text: String,
    pub segments: Vec<TranscriptSegment>,
    #[serde(default)]
    pub transcription_ms: Option<u64>,
}

const TRANSCRIPT_ERROR_FILE: &str = "transcript_error.json";

#[derive(Serialize, Deserialize, Debug)]
pub struct TranscriptError {
    pub message: String,
}

/// Trascrive un canale audio mono a 16 kHz suddividendolo in blocchi da 120 secondi
/// per garantire stabilità della memoria anche su registrazioni prolungate.
fn transcribe_channel(
    model: &mut ParakeetTDT,
    samples: Vec<f32>,
    speaker: Option<String>,
) -> Result<Vec<TranscriptSegment>, String> {
    if samples.is_empty() {
        return Ok(Vec::new());
    }

    const CHUNK_LEN: usize = 120 * 16_000;
    let mut segments = Vec::new();

    for (chunk_idx, chunk) in samples.chunks(CHUNK_LEN).enumerate() {
        let chunk_offset_sec = (chunk_idx * CHUNK_LEN) as f64 / 16_000.0;

        // Skip chunk puramente silenziosi (senza alcun segnale audio significativo)
        let peak = chunk.iter().fold(0.0f32, |acc, &s| acc.max(s.abs()));
        if peak < 0.001 {
            continue;
        }

        let res = model
            .transcribe_samples(
                chunk.to_vec(),
                16000,
                1,
                Some(TimestampMode::Sentences),
            )
            .map_err(|e| format!("Errore inferenza Parakeet TDT: {e}"))?;

        for token in res.tokens {
            let text = token.text.trim().to_string();
            if !text.is_empty() {
                segments.push(TranscriptSegment {
                    id: 0,
                    start: ((chunk_offset_sec + token.start as f64) * 100.0).round() / 100.0,
                    end: ((chunk_offset_sec + token.end as f64) * 100.0).round() / 100.0,
                    text,
                    speaker: speaker.clone(),
                });
            }
        }
    }

    Ok(segments)
}

pub fn build_transcript_markdown(
    entry_name: &str,
    wav_path: &Path,
    result: &TranscriptResult,
) -> String {
    let total_secs = result.duration.round() as u64;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    let duration_str = format!("{mins}:{secs:02}");

    let mut md = String::new();
    md.push_str("---\n");
    md.push_str(&format!("title: \"{entry_name}\"\n"));
    md.push_str(&format!("duration: \"{duration_str}\"\n"));
    md.push_str("tags:\n  - trascrizione\n  - earwig\n");
    md.push_str(&format!("source: \"{}\"\n", wav_path.to_string_lossy()));
    md.push_str("---\n\n");

    md.push_str(&format!("# {entry_name}\n\n"));
    md.push_str(&format!(
        "[Ascolta registrazione audio](file://{})\n\n",
        wav_path.to_string_lossy()
    ));
    md.push_str("## Trascrizione\n\n");

    let mut groups: Vec<(String, String)> = Vec::new();
    for seg in &result.segments {
        let speaker = seg.speaker.as_deref().unwrap_or("unknown");
        let label = match speaker {
            "0" => "Tu",
            "1" => "Altri",
            _ => "Voce",
        };
        let text = seg.text.trim();
        if text.is_empty() {
            continue;
        }

        if let Some((last_label, last_text)) = groups.last_mut() {
            if last_label == label {
                last_text.push(' ');
                last_text.push_str(text);
                continue;
            }
        }
        groups.push((label.to_string(), text.to_string()));
    }

    if groups.is_empty() {
        md.push_str(&result.text);
    } else {
        for (speaker, text) in groups {
            md.push_str(&format!("**{speaker}:** {text}\n\n"));
        }
    }

    md
}

async fn run_transcription(app: &AppHandle, path: &str) -> Result<TranscriptResult, String> {
    let started = std::time::Instant::now();
    let app_clone = app.clone();
    let path_str = path.to_string();
    let settings = get_stt_settings(app.clone()).await;

    let result = tokio::task::spawn_blocking(move || {
        let state = app_clone.state::<SttEngineState>();
        let mut model_guard = get_or_load_model(&app_clone, &state)?;
        let model = model_guard
            .as_mut()
            .ok_or("Modello Parakeet TDT non disponibile")?;

        let mut reader = hound::WavReader::open(&path_str)
            .map_err(|e| format!("Impossibile aprire il file WAV: {e}"))?;
        let spec = reader.spec();

        let raw_samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Int => {
                let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
                reader
                    .samples::<i32>()
                    .map(|s| s.map(|v| v as f32 / max))
                    .collect::<Result<Vec<f32>, _>>()
                    .map_err(|e| format!("Errore decodifica campioni WAV: {e}"))?
            }
            hound::SampleFormat::Float => reader
                .samples::<f32>()
                .collect::<Result<Vec<f32>, _>>()
                .map_err(|e| format!("Errore decodifica campioni WAV: {e}"))?,
        };

        // Assicura frequenza a 16 kHz
        let (samples, channels) = if spec.sample_rate != 16000 {
            if spec.channels == 2 {
                let left: Vec<f32> = raw_samples.iter().step_by(2).copied().collect();
                let right: Vec<f32> = raw_samples.iter().skip(1).step_by(2).copied().collect();
                let left_res = crate::recorder::audio::resample_linear(&left, spec.sample_rate, 16000);
                let right_res = crate::recorder::audio::resample_linear(&right, spec.sample_rate, 16000);
                (crate::recorder::audio::interleave_stereo(&left_res, &right_res), 2)
            } else {
                (crate::recorder::audio::resample_linear(&raw_samples, spec.sample_rate, 16000), 1)
            }
        } else {
            (raw_samples, spec.channels)
        };

        // Diarizzazione fisica garantita:
        // Channel 0 (Sinistro) = Microfono dell'utente (YOU -> "0")
        // Channel 1 (Destro)   = Audio di sistema (THEM -> "1")
        let mut segments = if channels == 2 {
            let left_samples: Vec<f32> = samples.iter().step_by(2).copied().collect();
            let right_samples: Vec<f32> = samples.iter().skip(1).step_by(2).copied().collect();

            let mut segs_left = transcribe_channel(model, left_samples, Some("0".to_string()))?;
            let segs_right = transcribe_channel(model, right_samples, Some("1".to_string()))?;

            segs_left.extend(segs_right);
            segs_left.sort_by(|a, b| {
                a.start
                    .partial_cmp(&b.start)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            segs_left
        } else {
            transcribe_channel(model, samples.clone(), None)?
        };

        for (i, seg) in segments.iter_mut().enumerate() {
            seg.id = i as u32;
        }

        let full_text = segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<&str>>()
            .join("\n");

        let duration = if channels == 2 {
            (samples.len() / 2) as f64 / 16000.0
        } else {
            samples.len() as f64 / 16000.0
        };

        let result = TranscriptResult {
            task: "transcribe".to_string(),
            language: "it".to_string(),
            duration,
            text: full_text,
            segments,
            transcription_ms: Some(started.elapsed().as_millis() as u64),
        };

        let wav_path_buf = PathBuf::from(&path_str);
        if let Some(folder) = wav_path_buf.parent() {
            if let Ok(json) = serde_json::to_vec(&result) {
                let _ = std::fs::write(folder.join("transcript.json"), json);
            }

            let entry_name = folder
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Registrazione");

            let md_content = build_transcript_markdown(entry_name, &wav_path_buf, &result);

            // 1. Salva transcript.md nella cartella della registrazione
            let _ = std::fs::write(folder.join("transcript.md"), &md_content);

            // 2. Se è configurata una cartella personalizzata (es. Vault Obsidian), salva {entry_name}.md
            if let Some(custom_vault) = &settings.transcripts_dir {
                let vault_dir = PathBuf::from(custom_vault);
                if vault_dir != folder {
                    let _ = std::fs::create_dir_all(&vault_dir);
                    let dest = vault_dir.join(format!("{entry_name}.md"));
                    let _ = std::fs::write(dest, &md_content);
                }
            }
        }

        Ok::<TranscriptResult, String>(result)
    })
    .await
    .map_err(|e| e.to_string())??;

    Ok(result)
}

#[tauri::command]
pub async fn transcribe_recording(app: AppHandle, path: String) -> Result<TranscriptResult, String> {
    let result = run_transcription(&app, &path).await;

    if let Some(folder) = Path::new(&path).parent() {
        let error_sidecar = folder.join(TRANSCRIPT_ERROR_FILE);
        match &result {
            Ok(_) => {
                tokio::fs::remove_file(&error_sidecar).await.ok();
            }
            Err(message) => {
                let error = TranscriptError {
                    message: message.clone(),
                };
                if let Ok(json) = serde_json::to_vec(&error) {
                    tokio::fs::write(&error_sidecar, json).await.ok();
                }
            }
        }
    }

    result
}
