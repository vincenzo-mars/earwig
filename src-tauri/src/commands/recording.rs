//! Cattura microfono + audio di sistema, import di file esterni e listing.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use super::stt::{TranscriptError, TranscriptResult};
use super::{get_stt_settings, is_stt_model_ready, recordings_dir};
use crate::recorder::audio::{self, TARGET_SAMPLE_RATE};
use crate::recorder::{aec, mic, system_audio, RecorderState};

const AUDIO_EXTENSIONS: &[&str] = &["wav", "mp3", "m4a", "mp4", "flac", "ogg", "aac"];

/// Guardia condivisa da `start_recording` e `import_audio_file`: entrambe
/// finiscono in `transcribe_recording`, quindi entrambe devono rifiutarsi se
/// il modello non è sul disco o whisper-server non è in esecuzione. Ripete
/// lato backend il gate già applicato in UI, per il caso in cui il frontend
/// sia disallineato (es. chiamata programmatica, race sullo stato).
async fn ensure_stt_ready(app: &AppHandle) -> Result<(), String> {
    let settings = get_stt_settings(app.clone()).await;
    if !is_stt_model_ready(app, &settings) {
        return Err(
            "Modello vocale (Parakeet TDT v3) non scaricato: scaricalo dalle impostazioni prima di continuare"
                .to_string(),
        );
    }
    Ok(())
}

/// Crea la cartella `<recordings_dir>/<timestamp>` per una nuova entry e
/// ritorna il path del WAV da scriverci. Condivisa da `stop_recording` e
/// `import_audio_file`, che devono produrre entry indistinguibili.
async fn new_recording_path(app: &AppHandle, custom_name: Option<String>) -> Result<PathBuf, String> {
    let settings = get_stt_settings(app.clone()).await;
    let dir = recordings_dir(app, &settings);
    let folder_name = custom_name.unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d %H.%M.%S").to_string());
    let folder = dir.join(&folder_name);
    tokio::fs::create_dir_all(&folder)
        .await
        .map_err(|e| e.to_string())?;
    Ok(folder.join("recording.wav"))
}

// ── Import ─────────────────────────────────────────────────────────────────────

/// Decodifica un file audio arbitrario (wav/mp3/m4a/mp4/flac/ogg) in campioni f32
/// interleaved via symphonia (Rust puro, niente ffmpeg — coerente con il resample
/// puro-Rust già usato qui). Ritorna (samples, channels, sample_rate).
fn decode_audio(path: &Path) -> Result<(Vec<f32>, u16, u32), String> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::errors::Error as SymphoniaError;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("Formato audio non riconosciuto: {e}"))?;
    let mut format = probed.format;

    let track = format
        .default_track()
        .ok_or("Nessuna traccia audio nel file")?;
    let track_id = track.id;
    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or("Sample rate mancante nel file audio")?;
    let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(1) as u16;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| e.to_string())?;

    let mut samples: Vec<f32> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            // Fine stream: symphonia segnala EOF come IoError(UnexpectedEof).
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break
            }
            Err(e) => return Err(e.to_string()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
                buf.copy_interleaved_ref(decoded);
                samples.extend_from_slice(buf.samples());
            }
            // Un pacchetto corrotto non deve abortire l'intero file.
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(e.to_string()),
        }
    }

    if samples.is_empty() {
        return Err("Il file audio non contiene campioni decodificabili".to_string());
    }

    Ok((samples, channels, sample_rate))
}

/// Importa un file audio esterno: apre il picker, decodifica, e lo salva come
/// `recording.wav` (mono 16kHz 16-bit) in una nuova cartella — identico a ciò che
/// produce `stop_recording`, così l'entry segue lo stesso flusso (Records +
/// `transcribe_recording`). Ritorna il path del WAV, oppure `None` se annullato.
#[tauri::command]
pub async fn import_audio_file(app: AppHandle) -> Result<Option<String>, String> {
    ensure_stt_ready(&app).await?;

    let picker_app = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        picker_app
            .dialog()
            .file()
            .add_filter("Audio", AUDIO_EXTENSIONS)
            .blocking_pick_file()
            .and_then(|fp| fp.into_path().ok())
    })
    .await
    .map_err(|e| e.to_string())?;

    let Some(src) = picked else {
        return Ok(None);
    };

    let (samples, channels, sample_rate) = {
        let src = src.clone();
        tokio::task::spawn_blocking(move || decode_audio(&src))
            .await
            .map_err(|e| e.to_string())??
    };

    let mono = audio::to_mono(samples, channels);
    let mono16k = audio::resample_linear(&mono, sample_rate, TARGET_SAMPLE_RATE);

    let path = new_recording_path(&app, None).await?;
    audio::write_wav(&mono16k, &path, TARGET_SAMPLE_RATE, 1)?;

    Ok(Some(path.to_string_lossy().into_owned()))
}

// ── Registrazione ─────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct RecordingEntry {
    pub folder_path: String,
    pub name: String,
    pub transcript: Option<TranscriptResult>,
    /// Messaggio dell'ultimo fallimento di trascrizione, letto da
    /// `transcript_error.json`. Sempre `None` quando `transcript` è presente:
    /// un rilancio riuscito rimuove il sidecar d'errore (vedi `stt.rs`).
    pub error: Option<String>,
}

#[tauri::command]
pub async fn start_recording(
    app: AppHandle,
    state: State<'_, RecorderState>,
) -> Result<(), String> {
    ensure_stt_ready(&app).await?;

    let mut inner = state.0.lock().await;
    if inner.is_recording {
        return Err("Registrazione già in corso".to_string());
    }

    let mic_buf = inner.mic_samples.clone();
    let sys_buf = inner.sys_samples.clone();

    mic_buf.lock().unwrap().clear();
    sys_buf.lock().unwrap().clear();

    let mic_info = mic::start_mic_capture(mic_buf)?;
    // SCStream (macOS) converte nativamente al rate richiesto — catturare l'audio
    // di sistema a mono (1 canale) e a TARGET_SAMPLE_RATE evita downmix e resampling a valle.
    let sys_capture = match system_audio::start(sys_buf, TARGET_SAMPLE_RATE, 1) {
        Ok(capture) => Some(capture),
        Err(e) => {
            eprintln!("Cattura audio di sistema non disponibile: {e}");
            None
        }
    };

    inner.sample_rate = TARGET_SAMPLE_RATE;
    inner.mic_native_rate = mic_info.sample_rate;
    inner.channels = mic_info.channels;
    inner.mic_stream = Some(mic_info.stream);
    inner.sys_capture = sys_capture;
    inner.is_recording = true;

    Ok(())
}

#[tauri::command]
pub async fn stop_recording(
    state: State<'_, RecorderState>,
    app: AppHandle,
    timestamp: Option<String>,
) -> Result<String, String> {
    // Dal lock si esce solo con i buffer e i parametri: resample/AEC/scrittura
    // su minuti di audio sono CPU-bound e non devono girare né dentro il lock
    // (bloccherebbero ogni altro command) né sul runtime async.
    let (mic_raw, sys, sr, ch, mic_native_rate, had_sys_audio) = {
        let mut inner = state.0.lock().await;
        if !inner.is_recording {
            return Err("Nessuna registrazione in corso".to_string());
        }

        // Resettiamo sempre lo stato a non registrando prima di qualsiasi operazione
        // per evitare che errori di cleanup o chiamate concorrenti lascino il registratore bloccato.
        inner.is_recording = false;
        inner.mic_stream = None;
        let had_sys_audio = inner.sys_capture.is_some();
        if let Some(mut sys) = inner.sys_capture.take() {
            let _ = sys.stop();
        }

        let mic_raw = std::mem::take(&mut *inner.mic_samples.lock().unwrap());
        let sys = std::mem::take(&mut *inner.sys_samples.lock().unwrap());
        (
            mic_raw,
            sys,
            inner.sample_rate,
            inner.channels,
            inner.mic_native_rate,
            had_sys_audio,
        )
    };

    let path = new_recording_path(&app, timestamp).await?;
    let wav_path = path.clone();
    tokio::task::spawn_blocking(move || {
        // Il mic cattura al rate nativo del device (cpal non lo forza) e con il
        // numero di canali del device: portarlo a mono prima del resample è
        // obbligatorio, perché l'interpolazione lineare su buffer interleaved
        // mescolerebbe campioni di canali diversi.
        let mic = audio::to_mono(mic_raw, ch);
        let mic = audio::resample_linear(&mic, mic_native_rate, sr);

        // AEC: rimuove dal mic l'echo acustico delle casse prima di scrivere il
        // file. Non è solo qualità audio: senza, l'audio di sistema rientrato nel
        // canale mic falserebbe il conto di energia con cui whisper attribuisce
        // gli speaker.
        let mic = if had_sys_audio && !sys.is_empty() {
            aec::cancel_echo(&mic, &sys, sr)
        } else {
            mic
        };

        // Bilanciamento del volume: dopo l'AEC il microfono ha spesso un livello RMS
        // notevolmente inferiore rispetto all'audio digitale di sistema (catturato a 0/-6 dBFS).
        // Pareggiamo l'RMS attivo del microfono a quello del sistema (o al target speech di 0.10)
        // per garantire un ascolto stereo naturale, evitare che nel mix mono di Whisper la voce
        // dell'utente scompaia causando allucinazioni, e rendere equo il confronto di energia L/R.
        let mic = if had_sys_audio && !sys.is_empty() {
            audio::balance_speech_levels(mic, &sys)
        } else {
            audio::normalize_speech(mic, 0.10)
        };

        // Con audio di sistema il file è stereo (mic a sinistra, sistema a
        // destra) così whisper può diarizzarlo da solo; senza, resta mono.
        let (samples, channels) = if had_sys_audio && !sys.is_empty() {
            (audio::interleave_stereo(&mic, &sys), 2)
        } else {
            (mic, 1)
        };
        audio::write_wav(&samples, &wav_path, sr, channels)
    })
    .await
    .map_err(|e| e.to_string())??;

    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn list_recordings(app: AppHandle) -> Result<Vec<RecordingEntry>, String> {
    let settings = get_stt_settings(app.clone()).await;
    let dir = recordings_dir(&app, &settings);

    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut read_dir = tokio::fs::read_dir(&dir).await.map_err(|e| e.to_string())?;
    let mut result = Vec::new();

    while let Some(entry) = read_dir.next_entry().await.map_err(|e| e.to_string())? {
        let folder = entry.path();
        if !folder.is_dir() || !folder.join("recording.wav").exists() {
            continue;
        }
        let name = folder
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let transcript = tokio::fs::read(folder.join("transcript.json"))
            .await
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());

        if let Some(ref tr) = transcript {
            let md_path = folder.join("transcript.md");
            if !md_path.exists() {
                let md = crate::commands::stt::build_transcript_markdown(
                    &name,
                    &folder.join("recording.wav"),
                    tr,
                );
                let _ = tokio::fs::write(&md_path, &md).await;
                if let Some(custom_vault) = &settings.transcripts_dir {
                    let vault_dir = std::path::PathBuf::from(custom_vault);
                    if vault_dir != folder {
                        let _ = tokio::fs::create_dir_all(&vault_dir).await;
                        let dest = vault_dir.join(format!("{name}.md"));
                        let _ = tokio::fs::write(dest, &md).await;
                    }
                }
            }
        }
        // Il sidecar d'errore non ha motivo di esistere insieme a un transcript
        // valido (vedi `transcribe_recording`): lo si legge solo in sua assenza.
        let error = if transcript.is_none() {
            tokio::fs::read(folder.join("transcript_error.json"))
                .await
                .ok()
                .and_then(|b| serde_json::from_slice::<TranscriptError>(&b).ok())
                .map(|e| e.message)
        } else {
            None
        };
        result.push(RecordingEntry {
            folder_path: folder.to_string_lossy().into_owned(),
            name,
            transcript,
            error,
        });
    }

    result.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(result)
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::process::Command;

    /// Genera una fixture audio compressa con gli strumenti nativi macOS (come lo
    /// smoke test STT del progetto), poi verifica che `decode_audio` la porti a
    /// campioni f32 — copre il ramo rischioso nuovo (symphonia mp4/m4a/AAC), non
    /// il WAV che hound già gestiva.
    fn decode_fixture(afconvert_fmt: &str, ext: &str) -> (Vec<f32>, u16, u32) {
        // Path unici per formato: i test girano in parallelo e condividere la
        // fixture aiff causava collisioni di scrittura (afconvert errore -54).
        let dir = std::env::temp_dir();
        let aiff = dir.join(format!("earwig_decode_fix_{ext}.aiff"));
        let out = dir.join(format!("earwig_decode_fix.{ext}"));
        Command::new("say")
            .args(["-v", "Alice", "-o"])
            .arg(&aiff)
            .arg("prova di decodifica audio")
            .status()
            .expect("say non disponibile");
        Command::new("afconvert")
            .args(["-f", afconvert_fmt, "-d", "aac"])
            .arg(&aiff)
            .arg(&out)
            .status()
            .expect("afconvert non disponibile");
        decode_audio(&out).expect("decode_audio ha fallito")
    }

    #[test]
    fn decodes_m4a_to_samples() {
        let (samples, channels, rate) = decode_fixture("m4af", "m4a");
        assert!(!samples.is_empty(), "nessun campione decodificato da m4a");
        assert!(channels >= 1);
        assert!(rate > 0);
    }

    #[test]
    fn decodes_mp4_to_samples() {
        let (samples, _, _) = decode_fixture("mp4f", "mp4");
        assert!(!samples.is_empty(), "nessun campione decodificato da mp4");
    }
}
