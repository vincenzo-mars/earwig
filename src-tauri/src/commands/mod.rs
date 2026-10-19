//! Tauri commands esposti al frontend, divisi per dominio:
//! qui impostazioni, percorsi e permessi OS; `stt` per modello, server e
//! trascrizione; `recording` per cattura, import e listing; `llm` per
//! ricerca modelli Hugging Face, ciclo di vita di llama-server e note
//! (riassunto/chat) per registrazione; `server` per le primitive di
//! processo/porta condivise fra `stt` e `llm`; `download` per lo streaming
//! HTTP condiviso fra il download del modello whisper e quello dell'LLM.

pub mod download;
pub mod llm;
pub mod recording;
pub mod server;
pub mod stt;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::permissions;

pub(crate) const STT_PORT: u16 = 8080;
// llama-server di suo usa 8080 come default (stesso di whisper-server): va
// sempre passato `--port` esplicito, mai lasciato implicito.
pub(crate) const LLM_PORT: u16 = 8081;

// ── Settings ─────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SttSettings {
    pub local_ready: bool,
    pub model_dir: Option<String>,
    pub recordings_dir: Option<String>,
    // Repo/file scelti dall'utente per il modello LLM locale (riassunto/chat).
    // `#[serde(default)]` è obbligatorio: senza, un settings.json esistente
    // scritto prima di questi campi fallirebbe il parsing e resetterebbe in
    // silenzio anche model_dir/recordings_dir (vedi get_stt_settings sotto).
    #[serde(default)]
    pub llm_hf_repo: String,
    #[serde(default)]
    pub llm_hf_file: String,
    // Size del file scelto (nota al frontend da HfGgufFile.size_bytes al
    // momento della selezione): serve solo a etichettare il bottone di
    // download, non al calcolo della percentuale (quella viene da
    // `content_length()` durante lo stream).
    #[serde(default)]
    pub llm_size_bytes: u64,
    // Come `local_ready` per whisper: valore persistito solo indicativo, la
    // verità è sempre il file sul disco (vedi get_stt_settings sotto).
    #[serde(default)]
    pub llm_ready: bool,
}

fn app_data_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("earwig"))
}

fn settings_path(app: &AppHandle) -> PathBuf {
    app_data_dir(app).join("settings.json")
}

/// Lockfile del processo di un server locale (`whisper`, `llm`): sopravvive
/// alla morte violenta dell'app ed è l'unico modo, alla sessione successiva,
/// di distinguere un nostro orfano da un processo di terzi che occupa la
/// stessa porta (vedi `server.rs`).
pub(crate) fn server_lock_path(app: &AppHandle, name: &str) -> PathBuf {
    app_data_dir(app).join("run").join(format!("{name}.json"))
}

fn model_dir(app: &AppHandle, settings: &SttSettings) -> PathBuf {
    match &settings.model_dir {
        Some(dir) => PathBuf::from(dir),
        None => app_data_dir(app),
    }
}

pub(crate) fn bundled_bin_path(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    app.path()
        .resolve(name, tauri::path::BaseDirectory::Resource)
        .map_err(|e| e.to_string())
}

pub(crate) fn local_model_path(app: &AppHandle, settings: &SttSettings) -> PathBuf {
    model_dir(app, settings)
        .join("models")
        .join("ggml-large-v3-turbo.bin")
}

/// Percorso del GGUF scelto dall'utente, scaricato da earwig stesso (vedi
/// `llm::download_llm_model`) invece che dal downloader nativo di
/// llama-server: la sua barra di progresso è gated su `isatty(stdout)` nel
/// sorgente di llama.cpp e non produce nulla sotto `Stdio::piped()` (vedi
/// DEVLOG). Lo slash del repo è sanitizzato in `--`: nessun hash, percorso
/// ispezionabile a mano nel Finder.
pub(crate) fn llm_models_dir(app: &AppHandle, settings: &SttSettings) -> PathBuf {
    model_dir(app, settings).join("llm-models")
}

pub(crate) fn llm_model_path(app: &AppHandle, settings: &SttSettings) -> Option<PathBuf> {
    if settings.llm_hf_repo.is_empty() || settings.llm_hf_file.is_empty() {
        return None;
    }
    Some(
        llm_models_dir(app, settings)
            .join(settings.llm_hf_repo.replace('/', "--"))
            .join(&settings.llm_hf_file),
    )
}

/// Vecchia directory della cache nativa di llama-server (`LLAMA_CACHE`,
/// layout hash-based `models--org--repo/blobs/...`), da quando il download
/// era delegato a `--hf-repo`/`--hf-file`. Non più scritta: resta solo perché
/// `clear_llm_cache` deve poter ripulire quella di chi ha già scaricato un
/// modello con una versione precedente di earwig (vedi DEVLOG).
pub(crate) fn llm_cache_dir(app: &AppHandle, settings: &SttSettings) -> PathBuf {
    model_dir(app, settings).join("llm-cache")
}

fn default_recordings_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .document_dir()
        .unwrap_or_else(|_| app_data_dir(app))
        .join("Earwig")
        .join("Records")
}

pub(crate) fn recordings_dir(app: &AppHandle, settings: &SttSettings) -> PathBuf {
    match &settings.recordings_dir {
        Some(dir) => PathBuf::from(dir),
        None => default_recordings_dir(app),
    }
}

#[tauri::command]
pub async fn get_stt_settings(app: AppHandle) -> SttSettings {
    let path = settings_path(&app);
    let mut settings: SttSettings = tokio::fs::read_to_string(&path)
        .await
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    // local_ready persistito è solo l'ultimo valore noto: la verità è il file
    // sul disco (l'utente può cancellarlo a mano, un download può essere stato
    // interrotto). Va sempre riverificato, mai fidarsi del flag salvato.
    settings.local_ready = local_model_path(&app, &settings).exists();
    settings.llm_ready = llm_model_path(&app, &settings).is_some_and(|p| p.exists());
    settings
}

#[tauri::command]
pub async fn save_stt_settings(app: AppHandle, settings: SttSettings) -> Result<(), String> {
    let path = settings_path(&app);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    tokio::fs::write(&path, json)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_local_model_path(app: AppHandle) -> String {
    let settings = get_stt_settings(app.clone()).await;
    let path = local_model_path(&app, &settings);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }
    path.to_string_lossy().into_owned()
}

#[tauri::command]
pub async fn get_recordings_dir(app: AppHandle) -> String {
    let settings = get_stt_settings(app.clone()).await;
    let dir = recordings_dir(&app, &settings);
    tokio::fs::create_dir_all(&dir).await.ok();
    dir.to_string_lossy().into_owned()
}

// ── Permessi OS ───────────────────────────────────────────────────────────────

#[tauri::command]
pub fn check_screen_recording_permission() -> bool {
    permissions::screen_recording_granted()
}

#[tauri::command]
pub fn open_permission_settings(pane: String) -> Result<(), String> {
    let pane = match pane.as_str() {
        "microphone" => permissions::PermissionPane::Microphone,
        "screen-recording" => permissions::PermissionPane::ScreenRecording,
        other => return Err(format!("Pannello permessi sconosciuto: {other}")),
    };
    permissions::open_settings_pane(pane)
}
