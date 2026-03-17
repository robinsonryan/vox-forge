//! Audio capture using cpal.
//!
//! Platform-agnostic audio recording. cpal handles the backend
//! selection (ALSA/PipeWire on Linux, WASAPI on Windows).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::error::{Error, Result};

/// Information about an audio input device.
#[derive(Debug, Clone)]
pub struct AudioDevice {
    pub name: String,
    pub is_default: bool,
}

/// Audio capture manager.
pub struct AudioCapture {
    device: cpal::Device,
    sample_rate: u32,
}

impl AudioCapture {
    /// Create a new `AudioCapture` with the specified device (or default).
    pub fn new(device_name: Option<&str>) -> Result<Self> {
        let host = cpal::default_host();

        let device = if let Some(name) = device_name {
            host.input_devices()
                .map_err(|e| Error::Audio(format!("Failed to enumerate devices: {e}")))?
                .find(|d| d.name().map(|n| n == name).unwrap_or(false))
                .ok_or_else(|| Error::Audio(format!("Audio device '{name}' not found")))?
        } else {
            host.default_input_device()
                .ok_or_else(|| Error::Audio("No default input device found".to_string()))?
        };

        let config = device
            .default_input_config()
            .map_err(|e| Error::Audio(format!("No supported input config: {e}")))?;

        Ok(Self {
            device,
            sample_rate: config.sample_rate().0,
        })
    }

    /// List available audio input devices.
    pub fn list_devices() -> Result<Vec<AudioDevice>> {
        let host = cpal::default_host();
        let default_name = host.default_input_device().and_then(|d| d.name().ok());

        let mut devices = Vec::new();
        for device in host
            .input_devices()
            .map_err(|e| Error::Audio(format!("Failed to enumerate devices: {e}")))?
        {
            if let Ok(name) = device.name() {
                let is_default = default_name.as_deref() == Some(&name);
                devices.push(AudioDevice { name, is_default });
            }
        }
        Ok(devices)
    }

    /// Record audio. Returns a handle that collects samples.
    /// Call `stop()` on the handle to get the audio buffer.
    pub fn start_recording(&self) -> Result<RecordingHandle> {
        let buffer: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
        let buffer_clone = Arc::clone(&buffer);
        let recording = Arc::new(AtomicBool::new(true));
        let recording_clone = Arc::clone(&recording);

        let native_sample_rate = self.sample_rate;
        let target_sample_rate = 16_000u32;

        let config = cpal::StreamConfig {
            channels: 1,
            sample_rate: cpal::SampleRate(native_sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let err_fn = |err: cpal::StreamError| {
            tracing::error!("Audio stream error: {err}");
        };

        let stream = self
            .device
            .build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if !recording_clone.load(Ordering::Relaxed) {
                        return;
                    }
                    if let Ok(mut buf) = buffer_clone.lock() {
                        buf.extend_from_slice(data);
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| Error::Audio(format!("Failed to build input stream: {e}")))?;

        stream
            .play()
            .map_err(|e| Error::Audio(format!("Failed to start stream: {e}")))?;

        Ok(RecordingHandle {
            _stream: stream,
            buffer,
            recording,
            native_sample_rate,
            target_sample_rate,
        })
    }

    /// The native sample rate of the selected device.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

/// Handle to an in-progress recording.
pub struct RecordingHandle {
    _stream: cpal::Stream,
    buffer: Arc<Mutex<Vec<f32>>>,
    recording: Arc<AtomicBool>,
    native_sample_rate: u32,
    target_sample_rate: u32,
}

impl RecordingHandle {
    /// Get a snapshot of current samples (useful for VAD processing).
    pub fn current_samples(&self) -> Vec<f32> {
        self.buffer.lock().map(|b| b.clone()).unwrap_or_default()
    }

    /// Stop recording and return the audio buffer resampled to 16 kHz.
    pub fn stop(self) -> Result<AudioBuffer> {
        self.recording.store(false, Ordering::Relaxed);
        // Small delay so the stream callback can observe the flag.
        std::thread::sleep(std::time::Duration::from_millis(50));

        let samples = self
            .buffer
            .lock()
            .map_err(|e| Error::Audio(format!("Failed to get audio buffer: {e}")))?
            .clone();

        let samples = if self.native_sample_rate == self.target_sample_rate {
            samples
        } else {
            resample(&samples, self.native_sample_rate, self.target_sample_rate)
        };

        let duration_ms = (samples.len() as u64 * 1000) / u64::from(self.target_sample_rate);

        Ok(AudioBuffer {
            samples,
            sample_rate: self.target_sample_rate,
            duration_ms,
        })
    }

    /// The number of samples recorded so far.
    pub fn sample_count(&self) -> usize {
        self.buffer.lock().map(|b| b.len()).unwrap_or(0)
    }
}

/// Recorded audio buffer ready for transcription.
pub struct AudioBuffer {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub duration_ms: u64,
}

impl AudioBuffer {
    /// Peak amplitude across all samples.
    pub fn peak_amplitude(&self) -> f32 {
        self.samples.iter().map(|s| s.abs()).fold(0.0_f32, f32::max)
    }

    /// Peak amplitude in decibels.
    pub fn peak_db(&self) -> f32 {
        super::amplitude_to_db(self.peak_amplitude())
    }

    /// Encode as WAV bytes (16-bit PCM mono) for cloud API upload.
    pub fn to_wav_bytes(&self) -> Result<Vec<u8>> {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: self.sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(&mut cursor, spec)
                .map_err(|e| Error::Audio(format!("WAV encoding error: {e}")))?;
            for &sample in &self.samples {
                #[allow(clippy::cast_possible_truncation)]
                let s = (sample * 32_767.0).clamp(-32_768.0, 32_767.0) as i16;
                writer
                    .write_sample(s)
                    .map_err(|e| Error::Audio(format!("WAV sample write error: {e}")))?;
            }
            writer
                .finalize()
                .map_err(|e| Error::Audio(format!("WAV finalize error: {e}")))?;
        }
        Ok(cursor.into_inner())
    }
}

/// Linear interpolation resampling from `from_rate` to `to_rate`.
#[allow(clippy::cast_precision_loss)]
fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || samples.is_empty() {
        return samples.to_vec();
    }

    let ratio = f64::from(from_rate) / f64::from(to_rate);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let output_len = (samples.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_idx = i as f64 * ratio;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let idx = src_idx as usize;
        #[allow(clippy::cast_possible_truncation)]
        let frac = (src_idx - idx as f64) as f32;

        let sample = if idx + 1 < samples.len() {
            samples[idx] * (1.0 - frac) + samples[idx + 1] * frac
        } else if idx < samples.len() {
            samples[idx]
        } else {
            0.0
        };
        output.push(sample);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── resample tests ─────────────────────────────────────────────

    #[test]
    fn resample_same_rate_returns_identical() {
        let input = vec![0.1, 0.2, 0.3, 0.4];
        let output = resample(&input, 16000, 16000);
        assert_eq!(input, output);
    }

    #[test]
    fn resample_empty_input() {
        let output = resample(&[], 48000, 16000);
        assert!(output.is_empty());
    }

    #[test]
    fn resample_downsample_48k_to_16k() {
        // 48 samples at 48 kHz = 1 ms -> should produce ~16 samples at 16 kHz
        let input: Vec<f32> = (0..48).map(|i| i as f32 / 48.0).collect();
        let output = resample(&input, 48000, 16000);
        assert_eq!(output.len(), 16);
        // First sample should be 0.0
        assert!((output[0] - 0.0).abs() < 1e-5);
        // Values should be monotonically increasing
        for w in output.windows(2) {
            assert!(w[1] >= w[0], "Expected monotonic increase");
        }
    }

    #[test]
    fn resample_upsample_16k_to_48k() {
        let input: Vec<f32> = (0..16).map(|i| i as f32 / 16.0).collect();
        let output = resample(&input, 16000, 48000);
        assert_eq!(output.len(), 48);
        // First sample preserved
        assert!((output[0] - 0.0).abs() < 1e-5);
        // Should be monotonically increasing
        for w in output.windows(2) {
            assert!(w[1] >= w[0], "Expected monotonic increase");
        }
    }

    #[test]
    fn resample_preserves_constant_signal() {
        let input = vec![0.5_f32; 480];
        let output = resample(&input, 48000, 16000);
        for s in &output {
            assert!(
                (s - 0.5).abs() < 1e-5,
                "Constant signal should be preserved"
            );
        }
    }

    // ── AudioBuffer WAV encoding tests ─────────────────────────────

    #[test]
    fn wav_bytes_starts_with_riff_header() {
        let buf = AudioBuffer {
            samples: vec![0.0; 160],
            sample_rate: 16000,
            duration_ms: 10,
        };
        let wav = buf.to_wav_bytes().expect("WAV encoding should succeed");
        assert!(wav.len() > 44, "WAV should have header + data");
        assert_eq!(&wav[..4], b"RIFF", "Should start with RIFF header");
        assert_eq!(&wav[8..12], b"WAVE", "Should contain WAVE format");
    }

    #[test]
    fn wav_bytes_roundtrip_via_hound() {
        let samples = vec![0.0, 0.5, -0.5, 1.0, -1.0];
        let buf = AudioBuffer {
            samples: samples.clone(),
            sample_rate: 16000,
            duration_ms: 0,
        };
        let wav = buf.to_wav_bytes().expect("WAV encoding should succeed");

        // Read back with hound
        let cursor = std::io::Cursor::new(wav);
        let mut reader = hound::WavReader::new(cursor).expect("WAV should be valid");
        let spec = reader.spec();
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.sample_rate, 16000);
        assert_eq!(spec.bits_per_sample, 16);

        let decoded: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
        assert_eq!(decoded.len(), samples.len());
        // Check that values round-trip reasonably (within 16-bit quantization)
        for (orig, &decoded_s) in samples.iter().zip(&decoded) {
            let expected = (orig * 32_767.0).clamp(-32_768.0, 32_767.0) as i16;
            assert_eq!(decoded_s, expected);
        }
    }

    #[test]
    fn wav_bytes_empty_buffer() {
        let buf = AudioBuffer {
            samples: vec![],
            sample_rate: 16000,
            duration_ms: 0,
        };
        let wav = buf.to_wav_bytes().expect("Empty WAV should still succeed");
        // Just a header, no sample data
        assert!(!wav.is_empty());
        assert_eq!(&wav[..4], b"RIFF");
    }

    #[test]
    fn wav_bytes_clamps_out_of_range() {
        // Samples exceeding [-1, 1] should be clamped
        let buf = AudioBuffer {
            samples: vec![2.0, -2.0],
            sample_rate: 16000,
            duration_ms: 0,
        };
        let wav = buf
            .to_wav_bytes()
            .expect("Should not error on out-of-range");
        let cursor = std::io::Cursor::new(wav);
        let mut reader = hound::WavReader::new(cursor).unwrap();
        let decoded: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
        assert_eq!(decoded[0], 32_767); // clamped positive
        assert_eq!(decoded[1], -32_768); // clamped negative
    }
}
