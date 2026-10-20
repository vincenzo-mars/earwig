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

/// Preset di modello scaricabile: tutti devono essere export ONNX di
/// Parakeet TDT compatibili con `parakeet_rs` (encoder/decoder_joint/vocab).
pub(crate) struct SttPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub dir_name: &'static str,
    pub base_url: &'static str,
    pub files: &'static [&'static str],
}

pub(crate) const STT_PRESETS: &[SttPreset] = &[
    SttPreset {
        id: "parakeet-v3-int8",
        label: "Consigliato: multilingua, ~671 MB",
        // Stessa dir storica dell'int8: gli install esistenti non riscaricano.
        dir_name: "parakeet-tdt-0.6b-v3",
        base_url: PARAKEET_BASE_URL,
        files: &[
            "vocab.txt",
            "decoder_joint-model.int8.onnx",
            "encoder-model.int8.onnx",
        ],
    },
    SttPreset {
        id: "parakeet-v3-fp32",
        label: "Massima qualità: multilingua, ~2.55 GB",
        dir_name: "parakeet-tdt-0.6b-v3-fp32",
        base_url: PARAKEET_BASE_URL,
        files: &[
            "vocab.txt",
            "decoder_joint-model.onnx",
            "encoder-model.onnx",
            "encoder-model.onnx.data",
        ],
    },
];

pub(crate) fn stt_preset(id: &str) -> &'static SttPreset {
    STT_PRESETS
        .iter()
        .find(|p| p.id == id)
        .unwrap_or(&STT_PRESETS[0])
}

#[derive(Serialize)]
pub struct SttPresetDto {
    pub id: String,
    pub label: String,
}

#[tauri::command]
pub fn get_stt_presets() -> Vec<SttPresetDto> {
    STT_PRESETS
        .iter()
        .map(|p| SttPresetDto {
            id: p.id.to_string(),
            label: p.label.to_string(),
        })
        .collect()
}

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
    let preset = stt_preset(&settings.stt_model_id);
    let parakeet_dir = local_parakeet_dir(&app, &settings);
    tokio::fs::create_dir_all(&parakeet_dir)
        .await
        .map_err(|e| e.to_string())?;

    // Una sola installazione alla volta: al cambio di preset la dir del
    // modello precedente viene rimossa (sostituzione, non accumulo).
    for other in STT_PRESETS.iter().filter(|p| p.id != preset.id) {
        let old_dir = parakeet_dir
            .parent()
            .map(|models| models.join(other.dir_name));
        if let Some(old_dir) = old_dir {
            if old_dir != parakeet_dir {
                tokio::fs::remove_dir_all(&old_dir).await.ok();
            }
        }
    }

    for file in preset.files {
        let dest = parakeet_dir.join(file);
        if dest.exists() {
            continue;
        }
        let label = if file.contains("encoder") {
            "model"
        } else if file.contains("decoder") {
            "decoder"
        } else {
            "vocab"
        };
        download_to_file(
            &app,
            &format!("{}/{file}", preset.base_url),
            &dest,
            "download-progress",
            label,
        )
        .await?;
    }

    // Il motore in-memory potrebbe puntare alla dir appena cancellata o al
    // modello nuovo: si azzera e il lazy-load ricaricherà dalla dir scelta.
    app.state::<SttEngineState>()
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .take();

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

/// Trascrive un canale audio mono a 16 kHz suddividendolo in blocchi da ~120
/// secondi per garantire stabilità della memoria anche su registrazioni
/// prolungate. I tagli non cadono a tempo fisso: vicino al limite di 120s il
/// blocco viene spezzato nel frame più silenzioso di una finestra di ±5s, per
/// non troncare parole e frasi a metà.
fn transcribe_channel(
    model: &mut ParakeetTDT,
    samples: Vec<f32>,
    speaker: Option<String>,
) -> Result<Vec<TranscriptSegment>, String> {
    if samples.is_empty() {
        return Ok(Vec::new());
    }

    const CHUNK_LEN: usize = 120 * 16_000;
    const SEARCH_WINDOW: usize = 5 * 16_000;
    const FRAME: usize = 480; // 30 ms a 16 kHz
    let mut segments = Vec::new();

    // Bordi dei blocchi: il bordo successivo cerca il frame con l'energia
    // minima nella finestra attorno a pos + CHUNK_LEN, così il taglio cade in
    // una pausa invece che a metà parola.
    let mut boundaries = vec![0usize];
    while samples.len() - *boundaries.last().unwrap() > CHUNK_LEN {
        let target = boundaries.last().unwrap() + CHUNK_LEN;
        let lo = target.saturating_sub(SEARCH_WINDOW);
        let hi = (target + SEARCH_WINDOW).min(samples.len());
        let mut quietest = target;
        let mut quietest_energy = f32::MAX;
        let mut frame = lo;
        while frame + FRAME <= hi {
            let energy: f32 = samples[frame..frame + FRAME].iter().map(|s| s * s).sum::<f32>()
                / FRAME as f32;
            if energy < quietest_energy {
                quietest_energy = energy;
                quietest = frame + FRAME / 2;
            }
            frame += FRAME;
        }
        boundaries.push(quietest);
    }
    boundaries.push(samples.len());

    for pair in boundaries.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        let chunk = &samples[start..end];
        let chunk_offset_sec = start as f64 / 16_000.0;

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

        // Input ASR ottimizzato: stop_recording salva mic_asr.wav quando il mic
        // conviene nudo (cuffie, nessun leakage) o con solo AEC (altoparlanti),
        // sempre a 16 kHz e senza gain. Il canale "Altri" resta il destro del
        // WAV stereo (audio di sistema digitale, già pulito). Le registrazioni
        // vecchie senza mic_asr.wav seguono la catena storica.
        let asr_mic_path = Path::new(&path_str)
            .parent()
            .map(|p| p.join("mic_asr.wav"))
            .filter(|p| p.exists());

        let mut segments = if let Some(asr_path) = asr_mic_path {
            let mut asr_reader = hound::WavReader::open(&asr_path)
                .map_err(|e| format!("Impossibile aprire mic_asr.wav: {e}"))?;
            let asr_spec = asr_reader.spec();
            let mic_samples: Vec<f32> = match asr_spec.sample_format {
                hound::SampleFormat::Int => {
                    let max = (1i64 << (asr_spec.bits_per_sample - 1)) as f32;
                    asr_reader
                        .samples::<i32>()
                        .map(|s| s.map(|v| v as f32 / max))
                        .collect::<Result<Vec<f32>, _>>()
                        .map_err(|e| format!("Errore decodifica campioni: {e}"))?
                }
                hound::SampleFormat::Float => asr_reader
                    .samples::<f32>()
                    .collect::<Result<Vec<f32>, _>>()
                    .map_err(|e| format!("Errore decodifica campioni: {e}"))?,
            };
            let mic_samples = if asr_spec.sample_rate != 16000 {
                crate::recorder::audio::resample(&mic_samples, asr_spec.sample_rate, 16000)
            } else {
                mic_samples
            };

            let mut segs = transcribe_channel(model, mic_samples, Some("0".to_string()))?;

            if spec.channels == 2 {
                let right: Vec<f32> = raw_samples.iter().skip(1).step_by(2).copied().collect();
                let right = if spec.sample_rate != 16000 {
                    crate::recorder::audio::resample(&right, spec.sample_rate, 16000)
                } else {
                    right
                };
                segs.extend(transcribe_channel(model, right, Some("1".to_string()))?);
            }
            segs
        } else if spec.sample_rate != 16000 {
            if spec.channels == 2 {
                let left: Vec<f32> = raw_samples.iter().step_by(2).copied().collect();
                let right: Vec<f32> = raw_samples.iter().skip(1).step_by(2).copied().collect();
                let left_res = crate::recorder::audio::resample(&left, spec.sample_rate, 16000);
                let right_res = crate::recorder::audio::resample(&right, spec.sample_rate, 16000);
                let mut segs =
                    transcribe_channel(model, left_res, Some("0".to_string()))?;
                segs.extend(transcribe_channel(model, right_res, Some("1".to_string()))?);
                segs
            } else {
                let mono = crate::recorder::audio::resample(&raw_samples, spec.sample_rate, 16000);
                transcribe_channel(model, mono, None)?
            }
        } else if spec.channels == 2 {
            let left: Vec<f32> = raw_samples.iter().step_by(2).copied().collect();
            let right: Vec<f32> = raw_samples.iter().skip(1).step_by(2).copied().collect();
            let mut segs = transcribe_channel(model, left, Some("0".to_string()))?;
            segs.extend(transcribe_channel(model, right, Some("1".to_string()))?);
            segs
        } else {
            transcribe_channel(model, raw_samples.clone(), None)?
        };

        segments.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));

        for (i, seg) in segments.iter_mut().enumerate() {
            seg.id = i as u32;
        }

        let full_text = segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<&str>>()
            .join("\n");

        let duration = raw_samples.len() as f64 / (spec.sample_rate as f64 * spec.channels as f64);

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
