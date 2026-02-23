//! Voice Activity Detection using RMS energy.
//!
//! Analyses audio samples in 50 ms windows and determines whether
//! speech is present based on an energy threshold in dB.

use std::time::Duration;

/// Voice Activity Detector using RMS energy.
pub struct VoiceActivityDetector {
    threshold_db: f32,
    silence_timeout: Duration,
    ignore_initial: Duration,
    window_size_samples: usize,
}

/// Result of a VAD analysis pass.
pub struct VadResult {
    /// Whether the most recent window contains speech.
    pub is_speech: bool,
    /// dB level of the most recent window.
    pub current_db: f32,
    /// Peak dB across all windows.
    pub peak_db: f32,
    /// Duration of trailing silence.
    pub silence_duration: Duration,
    /// Whether the recording should auto-stop.
    pub should_stop: bool,
}

impl VoiceActivityDetector {
    /// Create a new VAD.
    ///
    /// * `threshold_db`     -- energy level below which audio is silence (e.g. -40.0)
    /// * `silence_timeout_s` -- seconds of continuous silence before auto-stop
    /// * `sample_rate`       -- sample rate of the audio being analysed
    pub fn new(threshold_db: f32, silence_timeout_s: f32, sample_rate: u32) -> Self {
        let window_ms = 50; // 50 ms sliding windows
        let window_size_samples = (sample_rate as usize * window_ms) / 1000;

        Self {
            threshold_db,
            silence_timeout: Duration::from_secs_f32(silence_timeout_s),
            ignore_initial: Duration::from_secs(1),
            window_size_samples,
        }
    }

    /// Analyse audio samples and determine voice activity.
    ///
    /// `recording_duration` is how long the recording has been running;
    /// this is used to suppress auto-stop during the initial grace period.
    pub fn analyze(&self, samples: &[f32], recording_duration: Duration) -> VadResult {
        if samples.is_empty() {
            return VadResult {
                is_speech: false,
                current_db: -100.0,
                peak_db: -100.0,
                silence_duration: Duration::ZERO,
                should_stop: false,
            };
        }

        let mut peak_db = f32::NEG_INFINITY;
        let mut current_db = -100.0_f32;
        let mut silence_windows = 0u32;
        let mut total_windows = 0u32;

        for chunk in samples.chunks(self.window_size_samples) {
            if chunk.len() < self.window_size_samples / 2 {
                continue; // Skip very small trailing chunks
            }

            let rms = compute_rms(chunk);
            let db = rms_to_db(rms);

            peak_db = peak_db.max(db);
            current_db = db;
            total_windows += 1;

            if db < self.threshold_db {
                silence_windows += 1;
            } else {
                silence_windows = 0; // Reset on speech
            }
        }

        // Handle the case where no full windows were processed.
        if total_windows == 0 {
            return VadResult {
                is_speech: false,
                current_db: -100.0,
                peak_db: -100.0,
                silence_duration: Duration::ZERO,
                should_stop: false,
            };
        }

        let silence_duration = Duration::from_millis(u64::from(silence_windows) * 50);

        let is_speech = current_db >= self.threshold_db;

        let should_stop =
            recording_duration > self.ignore_initial && silence_duration >= self.silence_timeout;

        VadResult {
            is_speech,
            current_db,
            peak_db,
            silence_duration,
            should_stop,
        }
    }
}

/// Compute RMS (root mean square) of a sample slice.
#[allow(clippy::cast_precision_loss)]
fn compute_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

/// Convert an RMS value to decibels.
fn rms_to_db(rms: f32) -> f32 {
    if rms <= 0.0 {
        return -100.0;
    }
    20.0 * rms.log10()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── RMS / dB helpers ───────────────────────────────────────────

    #[test]
    fn rms_of_silence_is_zero() {
        let samples = vec![0.0_f32; 800];
        assert!((compute_rms(&samples) - 0.0).abs() < 1e-7);
    }

    #[test]
    fn rms_of_constant_signal() {
        let samples = vec![0.5_f32; 800];
        let rms = compute_rms(&samples);
        assert!(
            (rms - 0.5).abs() < 1e-5,
            "RMS of constant 0.5 should be 0.5, got {rms}"
        );
    }

    #[test]
    fn rms_of_empty_is_zero() {
        assert!((compute_rms(&[]) - 0.0).abs() < 1e-7);
    }

    #[test]
    fn db_of_zero_rms_is_floor() {
        assert_eq!(rms_to_db(0.0), -100.0);
    }

    #[test]
    fn db_of_unity_is_zero() {
        let db = rms_to_db(1.0);
        assert!(db.abs() < 1e-5, "RMS 1.0 should be 0 dB, got {db}");
    }

    #[test]
    fn db_of_half_is_about_minus_six() {
        let db = rms_to_db(0.5);
        // 20 * log10(0.5) ~= -6.02
        assert!(
            (db - (-6.0206)).abs() < 0.01,
            "RMS 0.5 should be ~-6 dB, got {db}"
        );
    }

    #[test]
    fn db_of_negative_rms_is_floor() {
        assert_eq!(rms_to_db(-0.1), -100.0);
    }

    // ── VoiceActivityDetector tests ────────────────────────────────

    fn make_vad() -> VoiceActivityDetector {
        // -40 dB threshold, 2s silence timeout, 16 kHz
        VoiceActivityDetector::new(-40.0, 2.0, 16000)
    }

    #[test]
    fn empty_samples_returns_no_speech() {
        let vad = make_vad();
        let result = vad.analyze(&[], Duration::from_secs(5));
        assert!(!result.is_speech);
        assert!(!result.should_stop);
        assert_eq!(result.current_db, -100.0);
        assert_eq!(result.peak_db, -100.0);
    }

    #[test]
    fn pure_silence_detected_as_silence() {
        let vad = make_vad();
        // 3 seconds of silence at 16 kHz
        let samples = vec![0.0_f32; 16000 * 3];
        let result = vad.analyze(&samples, Duration::from_secs(5));
        assert!(!result.is_speech);
        assert!(result.current_db < -40.0);
    }

    #[test]
    fn loud_signal_detected_as_speech() {
        let vad = make_vad();
        // Constant signal at 0.5 amplitude -> ~-6 dB, well above -40 dB
        let samples = vec![0.5_f32; 16000];
        let result = vad.analyze(&samples, Duration::from_secs(5));
        assert!(result.is_speech);
        assert!(result.current_db > -40.0);
        assert!(result.peak_db > -40.0);
    }

    #[test]
    fn should_stop_after_silence_timeout() {
        let vad = make_vad(); // 2s timeout
        // 3 seconds of silence at 16 kHz
        let samples = vec![0.0_f32; 16000 * 3];
        // Recording has been going for 5 seconds (past ignore period)
        let result = vad.analyze(&samples, Duration::from_secs(5));
        assert!(
            result.should_stop,
            "Should auto-stop after 3s silence with 2s timeout"
        );
    }

    #[test]
    fn should_not_stop_during_initial_ignore_period() {
        let vad = make_vad(); // ignore_initial = 1s
        // 3 seconds of silence
        let samples = vec![0.0_f32; 16000 * 3];
        // Recording has only been going for 0.5s (inside ignore period)
        let result = vad.analyze(&samples, Duration::from_millis(500));
        assert!(
            !result.should_stop,
            "Should NOT auto-stop during initial ignore period"
        );
    }

    #[test]
    fn should_not_stop_when_silence_shorter_than_timeout() {
        let vad = make_vad(); // 2s timeout
        // 1 second of silence
        let samples = vec![0.0_f32; 16000];
        let result = vad.analyze(&samples, Duration::from_secs(5));
        assert!(
            !result.should_stop,
            "Should NOT stop when silence < timeout"
        );
    }

    #[test]
    fn speech_followed_by_short_silence_resets_counter() {
        let vad = make_vad(); // -40 dB threshold, 2s timeout

        // Build: 2s of speech, then 1s of silence
        // At 16 kHz with 50ms windows = 800 samples/window
        let mut samples = Vec::new();
        // 2 seconds of speech (amplitude 0.5)
        samples.extend(vec![0.5_f32; 16000 * 2]);
        // 1 second of silence
        samples.extend(vec![0.0_f32; 16000]);

        let result = vad.analyze(&samples, Duration::from_secs(5));
        // Trailing silence is only ~1s, less than 2s timeout
        assert!(
            !result.should_stop,
            "Should NOT stop: only 1s silence after speech"
        );
        // The silence counter should reflect ~1s
        assert!(
            result.silence_duration >= Duration::from_millis(900),
            "Trailing silence should be ~1s, got {:?}",
            result.silence_duration
        );
        assert!(
            result.silence_duration <= Duration::from_millis(1100),
            "Trailing silence should be ~1s, got {:?}",
            result.silence_duration
        );
    }

    #[test]
    fn peak_db_captures_loudest_window() {
        let vad = make_vad();

        // 1s of quiet (0.001 amplitude) then 1s of loud (0.8 amplitude)
        let mut samples = Vec::new();
        samples.extend(vec![0.001_f32; 16000]);
        samples.extend(vec![0.8_f32; 16000]);

        let result = vad.analyze(&samples, Duration::from_secs(5));
        // Peak should reflect the loud section (~-1.9 dB)
        assert!(
            result.peak_db > -5.0,
            "Peak should capture the loud section, got {}",
            result.peak_db
        );
    }

    #[test]
    fn tiny_chunk_below_half_window_is_skipped() {
        let vad = make_vad();
        // Window size at 16 kHz = 800 samples. Provide 300 (< 400 = half).
        let samples = vec![0.5_f32; 300];
        let result = vad.analyze(&samples, Duration::from_secs(5));
        // Too small to form a valid window; treated as no data
        assert!(!result.is_speech);
        assert_eq!(result.current_db, -100.0);
    }
}
