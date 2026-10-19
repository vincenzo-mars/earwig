//! Ricerca modelli Hugging Face, ciclo di vita di llama-server (riassunto e
//! chat locali), e sidecar note per registrazione.

use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::download::download_to_file;
use super::server::{
    default_threads, port_is_open, spawn_tracked, stop_tracked_server, try_adopt_running_server,
};
use super::{
    bundled_bin_path, get_stt_settings, llm_cache_dir, llm_model_path, llm_models_dir,
    save_stt_settings, server_lock_path, LLM_PORT,
};

/// Nome del lockfile del processo llama-server (vedi `server_lock_path`).
pub(crate) const LLM_LOCK: &str = "llm";

// ── Ciclo di vita del server ───────────────────────────────────────────────────

/// Tiene l'handle del processo llama-server per poterlo terminare alla
/// chiusura dell'app, come `WhisperServerState`. Processo indipendente da
/// quello di whisper: avviare/fermare l'uno non tocca l'altro.
pub struct LlamaServerState(pub std::sync::Mutex<Option<tokio::process::Child>>);

impl LlamaServerState {
    pub fn new() -> Self {
        Self(std::sync::Mutex::new(None))
    }
}

/// RAM totale letta direttamente dall'OS (sysctl su macOS, /proc/meminfo su
/// Linux): un numero solo non giustifica la dipendenza `sysinfo`.
#[tauri::command]
pub fn get_system_memory_gb() -> f64 {
    total_memory_bytes() as f64 / 1024f64.powi(3)
}

#[cfg(target_os = "macos")]
fn total_memory_bytes() -> u64 {
    unsafe extern "C" {
        fn sysctlbyname(
            name: *const std::os::raw::c_char,
            oldp: *mut std::os::raw::c_void,
            oldlenp: *mut usize,
            newp: *mut std::os::raw::c_void,
            newlen: usize,
        ) -> i32;
    }
    let mut value: u64 = 0;
    let mut len = std::mem::size_of::<u64>();
    let rc = unsafe {
        sysctlbyname(
            c"hw.memsize".as_ptr(),
            &mut value as *mut u64 as *mut _,
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc == 0 {
        value
    } else {
        0
    }
}

#[cfg(not(target_os = "macos"))]
fn total_memory_bytes() -> u64 {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("MemTotal:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|kb| kb.parse::<u64>().ok())
        })
        .map(|kb| kb * 1024)
        .unwrap_or(0)
}

/// A differenza di whisper-server, llama-server apre la porta prima ancora
/// di aver caricato il modello (`/health` risponde 503 mentre carica, 200
/// quando è pronto): il solo probe TCP non basta a distinguere "in
/// caricamento" da "pronto".
#[tauri::command]
pub async fn check_llm_server() -> String {
    if !port_is_open(LLM_PORT).await {
        return "stopped".to_string();
    }
    match reqwest::get(format!("http://127.0.0.1:{LLM_PORT}/health")).await {
        Ok(resp) if resp.status().is_success() => "running".to_string(),
        _ => "loading".to_string(),
    }
}

#[tauri::command]
pub async fn start_llm_server(app: AppHandle) -> Result<(), String> {
    let settings = get_stt_settings(app.clone()).await;
    let model_path = llm_model_path(&app, &settings)
        .filter(|p| p.exists())
        .ok_or_else(|| "Modello LLM non scaricato: scaricalo dalle impostazioni".to_string())?;
    let model_path = model_path
        .to_str()
        .ok_or("Percorso del modello non rappresentabile come UTF-8")?;

    let lock_path = server_lock_path(&app, LLM_LOCK);
    if try_adopt_running_server(LLM_PORT, &lock_path, model_path).await? {
        return Ok(());
    }

    let bin = bundled_bin_path(&app, "llama-server")?;

    spawn_tracked(
        &bin,
        &[
            "--model",
            model_path,
            "--host",
            "127.0.0.1",
            "--port",
            &LLM_PORT.to_string(),
            "--alias",
            "earwig-llm",
            "--ctx-size",
            "16384",
            "--threads",
            &default_threads().to_string(),
            "--n-gpu-layers",
            "999",
            "--parallel",
            "1",
            "--no-webui",
        ],
        &app.state::<LlamaServerState>().0,
        &lock_path,
        model_path,
    )?;

    // Niente attesa di `/health` qui: il modello è già sul disco (vedi guard
    // sopra), ma caricare qualche GB in RAM/VRAM può comunque richiedere
    // parecchi secondi. Bloccare il comando Tauri per quel tempo sarebbe
    // sbagliato: il frontend fa polling di `check_llm_server` mostrando
    // "loading" nel frattempo.
    Ok(())
}

/// Scarica il GGUF scelto direttamente da Hugging Face (stesso pattern di
/// `download_local_model` per whisper), invece di lasciarlo scaricare a
/// llama-server via `--hf-repo`/`--hf-file`: la sua barra di progresso
/// nativa è gated su `isatty(stdout)` nel sorgente di llama.cpp
/// (`common/download.cpp`) e non produce alcun output sotto
/// `Stdio::piped()`, quindi non è osservabile da un processo figlio (vedi
/// DEVLOG). Scaricando noi stessi, la percentuale viene da `content_length()`
/// esattamente come per whisper, e `llm_ready` diventa derivabile dal file
/// sul disco invece che da un evento da tracciare a parte.
#[tauri::command]
pub async fn download_llm_model(app: AppHandle) -> Result<(), String> {
    let settings = get_stt_settings(app.clone()).await;
    let model_path = llm_model_path(&app, &settings).ok_or_else(|| {
        "Nessun modello LLM selezionato: scegline uno dalle impostazioni".to_string()
    })?;

    let url = format!(
        "https://huggingface.co/{}/resolve/main/{}",
        settings.llm_hf_repo, settings.llm_hf_file
    );

    download_to_file(&app, &url, &model_path, "llm-download-progress", "llm").await
}

#[tauri::command]
pub async fn stop_llm_server(app: AppHandle) -> Result<(), String> {
    stop_tracked_server(
        &app.state::<LlamaServerState>().0,
        LLM_PORT,
        &server_lock_path(&app, LLM_LOCK),
    )
    .await
}

#[tauri::command]
pub async fn restart_llm_server(app: AppHandle) -> Result<(), String> {
    stop_tracked_server(
        &app.state::<LlamaServerState>().0,
        LLM_PORT,
        &server_lock_path(&app, LLM_LOCK),
    )
    .await?;
    start_llm_server(app).await
}

#[tauri::command]
pub async fn set_llm_model(
    app: AppHandle,
    repo: String,
    file: String,
    size_bytes: u64,
) -> Result<(), String> {
    let mut settings = get_stt_settings(app.clone()).await;
    settings.llm_hf_repo = repo;
    settings.llm_hf_file = file;
    settings.llm_size_bytes = size_bytes;
    save_stt_settings(app, settings).await
}

/// Cancella sia i modelli scaricati da earwig stesso (`llm-models/`) sia la
/// vecchia cache nativa di llama-server (`llm-cache/`, layout hash-based) per
/// chi aveva già scaricato un modello con una versione precedente
/// dell'app: nessuna migrazione automatica, solo pulizia su richiesta
/// esplicita (vedi DEVLOG).
#[tauri::command]
pub async fn clear_llm_cache(app: AppHandle) -> Result<(), String> {
    // Il processo tiene i file di modello aperti (mmap): va fermato prima di
    // cancellare, stesso motivo di `delete_local_model` per whisper.
    stop_tracked_server(
        &app.state::<LlamaServerState>().0,
        LLM_PORT,
        &server_lock_path(&app, LLM_LOCK),
    )
    .await?;

    let settings = get_stt_settings(app.clone()).await;

    for dir in [
        llm_models_dir(&app, &settings),
        llm_cache_dir(&app, &settings),
    ] {
        match tokio::fs::remove_dir_all(&dir).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }

    Ok(())
}

// ── Ricerca modelli Hugging Face ───────────────────────────────────────────────

const HF_API_BASE: &str = "https://huggingface.co/api/models";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HfModelSummary {
    pub id: String,
    pub downloads: u64,
    pub license: Option<String>,
}

#[derive(Deserialize)]
struct HfSearchEntry {
    id: String,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    tags: Vec<String>,
}

fn license_from_tags(tags: &[String]) -> Option<String> {
    tags.iter()
        .find_map(|t| t.strip_prefix("license:").map(|s| s.to_string()))
}

/// Proxy verso l'API di ricerca pubblica di Hugging Face, filtrata ai soli
/// repo GGUF text-generation. Lato Rust (non fetch diretta dal webview) per
/// coerenza con l'unico altro precedente di HTTP nel progetto: tutto passa
/// da qui, mai dal webview.
#[tauri::command]
pub async fn search_hf_models(query: String) -> Result<Vec<HfModelSummary>, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(HF_API_BASE)
        .query(&[
            ("search", query.as_str()),
            ("filter", "gguf"),
            ("pipeline_tag", "text-generation"),
            ("sort", "downloads"),
            ("direction", "-1"),
            ("limit", "20"),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let entries: Vec<HfSearchEntry> = resp.json().await.map_err(|e| e.to_string())?;
    Ok(entries
        .into_iter()
        .map(|e| HfModelSummary {
            license: license_from_tags(&e.tags),
            id: e.id,
            downloads: e.downloads,
        })
        .collect())
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HfGgufFile {
    pub filename: String,
    pub size_bytes: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HfModelDetail {
    pub gated: bool,
    pub files: Vec<HfGgufFile>,
}

#[derive(Deserialize)]
struct HfSibling {
    rfilename: String,
    #[serde(default)]
    size: Option<u64>,
}

#[derive(Deserialize)]
struct HfModelDetailResponse {
    // Può essere `false`, `true`, o una stringa ("auto"/"manual") a seconda
    // della modalità di gating: qualunque cosa diversa da `false` è gated.
    #[serde(default)]
    gated: serde_json::Value,
    #[serde(default)]
    siblings: Vec<HfSibling>,
}

/// I modelli GGUF molto grandi (40GB+) sono a volte pubblicati come più file
/// (`<nome>-00001-of-00005.gguf`, llama.cpp li ricompone da sé caricando solo
/// il primo). Supportarli richiederebbe raggruppare gli shard e sommarne le
/// size per la progress bar: fuori scope per questa iterazione (vedi
/// DEVLOG), qui vengono esclusi dalla lista selezionabile.
fn is_gguf_shard(filename: &str) -> bool {
    let Some(stem) = filename.strip_suffix(".gguf") else {
        return false;
    };
    let Some((before, after)) = stem.rsplit_once("-of-") else {
        return false;
    };
    if after.is_empty() || !after.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    match before.rsplit_once('-') {
        Some((_, num)) => !num.is_empty() && num.chars().all(|c| c.is_ascii_digit()),
        None => false,
    }
}

/// `?blobs=true` è l'unico modo per ottenere la dimensione reale dei file
/// nell'endpoint di dettaglio repo — senza, `siblings[].size` è assente.
#[tauri::command]
pub async fn get_hf_model_files(repo_id: String) -> Result<HfModelDetail, String> {
    let client = reqwest::Client::new();
    let url = format!("{HF_API_BASE}/{repo_id}?blobs=true");
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let detail: HfModelDetailResponse = resp.json().await.map_err(|e| e.to_string())?;

    let gated = !matches!(detail.gated, serde_json::Value::Bool(false));
    let files = detail
        .siblings
        .into_iter()
        .filter(|s| s.rfilename.ends_with(".gguf") && !is_gguf_shard(&s.rfilename))
        .map(|s| HfGgufFile {
            filename: s.rfilename,
            size_bytes: s.size.unwrap_or(0),
        })
        .collect::<Vec<_>>();

    Ok(HfModelDetail { gated, files })
}

// ── Note per registrazione (riassunto + chat) ──────────────────────────────────

const NOTES_FILE: &str = "notes.json";

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct NotesSummary {
    pub text: String,
    pub key_points: Vec<String>,
    pub actions: Vec<String>,
    pub open_questions: Vec<String>,
    pub model: String,
    pub generated_at: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub at: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct RecordingNotes {
    pub version: u32,
    pub summary: Option<NotesSummary>,
    pub messages: Vec<ChatMessage>,
}

/// File mancante o non parsabile ⇒ documento vuoto, mai un errore — stesso
/// grado di tolleranza con cui `list_recordings` legge `transcript.json`.
#[tauri::command]
pub async fn read_recording_notes(folder_path: String) -> RecordingNotes {
    let path = Path::new(&folder_path).join(NOTES_FILE);
    tokio::fs::read(&path)
        .await
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Whole-document replace con scrittura atomica (`.part` + rename), stesso
/// schema del download modello: un crash a metà scrittura non deve lasciare
/// un `notes.json` troncato che poi fallisce il parsing e cancella in
/// silenzio la cronologia chat.
#[tauri::command]
pub async fn write_recording_notes(
    folder_path: String,
    notes: RecordingNotes,
) -> Result<(), String> {
    let folder = Path::new(&folder_path);
    let final_path = folder.join(NOTES_FILE);
    let tmp_path = folder.join(format!("{NOTES_FILE}.part"));
    let json = serde_json::to_vec(&notes).map_err(|e| e.to_string())?;
    tokio::fs::write(&tmp_path, json)
        .await
        .map_err(|e| e.to_string())?;
    tokio::fs::rename(&tmp_path, &final_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_recording_notes(folder_path: String) -> Result<(), String> {
    let path = Path::new(&folder_path).join(NOTES_FILE);
    match tokio::fs::remove_file(&path).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
