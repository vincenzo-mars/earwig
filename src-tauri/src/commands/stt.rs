//! Modello locale, ciclo di vita di whisper-server e trascrizione.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::download::download_to_file;
use super::server::{
    default_threads, port_is_open, spawn_tracked, stop_tracked_server, try_adopt_running_server,
    wait_for_port,
};
use super::{
    bundled_bin_path, get_stt_settings, local_model_path, save_stt_settings, server_lock_path,
    STT_PORT,
};

/// Nome del lockfile del processo whisper-server (vedi `server_lock_path`).
pub(crate) const STT_LOCK: &str = "whisper";

const MODEL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin";

// ── Download ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn download_local_model(app: AppHandle) -> Result<(), String> {
    let settings = get_stt_settings(app.clone()).await;
    // whisper-server è bundled come risorsa app — qui si scarica solo il modello
    let model_path = local_model_path(&app, &settings);

    download_to_file(&app, MODEL_URL, &model_path, "download-progress", "model").await?;

    let mut settings = get_stt_settings(app.clone()).await;
    settings.local_ready = true;
    save_stt_settings(app, settings).await
}

// ── Ciclo di vita del server ──────────────────────────────────────────────────

/// Tiene l'handle del processo whisper-server per poterlo terminare
/// alla chiusura dell'app (altrimenti resta in background dopo il quit).
pub struct WhisperServerState(pub std::sync::Mutex<Option<tokio::process::Child>>);

impl WhisperServerState {
    pub fn new() -> Self {
        Self(std::sync::Mutex::new(None))
    }
}

async fn start_local_server(app: AppHandle) -> Result<(), String> {
    let settings = get_stt_settings(app.clone()).await;
    let bin = bundled_bin_path(&app, "whisper-server")?;
    let model = local_model_path(&app, &settings);

    if !model.exists() {
        return Err("Modello locale non scaricato".to_string());
    }
    let model = model
        .to_str()
        .ok_or("Percorso del modello non rappresentabile come UTF-8")?;

    let lock_path = server_lock_path(&app, STT_LOCK);
    if try_adopt_running_server(STT_PORT, &lock_path, model).await? {
        return Ok(());
    }

    // Flash attention e numero di thread non hanno UI: si deducono dalla
    // macchina. `-fa` esiste solo dalle build recenti di whisper.cpp (vedi
    // scripts/build-whisper-server.sh, tag pinnato).
    spawn_tracked(
        &bin,
        &[
            "--model",
            model,
            "--host",
            "127.0.0.1",
            "--port",
            &STT_PORT.to_string(),
            "--threads",
            &default_threads().to_string(),
            "--flash-attn",
        ],
        &app.state::<WhisperServerState>().0,
        &lock_path,
        model,
    )?;

    wait_for_port(STT_PORT, 60).await
}

#[tauri::command]
pub async fn check_stt_server() -> String {
    if port_is_open(STT_PORT).await {
        "running".to_string()
    } else {
        "stopped".to_string()
    }
}

#[tauri::command]
pub async fn start_stt_server(app: AppHandle) -> Result<(), String> {
    start_local_server(app).await
}

async fn stop_local_server(app: &AppHandle) -> Result<(), String> {
    stop_tracked_server(
        &app.state::<WhisperServerState>().0,
        STT_PORT,
        &server_lock_path(app, STT_LOCK),
    )
    .await
}

#[tauri::command]
pub async fn stop_stt_server(app: AppHandle) -> Result<(), String> {
    stop_local_server(&app).await
}

#[tauri::command]
pub async fn restart_stt_server(app: AppHandle) -> Result<(), String> {
    stop_local_server(&app).await?;
    start_local_server(app).await
}

#[tauri::command]
pub async fn delete_local_model(app: AppHandle) -> Result<(), String> {
    // Il processo tiene il .bin aperto: va fermato prima di cancellare,
    // altrimenti su alcuni filesystem la delete fallisce o lascia un file
    // fantasma finché il processo non lo rilascia da solo.
    stop_local_server(&app).await?;

    let settings = get_stt_settings(app.clone()).await;
    let model_path = local_model_path(&app, &settings);
    match tokio::fs::remove_file(&model_path).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

// ── Trascrizione ──────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug)]
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
    // Tempo di elaborazione della trascrizione (non presente nella risposta del
    // server né nei transcript.json vecchi → `default` = None). Popolato da
    // `transcribe_recording` e persistito nel sidecar.
    #[serde(default)]
    pub transcription_ms: Option<u64>,
}

/// La diarizzazione la fa whisper: su un file **stereo** con `diarize=true`
/// confronta l'energia dei due canali segmento per segmento e ritorna `"0"` per
/// il sinistro (microfono) e `"1"` per il destro (audio di sistema), oppure
/// `"?"` quando le due energie sono a meno del 10% l'una dall'altra.
/// L'ambiguità non è una terza etichetta: viene normalizzata a `None`, lo stesso
/// stato dei file mono (import, registrazioni senza audio di sistema).
fn normalize_speaker(speaker: Option<String>) -> Option<String> {
    match speaker.as_deref() {
        Some("0") | Some("1") => speaker,
        _ => None,
    }
}

/// Traccia persistente di un fallimento, scritta accanto al WAV così
/// `list_recordings` può esporre lo stato "fallito" anche dopo un riavvio
/// dell'app (un errore di rete/parsing altrimenti non lascerebbe traccia sul
/// disco). Rimossa da `transcribe_recording` al primo rilancio riuscito.
const TRANSCRIPT_ERROR_FILE: &str = "transcript_error.json";

#[derive(Serialize, Deserialize, Debug)]
pub struct TranscriptError {
    pub message: String,
}

async fn run_transcription(path: &str) -> Result<TranscriptResult, String> {
    let base_url = format!("http://127.0.0.1:{STT_PORT}");
    let started = std::time::Instant::now();

    // I file prodotti da Earwig sono già nel formato che whisper.cpp richiede
    // (16kHz 16-bit PCM), quindi vanno inviati così come sono. In streaming
    // dal file, non caricati in memoria: un'ora di stereo sono ~230MB che non
    // hanno motivo di stare in RAM durante l'upload.
    let file = tokio::fs::File::open(path).await.map_err(|e| e.to_string())?;

    let part = reqwest::multipart::Part::stream(file)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;

    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("language", "it")
        .text("response_format", "verbose_json")
        // Ignorato dal server sui file mono, dove `pcmf32s` ha un solo canale.
        .text("diarize", "true");

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base_url}/inference"))
        .multipart(form)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let mut result: TranscriptResult = resp.json().await.map_err(|e| e.to_string())?;
    result.transcription_ms = Some(started.elapsed().as_millis() as u64);
    for segment in result.segments.iter_mut() {
        segment.speaker = normalize_speaker(segment.speaker.take());
    }

    let wav_path_buf = PathBuf::from(path);
    if let Some(folder) = wav_path_buf.parent() {
        if let Ok(json) = serde_json::to_vec(&result) {
            tokio::fs::write(folder.join("transcript.json"), json)
                .await
                .ok();
        }
    }

    Ok(result)
}

#[tauri::command]
pub async fn transcribe_recording(path: String) -> Result<TranscriptResult, String> {
    let result = run_transcription(&path).await;

    if let Some(folder) = Path::new(&path).parent() {
        let error_sidecar = folder.join(TRANSCRIPT_ERROR_FILE);
        match &result {
            // Un rilancio riuscito deve cancellare l'errore di un tentativo
            // precedente: altrimenti `list_recordings` continuerebbe a
            // mostrare "fallito" su un record che ora ha un transcript valido.
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
