// WebRTC AEC3 (Acoustic Echo Canceller 3) via Sonora (pure Rust).
// Removes system audio (speaker bleed) from the microphone buffer,
// including multi-path room reflections and background noise.

use sonora::config::{
    EchoCanceller, GainController2, HighPassFilter, NoiseSuppression, NoiseSuppressionLevel,
};
use sonora::{AudioProcessing, Config, StreamConfig};

/// Cancels acoustic echo in `mic` caused by `sys` (speakers) playing back into the room.
///
/// Both `mic` and `sys` are expected to be mono audio buffers at `sample_rate`
/// (typically 16 kHz for Whisper).
///
/// Operates on 10 ms frames as required by WebRTC APM / AEC3.
pub fn cancel_echo(mic: &[f32], sys: &[f32], sample_rate: u32) -> Vec<f32> {
    if mic.is_empty() || sys.is_empty() {
        return mic.to_vec();
    }

    // 10 ms frame length (e.g. 160 samples at 16 kHz)
    let frame_len = (sample_rate as usize) / 100;
    if frame_len == 0 {
        return mic.to_vec();
    }

    let config = Config {
        echo_canceller: Some(EchoCanceller::default()),
        noise_suppression: Some(NoiseSuppression {
            level: NoiseSuppressionLevel::Moderate,
            ..Default::default()
        }),
        high_pass_filter: Some(HighPassFilter::default()),
        gain_controller2: Some(GainController2::default()),
        ..Default::default()
    };

    let stream = StreamConfig::new(sample_rate, 1);
    let mut apm = AudioProcessing::builder()
        .config(config)
        .capture_config(stream)
        .render_config(stream)
        .build();

    let total_len = mic.len().max(sys.len());
    let mut out = Vec::with_capacity(mic.len());

    let mut render_frame = vec![0.0f32; frame_len];
    let mut render_dest = vec![0.0f32; frame_len];
    let mut capture_frame = vec![0.0f32; frame_len];
    let mut capture_dest = vec![0.0f32; frame_len];

    let mut offset = 0;
    while offset < total_len {
        // 1. Far-end (system audio / loudspeaker) frame into the render path
        render_frame.fill(0.0);
        if offset < sys.len() {
            let available = (sys.len() - offset).min(frame_len);
            render_frame[..available].copy_from_slice(&sys[offset..offset + available]);
        }
        let render_src: &[&[f32]] = &[&render_frame];
        let render_out: &mut [&mut [f32]] = &mut [&mut render_dest];
        let _ = apm.process_render_f32_with_config(render_src, &stream, &stream, render_out);

        // 2. Near-end (microphone) frame into the capture path
        capture_frame.fill(0.0);
        let actual_mic_samples = if offset < mic.len() {
            let available = (mic.len() - offset).min(frame_len);
            capture_frame[..available].copy_from_slice(&mic[offset..offset + available]);
            available
        } else {
            0
        };

        if actual_mic_samples > 0 {
            let capture_src: &[&[f32]] = &[&capture_frame];
            let capture_out: &mut [&mut [f32]] = &mut [&mut capture_dest];
            let _ = apm.process_capture_f32_with_config(capture_src, &stream, &stream, capture_out);

            out.extend_from_slice(&capture_dest[..actual_mic_samples]);
        }

        offset += frame_len;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_buffers() {
        assert!(cancel_echo(&[], &[], 16000).is_empty());
        let mic = vec![0.1, 0.2];
        assert_eq!(cancel_echo(&mic, &[], 16000), mic);
        assert!(cancel_echo(&[], &[0.1, 0.2], 16000).is_empty());
    }

    #[test]
    fn test_echo_cancellation_runs() {
        let sr = 16000;
        let num_samples = 160 * 10; // 100ms
        let sys = vec![0.2f32; num_samples];
        let mic = vec![0.2f32; num_samples];

        let cleaned = cancel_echo(&mic, &sys, sr);
        assert_eq!(cleaned.len(), mic.len());
    }
}
