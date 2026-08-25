use audio::vad::Segmenter;
use asr::Transcriber;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    println!("Loading model...");
    let transcriber = Transcriber::load("models/ggml-base.en.bin")?;

    let live = audio::start_stream()?;
    let mut seg = Segmenter::new(live.sample_rate);

    println!("Listening... speak, then pause, and I'll transcribe each utterance.");
    println!("(Ctrl+C to stop)");

    loop {
        match live.rx.recv_timeout(Duration::from_millis(500)) {
            Ok(chunk) => {
                let mono = audio::downmix(&chunk, live.channels);
                if let Some(utterance) = seg.push(&mono) {
                    let resampled = audio::resample_linear(&utterance, live.sample_rate, 16_000);
                    match transcriber.transcribe(&resampled) {
                        Ok(text) if !text.is_empty() => println!("> {text}"),
                        Ok(_) => {}
                        Err(e) => eprintln!("transcription error: {e}"),
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    Ok(())
}