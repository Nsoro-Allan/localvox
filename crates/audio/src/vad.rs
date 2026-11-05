//! Voice activity detection / segmentation.
//!
//! Current implementation is energy-based (RMS). It is lightweight and
//! has no external model dependency. For production-grade results
//! comparable to Handy (Silero VAD), add an ONNX Silero backend later
//! behind the same Segmenter API.

/// Energy-based speech segmenter. Feed it mono audio in small chunks as
/// it arrives from the mic; it accumulates samples while the volume is
/// above `threshold`, and returns a completed utterance once silence
/// has lasted longer than `hangover_ms`.
pub struct Segmenter {
    threshold: f32,
    hangover_samples: usize,
    sample_rate: u32,
    buffer: Vec<f32>,
    speaking: bool,
    silence_run: usize,
    /// Minimum speech length before we are willing to emit (avoids clicks).
    min_speech_samples: usize,
}

impl Segmenter {
    pub fn new(sample_rate: u32) -> Self {
        // Slightly more aggressive defaults than the original.
        Self::with_params(sample_rate, 0.012, 450, 200)
    }

    /// `threshold` — RMS above which a frame is considered speech.
    /// `hangover_ms` — how long silence must last to end an utterance.
    /// `min_speech_ms` — ignore utterances shorter than this.
    pub fn with_params(
        sample_rate: u32,
        threshold: f32,
        hangover_ms: u64,
        min_speech_ms: u64,
    ) -> Self {
        let hangover_samples = ((hangover_ms as f64 / 1000.0) * sample_rate as f64) as usize;
        let min_speech_samples = ((min_speech_ms as f64 / 1000.0) * sample_rate as f64) as usize;
        Self {
            threshold,
            hangover_samples,
            sample_rate,
            buffer: Vec::new(),
            speaking: false,
            silence_run: 0,
            min_speech_samples,
        }
    }

    fn rms(chunk: &[f32]) -> f32 {
        if chunk.is_empty() {
            return 0.0;
        }
        let sum_sq: f32 = chunk.iter().map(|s| s * s).sum();
        (sum_sq / chunk.len() as f32).sqrt()
    }

    /// Feed a chunk of mono samples. Returns Some(utterance) when a
    /// pause long enough to mark end-of-speech is detected.
    pub fn push(&mut self, chunk: &[f32]) -> Option<Vec<f32>> {
        let is_loud = Self::rms(chunk) > self.threshold;

        if is_loud {
            self.speaking = true;
            self.silence_run = 0;
            self.buffer.extend_from_slice(chunk);
            None
        } else if self.speaking {
            self.buffer.extend_from_slice(chunk);
            self.silence_run += chunk.len();
            if self.silence_run >= self.hangover_samples {
                self.speaking = false;
                self.silence_run = 0;
                let utterance = std::mem::take(&mut self.buffer);
                if utterance.len() >= self.min_speech_samples {
                    Some(utterance)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Force-flush whatever is buffered (e.g. when the user releases the
    /// hotkey). Returns None if the buffer is too short.
    pub fn flush(&mut self) -> Option<Vec<f32>> {
        self.speaking = false;
        self.silence_run = 0;
        let utterance = std::mem::take(&mut self.buffer);
        if utterance.len() >= self.min_speech_samples {
            Some(utterance)
        } else {
            None
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.speaking = false;
        self.silence_run = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loud_chunk(len: usize) -> Vec<f32> {
        (0..len)
            .map(|i| if i % 2 == 0 { 0.5 } else { -0.5 })
            .collect()
    }
    fn silent_chunk(len: usize) -> Vec<f32> {
        vec![0.0; len]
    }

    #[test]
    fn stays_silent_through_silence() {
        let mut seg = Segmenter::new(16_000);
        for _ in 0..10 {
            assert!(seg.push(&silent_chunk(160)).is_none());
        }
    }

    #[test]
    fn emits_utterance_after_speech_then_pause() {
        let mut seg = Segmenter::new(16_000);
        for _ in 0..5 {
            assert!(seg.push(&loud_chunk(1600)).is_none());
        }
        assert!(seg.push(&silent_chunk(1600)).is_none());
        let mut result = None;
        for _ in 0..10 {
            if let Some(out) = seg.push(&silent_chunk(1600)) {
                result = Some(out);
                break;
            }
        }
        assert!(result.expect("should emit an utterance").len() >= 5 * 1600);
    }

    #[test]
    fn flush_returns_buffered_speech() {
        let mut seg = Segmenter::new(16_000);
        for _ in 0..5 {
            seg.push(&loud_chunk(1600));
        }
        let out = seg.flush().expect("flush should return speech");
        assert!(out.len() >= 5 * 1600);
    }
}
