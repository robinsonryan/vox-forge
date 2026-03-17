//! Audio capture using cpal.
//!
//! Platform-agnostic audio recording. cpal handles the backend
//! selection (ALSA/PipeWire on Linux, WASAPI on Windows).

use std::collections::VecDeque;
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

/// Shared state between the always-on audio stream and the capture API.
struct CaptureState {
    /// Circular buffer holding the most recent pre-roll samples.
    pre_roll: VecDeque<f32>,
    /// Maximum number of samples to keep in the pre-roll buffer.
    pre_roll_capacity: usize,
    /// Recording buffer — samples are appended here while recording.
    recording_buf: Vec<f32>,
    /// Whether we are currently recording.
    is_recording: bool,
}

/// Audio capture manager with always-on pre-roll buffer.
///
/// Starts a background audio stream on creation that continuously fills a
/// circular pre-roll buffer. When `start_recording()` is called, the pre-roll
/// samples are prepended to the recording so speech that started just before
/// the hotkey press is captured.
pub struct AudioCapture {
    _stream: cpal::Stream,
    state: Arc<Mutex<CaptureState>>,
    recording_flag: Arc<AtomicBool>,
    native_sample_rate: u32,
    target_sample_rate: u32,
}

impl AudioCapture {
    /// Create a new `AudioCapture` with the specified device (or default).
    ///
    /// Immediately starts a background audio stream that feeds the pre-roll
    /// buffer. `pre_roll_ms` controls how many milliseconds of audio are
    /// kept for prepending when recording starts.
    pub fn new(device_name: Option<&str>, pre_roll_ms: u64) -> Result<Self> {
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

        let native_sample_rate = config.sample_rate().0;
        let target_sample_rate = 16_000u32;

        // Pre-roll capacity in native-rate samples
        #[allow(clippy::cast_possible_truncation)]
        let pre_roll_capacity = (u64::from(native_sample_rate) * pre_roll_ms / 1000) as usize;

        let state = Arc::new(Mutex::new(CaptureState {
            pre_roll: VecDeque::with_capacity(pre_roll_capacity),
            pre_roll_capacity,
            recording_buf: Vec::new(),
            is_recording: false,
        }));

        let recording_flag = Arc::new(AtomicBool::new(false));

        // Build the always-on background stream
        let state_clone = Arc::clone(&state);
        let flag_clone = Arc::clone(&recording_flag);

        let stream_config = cpal::StreamConfig {
            channels: 1,
            sample_rate: cpal::SampleRate(native_sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let err_fn = |err: cpal::StreamError| {
            tracing::error!("Audio stream error: {err}");
        };

        let stream = device
            .build_input_stream(
                &stream_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if flag_clone.load(Ordering::Relaxed) {
                        // Recording mode: append to recording buffer
                        if let Ok(mut s) = state_clone.lock() {
                            s.recording_buf.extend_from_slice(data);
                        }
                    } else {
                        // Idle mode: feed circular pre-roll buffer
                        if let Ok(mut s) = state_clone.lock() {
                            for &sample in data {
                                if s.pre_roll.len() >= s.pre_roll_capacity {
                                    s.pre_roll.pop_front();
                                }
                                s.pre_roll.push_back(sample);
                            }
                        }
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| Error::Audio(format!("Failed to build input stream: {e}")))?;

        stream
            .play()
            .map_err(|e| Error::Audio(format!("Failed to start stream: {e}")))?;

        tracing::info!(
            "Audio capture started (pre-roll: {pre_roll_ms}ms, {pre_roll_capacity} samples)"
        );

        Ok(Self {
            _stream: stream,
            state,
            recording_flag,
            native_sample_rate,
            target_sample_rate,
        })
    }

    /// Measure the ambient noise floor by sampling the pre-roll buffer.
    ///
    /// Waits for `duration_ms` to let the buffer fill, then computes the RMS
    /// energy in dB. Returns the noise floor level.
    pub fn calibrate_noise_floor(&self, duration_ms: u64) -> f32 {
        // Wait for the buffer to accumulate ambient noise
        std::thread::sleep(std::time::Duration::from_millis(duration_ms));

        let samples: Vec<f32> = self
            .state
            .lock()
            .map(|s| s.pre_roll.iter().copied().collect())
            .unwrap_or_default();

        if samples.is_empty() {
            tracing::warn!("No audio samples captured during calibration");
            return -40.0;
        }

        // Compute RMS in 50ms windows and take the median to ignore transient spikes
        let window_size = (self.native_sample_rate as usize * 50) / 1000;
        let mut window_dbs: Vec<f32> = samples
            .chunks(window_size)
            .filter(|chunk| chunk.len() >= window_size / 2)
            .map(|chunk| {
                let sum_sq: f32 = chunk.iter().map(|s| s * s).sum();
                #[allow(clippy::cast_precision_loss)]
                let rms = (sum_sq / chunk.len() as f32).sqrt();
                super::amplitude_to_db(rms)
            })
            .collect();

        if window_dbs.is_empty() {
            tracing::warn!("Not enough audio for calibration");
            return -40.0;
        }

        window_dbs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        window_dbs[window_dbs.len() / 2]
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

    /// Start recording. Drains the pre-roll buffer and begins capturing.
    /// Returns a handle to stop recording and retrieve the audio.
    pub fn start_recording(&self) -> Result<RecordingHandle> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| Error::Audio(format!("Failed to lock capture state: {e}")))?;

        // Drain pre-roll into the recording buffer
        state.recording_buf.clear();
        let pre_roll: Vec<f32> = state.pre_roll.drain(..).collect();
        let pre_roll_samples = pre_roll.len();
        state.recording_buf.extend(pre_roll);
        state.is_recording = true;
        drop(state);

        // Set the atomic flag so the stream callback switches to recording mode
        self.recording_flag.store(true, Ordering::Relaxed);

        tracing::debug!("Recording started with {pre_roll_samples} pre-roll samples");

        Ok(RecordingHandle {
            state: Arc::clone(&self.state),
            recording_flag: Arc::clone(&self.recording_flag),
            native_sample_rate: self.native_sample_rate,
            target_sample_rate: self.target_sample_rate,
        })
    }

    /// The native sample rate of the selected device.
    pub fn sample_rate(&self) -> u32 {
        self.native_sample_rate
    }
}

/// Handle to an in-progress recording.
pub struct RecordingHandle {
    state: Arc<Mutex<CaptureState>>,
    recording_flag: Arc<AtomicBool>,
    native_sample_rate: u32,
    target_sample_rate: u32,
}

impl RecordingHandle {
    /// Get a snapshot of the most recent samples for VAD processing.
    ///
    /// Only copies the last `max_samples` samples instead of the entire buffer,
    /// avoiding O(n) clones on every poll for long recordings.
    pub fn tail_samples(&self, max_samples: usize) -> Vec<f32> {
        self.state
            .lock()
            .map(|s| {
                let buf = &s.recording_buf;
                let start = buf.len().saturating_sub(max_samples);
                buf[start..].to_vec()
            })
            .unwrap_or_default()
    }

    /// Stop recording and return the audio buffer resampled to 16 kHz.
    pub fn stop(self) -> Result<AudioBuffer> {
        // Switch back to pre-roll mode
        self.recording_flag.store(false, Ordering::Relaxed);

        // Lock guarantees any in-progress callback has finished writing.
        let mut state = self
            .state
            .lock()
            .map_err(|e| Error::Audio(format!("Failed to get audio buffer: {e}")))?;

        state.is_recording = false;
        let samples = std::mem::take(&mut state.recording_buf);
        drop(state);

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
        self.state
            .lock()
            .map(|s| s.recording_buf.len())
            .unwrap_or(0)
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
