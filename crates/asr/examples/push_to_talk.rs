use hotkeys::{PushToTalk, PushToTalkEvent};
use injector::Injector;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    println!("Loading model...");
    let transcriber = asr::Transcriber::load("models/ggml-base.en.bin")?;
    let mut injector = Injector::new()?;
    let ptt = PushToTalk::new()?;
    let live = audio::start_stream()?;

    println!("Hold Ctrl+F9 to talk, release to transcribe and type it wherever your cursor is.");
    println!("(Ctrl+C in this terminal to quit)");

    let mut buffer: Vec<f32> = Vec::new();
    let mut recording = false;

    loop {
        if let Some(event) = ptt.try_recv() {
            match event {
                PushToTalkEvent::Pressed => {
                    recording = true;
                    buffer.clear();
                    println!("[listening...]");
                }
                PushToTalkEvent::Released => {
                    recording = false;
                    println!("[transcribing...]");
                    let resampled = audio::resample_linear(&buffer, live.sample_rate, 16_000);
                    match transcriber.transcribe(&resampled) {
                        Ok(text) if !text.is_empty() => {
                            println!("> {text}");
                            if let Err(e) = injector.inject(&text) {
                                eprintln!("failed to type text: {e}");
                            }
                        }
                        Ok(_) => println!("(heard nothing)"),
                        Err(e) => eprintln!("transcription error: {e}"),
                    }
                }
            }
        }

        match live.rx.recv_timeout(Duration::from_millis(50)) {
            Ok(chunk) => {
                if recording {
                    let mono = audio::downmix(&chunk, live.channels);
                    buffer.extend_from_slice(&mono);
                }
            }
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    Ok(())
}