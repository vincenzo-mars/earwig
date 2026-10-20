//! Trascrive un WAV con parakeet-rs (lo stesso motore dell'app) fuori
//! dall'applicazione: serve alla diagnostica della qualità di riconoscimento
//! per confrontare il WAV processato di Earwig contro `mic_raw.wav`.
//!
//! Su input a 48kHz confronta due resample: quello lineare dell'app e uno
//! con filtro passa-basso anti-alias prima della decimazione. Se il testo
//! del secondo è molto migliore, l'aliasing del resample lineare è il
//! colpevole dell'inaccuratezza.
//!
//! Uso:
//!   cargo run --example transcribe_wav -- <percorso.wav> [cartella_modello]
//!
//! `cartella_modello` di default è la dir del modello Parakeet dell'app
//! (`~/Library/Application Support/com.vincenzomars.earwig/parakeet-tdt-0.6b-v3`).

use parakeet_rs::Transcriber;
use std::path::PathBuf;

const DEFAULT_MODEL_DIR: &str =
    "Library/Application Support/com.vincenzomars.earwig/parakeet-tdt-0.6b-v3";

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let wav = args
        .next()
        .ok_or("Uso: transcribe_wav <percorso.wav> [cartella_modello]")?;
    let model_dir = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(default_model_dir);

    let env = parakeet_rs::ort::init()
        .build()
        .map_err(|e| format!("Inizializzazione ONNX runtime fallita: {e}"))?;
    let config = parakeet_rs::ExecutionConfig::new().with_environment(env);
    let mut model = parakeet_rs::ParakeetTDT::from_pretrained(&model_dir, Some(config))
        .map_err(|e| format!("Caricamento modello da {model_dir:?}: {e}"))?;

    let mut reader =
        hound::WavReader::open(&wav).map_err(|e| format!("Impossibile aprire il file WAV: {e}"))?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max))
                .collect::<Result<Vec<f32>, _>>()
                .map_err(|e| format!("Errore decodifica campioni: {e}"))?
        }
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<Vec<f32>, _>>()
            .map_err(|e| format!("Errore decodifica campioni: {e}"))?,
    };
    println!(
        "{}: {} Hz, {} canali, {:.1}s",
        wav,
        spec.sample_rate,
        spec.channels,
        samples.len() as f64 / (spec.sample_rate as f64 * spec.channels as f64)
    );

    if spec.channels == 2 {
        let left: Vec<f32> = samples.iter().step_by(2).copied().collect();
        let right: Vec<f32> = samples.iter().skip(1).step_by(2).copied().collect();
        println!("\n== Canale L (microfono) ==");
        transcribe_mono(&mut model, &left, spec.sample_rate);
        println!("\n== Canale R (sistema) ==");
        transcribe_mono(&mut model, &right, spec.sample_rate);
    } else {
        transcribe_mono(&mut model, &samples, spec.sample_rate);
    }
    Ok(())
}

fn transcribe_mono(model: &mut parakeet_rs::ParakeetTDT, samples: &[f32], rate: u32) {
    if rate == 16000 {
        print_result(transcribe(model, samples));
        return;
    }
    println!("(resample lineare {rate} -> 16000, identico all'app)");
    print_result(transcribe(model, &resample_linear(samples, rate, 16000)));

    println!("(resample con passa-basso anti-alias {rate} -> 16000)");
    print_result(transcribe(model, &resample_lowpass(samples, rate, 16000)));
}

fn transcribe(model: &mut parakeet_rs::ParakeetTDT, samples: &[f32]) -> Result<String, String> {
    model
        .transcribe_samples(samples.to_vec(), 16000, 1, None)
        .map(|r| r.text)
        .map_err(|e| format!("Trascrizione fallita: {e}"))
}

fn print_result(result: Result<String, String>) {
    match result {
        Ok(text) => println!("{text}\n"),
        Err(e) => println!("ERRORE: {e}\n"),
    }
}

fn resample_linear(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let out_len = (samples.len() as f64 / ratio).round() as usize;
    (0..out_len)
        .map(|i| {
            let pos = i as f64 * ratio;
            let idx = pos as usize;
            let frac = (pos - idx as f64) as f32;
            let a = samples.get(idx).copied().unwrap_or(0.0);
            let b = samples.get(idx + 1).copied().unwrap_or(a);
            a + (b - a) * frac
        })
        .collect()
}

/// Decimazione con filtro passa-basso FIR (finestra Hamming, cutoff a 0.45 *
/// min(from, to)): elimina le frequenze che il resampling lineare ripiega
/// dentro la banda vocale. Dopo il filtro la decimazione usa interpolazione
/// lineare, innocua perché lo spettro sopra Nyquist è già rimosso.
fn resample_lowpass(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    let cutoff = (from_rate.min(to_rate) as f32) * 0.45;
    let nyquist = from_rate as f32 / 2.0;
    let taps = 63;
    let fc = (cutoff / nyquist).clamp(0.01, 0.99);
    let mid = (taps - 1) as f32 / 2.0;

    let mut kernel: Vec<f32> = (0..taps)
        .map(|n| {
            let x = n as f32 - mid;
            let sinc = if x.abs() < f32::EPSILON {
                1.0
            } else {
                (std::f32::consts::PI * fc * x).sin() / (std::f32::consts::PI * fc * x)
            };
            let hamming =
                0.54 - 0.46 * (2.0 * std::f32::consts::PI * n as f32 / (taps - 1) as f32).cos();
            sinc * hamming
        })
        .collect();
    let sum: f32 = kernel.iter().sum();
    for k in &mut kernel {
        *k /= sum;
    }

    let filtered: Vec<f32> = (0..samples.len())
        .map(|i| {
            let mut acc = 0.0f32;
            for k in 0..taps {
                let xi = i as isize + k as isize - (taps as isize / 2);
                if xi >= 0 && (xi as usize) < samples.len() {
                    acc += samples[xi as usize] * kernel[k];
                }
            }
            acc
        })
        .collect();

    resample_linear(&filtered, from_rate, to_rate)
}

fn default_model_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(DEFAULT_MODEL_DIR)
}
