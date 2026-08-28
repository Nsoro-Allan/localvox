use anyhow::{anyhow, Result};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub fn load_wav_as_16k_mono(path: &str) -> Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<std::result::Result<_, _>>()?,
        hound::SampleFormat::Int => reader.samples::<i32>().map(|s| s.map(|v| v as f32 / i32::MAX as f32)).collect::<std::result::Result<_, _>>()?,
    };
    let mono: Vec<f32> = if spec.channels > 1 {
        raw.chunks(spec.channels as usize).map(|frame| frame.iter().sum::<f32>() / frame.len() as f32).collect()
    } else { raw };
    let target_rate = 16_000u32;
    if spec.sample_rate == target_rate { return Ok(mono); }
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

/// A loaded whisper.cpp model, ready to transcribe as many clips as you
/// want without re-reading the model file each time. Loading is the
/// expensive part; `.transcribe()` on an existing instance is cheap.
pub struct Transcriber {
    ctx: WhisperContext,
}

impl Transcriber {
    pub fn load(model_path: &str) -> Result<Self> {
        let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
            .map_err(|e| anyhow!("failed to load model: {e:?}"))?;
        Ok(Self { ctx })
    }

    pub fn transcribe(&self, samples: &[f32]) -> Result<String> {
        self.transcribe_with_prompt(samples, "")
    }

    /// Same as `transcribe`, but biases the model toward correctly
    /// recognizing specific words/phrases (names, jargon, acronyms) by
    /// feeding them as whisper.cpp's initial prompt.
    pub fn transcribe_with_prompt(&self, samples: &[f32], prompt: &str) -> Result<String> {
        let mut state = self.ctx.create_state().map_err(|e| anyhow!("failed to create state: {e:?}"))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_print_progress(false);
        params.set_print_special(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        if !prompt.is_empty() {
            params.set_initial_prompt(prompt);
        }
        let threads = std::thread::available_parallelism().map(|n| n.get() as i32).unwrap_or(4);
        params.set_n_threads(threads);

        state.full(params, samples).map_err(|e| anyhow!("transcription failed: {e:?}"))?;
        let num_segments = state.full_n_segments().map_err(|e| anyhow!("failed to get segment count: {e:?}"))?;
        let mut text = String::new();
        for i in 0..num_segments {
            let segment = state.full_get_segment_text(i).map_err(|e| anyhow!("failed to get segment text: {e:?}"))?;
            text.push_str(&segment);
        }
        Ok(text.trim().to_string())
    }
}

/// One-shot convenience wrapper — loads the model fresh every call.
/// Prefer `Transcriber` directly for more than one clip.
pub fn transcribe(model_path: &str, samples: &[f32]) -> Result<String> {
    Transcriber::load(model_path)?.transcribe(samples)
}