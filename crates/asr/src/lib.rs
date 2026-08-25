use anyhow::{anyhow, Result};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Reads a WAV file, downmixes to mono, and naively resamples to 16kHz —
/// the format whisper.cpp requires. Good enough for a first correctness
/// check; a proper resampler (e.g. `rubato`) can replace this later.
pub fn load_wav_as_16k_mono(path: &str) -> Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();

    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<std::result::Result<_, _>>()?,
        hound::SampleFormat::Int => reader
            .samples::<i32>()
            .map(|s| s.map(|v| v as f32 / i32::MAX as f32))
            .collect::<std::result::Result<_, _>>()?,
    };

    let mono: Vec<f32> = if spec.channels > 1 {
        raw.chunks(spec.channels as usize)
            .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
            .collect()
    } else {
        raw
    };

    let target_rate = 16_000u32;
    if spec.sample_rate == target_rate {
        return Ok(mono);
    }
    let ratio = target_rate as f64 / spec.sample_rate as f64;
    let out_len = (mono.len() as f64 * ratio) as usize;
    let mut resampled = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_pos = i as f64 / ratio;
        let idx = src_pos.floor() as usize;
        let frac = (src_pos - idx as f64) as f32;
        let a = *mono.get(idx).unwrap_or(&0.0);
        let b = *mono.get(idx + 1).unwrap_or(&a);
        resampled.push(a + (b - a) * frac);
    }
    Ok(resampled)
}

/// Transcribes 16kHz mono f32 samples using a whisper.cpp GGML model
/// (e.g. ggml-base.en.bin) and returns the full text.
pub fn transcribe(model_path: &str, samples: &[f32]) -> Result<String> {
    let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
        .map_err(|e| anyhow!("failed to load model: {e:?}"))?;
    let mut state = ctx
        .create_state()
        .map_err(|e| anyhow!("failed to create state: {e:?}"))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_print_progress(false);
    params.set_print_special(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    state
        .full(params, samples)
        .map_err(|e| anyhow!("transcription failed: {e:?}"))?;

    let num_segments = state
        .full_n_segments()
        .map_err(|e| anyhow!("failed to get segment count: {e:?}"))?;

    let mut text = String::new();
    for i in 0..num_segments {
        let segment = state
            .full_get_segment_text(i)
            .map_err(|e| anyhow!("failed to get segment text: {e:?}"))?;
        text.push_str(&segment);
    }

    Ok(text.trim().to_string())
}