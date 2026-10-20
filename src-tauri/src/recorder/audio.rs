//! Primitive audio condivise da recorder, AEC, diarizzazione e comandi:
//! energia, downmix, resampling, mix e scrittura WAV. Vivono qui perché
//! `rms` e la conversione a i16 erano duplicate in tre punti diversi.

use hound::{SampleFormat, WavSpec, WavWriter};
use std::path::Path;

/// Rate unico per registrazione *e* trascrizione: whisper.cpp richiede 16kHz
/// (`read_wav` in `common.cpp`), e registrare direttamente a questo rate
/// elimina il resampling a valle per le nuove registrazioni.
pub const TARGET_SAMPLE_RATE: u32 = 16_000;

#[allow(dead_code)]
pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

/// Downmix di un buffer interleaved a mono. Prende ownership per restituire il
/// buffer intatto quando è già mono, senza copiarlo.
pub fn to_mono(samples: Vec<f32>, channels: u16) -> Vec<f32> {
    if channels <= 1 {
        return samples;
    }
    samples
        .chunks(channels as usize)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Resample lineare puro-Rust: gestisce rate arbitrari in ingresso, inclusi
/// rapporti non interi (es. 44100 -> 16000). Sufficiente per parlato/ASR, non
/// per audio musicale ad alta fedeltà.
///
/// Assume un buffer **mono**: su audio interleaved interpolerebbe fra canali
/// diversi, quindi va sempre chiamato dopo `to_mono`.
pub fn resample_linear(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let out_len = (samples.len() as f64 / ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 * ratio;
        let idx = pos.floor() as usize;
        let frac = (pos - idx as f64) as f32;
        let a = samples.get(idx).copied().unwrap_or(0.0);
        let b = samples.get(idx + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }
    out
}

/// Intreccia mic e audio di sistema in un buffer stereo (mic a sinistra, sistema
/// a destra), zero-paddando il più corto. È il formato che permette a whisper di
/// fare la diarizzazione da solo: con `diarize=true` confronta l'energia dei due
/// canali e attribuisce ogni segmento a `"0"` (sinistra) o `"1"` (destra).
pub fn interleave_stereo(left: &[f32], right: &[f32]) -> Vec<f32> {
    let len = left.len().max(right.len());
    let mut out = Vec::with_capacity(len * 2);
    for i in 0..len {
        out.push(left.get(i).copied().unwrap_or(0.0).clamp(-1.0, 1.0));
        out.push(right.get(i).copied().unwrap_or(0.0).clamp(-1.0, 1.0));
    }
    out
}

/// Calcola l'RMS delle porzioni attive (parlato) ignorando i frame di silenzio
/// o rumore di fondo basso, per stimare il reale livello del parlato.
pub fn active_speech_rms(samples: &[f32], frame_size: usize) -> f32 {
    if samples.is_empty() || frame_size == 0 {
        return 0.0;
    }

    let mut frame_energies = Vec::with_capacity(samples.len() / frame_size + 1);
    for chunk in samples.chunks(frame_size) {
        if chunk.len() < frame_size / 2 {
            continue;
        }
        let energy: f32 = chunk.iter().map(|&s| s * s).sum::<f32>() / chunk.len() as f32;
        frame_energies.push(energy);
    }

    if frame_energies.is_empty() {
        return 0.0;
    }

    // Ordina le energie dei frame e prende il top 30% per stimare l'energia del parlato
    frame_energies.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let active_count = (frame_energies.len() * 30 / 100).max(1);
    let mean_active_energy =
        frame_energies[..active_count].iter().sum::<f32>() / active_count as f32;
    mean_active_energy.sqrt()
}

/// Normalizza il parlato verso un RMS target (es. 0.10 ~ -20 dBFS), applicando
/// un guadagno con limiter sul picco massimo (massimo 0.95) e un cap al guadagno (max 10x ~ +20 dB).
pub fn normalize_speech(mut samples: Vec<f32>, target_rms: f32) -> Vec<f32> {
    if samples.is_empty() || target_rms <= 0.0 {
        return samples;
    }

    // Frame da 50 ms a 16 kHz = 800 campioni
    let speech_rms = active_speech_rms(&samples, 800);
    if speech_rms <= 0.001 {
        // Audio praticamente silenzioso, evita di amplificare solo il rumore di fondo
        return samples;
    }

    let mut gain = (target_rms / speech_rms).clamp(1.0, 10.0);

    // Limiter sul picco per evitare qualsiasi clipping
    let peak = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    if peak > 0.0 && peak * gain > 0.95 {
        gain = 0.95 / peak;
    }

    if (gain - 1.0).abs() > 0.05 {
        for s in samples.iter_mut() {
            *s = (*s * gain).clamp(-1.0, 1.0);
        }
    }

    samples
}

/// Bilancia il parlato del canale mic rispetto all'audio di sistema:
/// se il mic è sensibilmente più basso del sistema, applica un guadagno per allineare
/// i livelli RMS attivi, garantendo coerenza per diarizzazione e ascolto stereo.
pub fn balance_speech_levels(mic: Vec<f32>, sys: &[f32]) -> Vec<f32> {
    if mic.is_empty() || sys.is_empty() {
        return mic;
    }

    let sys_rms = active_speech_rms(sys, 800);
    // Se il sistema ha un parlato rilevabile, usa il suo RMS come riferimento (con clamp di sicurezza [0.06, 0.18])
    let target_rms = if sys_rms > 0.01 {
        sys_rms.clamp(0.06, 0.18)
    } else {
        0.10
    };

    normalize_speech(mic, target_rms)
}

fn to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}

pub fn write_wav(
    samples: &[f32],
    path: &Path,
    sample_rate: u32,
    channels: u16,
) -> Result<(), String> {
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut writer = WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for &s in samples {
        writer.write_sample(to_i16(s)).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_active_speech_rms_empty() {
        assert_eq!(active_speech_rms(&[], 800), 0.0);
    }

    #[test]
    fn test_active_speech_rms_steady_tone() {
        let samples = vec![0.1f32; 8000];
        let rms = active_speech_rms(&samples, 800);
        assert!((rms - 0.1).abs() < 1e-4);
    }

    #[test]
    fn test_normalize_speech_boosts_quiet_audio() {
        let quiet = vec![0.02f32; 8000]; // RMS 0.02
        let normalized = normalize_speech(quiet, 0.10);
        let new_rms = active_speech_rms(&normalized, 800);
        assert!((new_rms - 0.10).abs() < 0.01);
    }

    #[test]
    fn test_normalize_speech_limits_peaks() {
        let mut samples = vec![0.02f32; 8000];
        samples[100] = 0.5; // High peak
        let normalized = normalize_speech(samples, 0.10);
        let max_peak = normalized.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(max_peak <= 0.95);
    }

    #[test]
    fn test_balance_speech_levels() {
        let quiet_mic = vec![0.02f32; 8000];
        let loud_sys = vec![0.12f32; 8000];
        let balanced = balance_speech_levels(quiet_mic, &loud_sys);
        let mic_rms = active_speech_rms(&balanced, 800);
        assert!((mic_rms - 0.12).abs() < 0.01);
    }
}
