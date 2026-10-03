//! Standalone Parakeet timing harness — no GUI or system dependencies.
//!
//! Loads the Parakeet int8 model via transcribe-rs, transcribes one WAV three
//! times, and prints load time, per-run wall time, and real-time factor.
//!
//! ```text
//! cargo build --release --example parakeet_bench
//! target/release/examples/parakeet_bench <model_dir> <wav>
//! ```
//!
//! The WAV must be 16 kHz mono 16-bit PCM.

use std::path::PathBuf;
use std::time::Instant;

use transcribe_rs::onnx::Quantization;
use transcribe_rs::onnx::parakeet::{ParakeetModel, ParakeetParams};

const RUNS: usize = 3;
const SAMPLE_RATE: f64 = 16_000.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(model_dir), Some(wav)) = (args.next(), args.next()) else {
        eprintln!("usage: parakeet_bench <model_dir> <wav>");
        std::process::exit(2);
    };
    let model_dir = PathBuf::from(model_dir);
    let wav = PathBuf::from(wav);

    let samples = transcribe_rs::audio::read_wav_samples(&wav)?;
    #[allow(clippy::cast_precision_loss)]
    let audio_secs = samples.len() as f64 / SAMPLE_RATE;
    println!("audio: {} ({audio_secs:.2}s)", wav.display());

    let load_start = Instant::now();
    let mut model = ParakeetModel::load(&model_dir, &Quantization::Int8)?;
    println!("load: {:.2?}", load_start.elapsed());

    let params = ParakeetParams::default();
    for run in 1..=RUNS {
        let start = Instant::now();
        let result = model.transcribe_with(&samples, &params)?;
        let wall = start.elapsed().as_secs_f64();
        println!(
            "run {run}: wall {wall:.3}s | RTF {:.3} | {:.1}x realtime",
            wall / audio_secs,
            audio_secs / wall
        );
        if run == 1 {
            println!("text: {}", result.text.trim());
        }
    }

    Ok(())
}
