fn main() -> anyhow::Result<()> {
    println!("Recording 5 seconds... speak now.");
    audio::record_to_wav("test.wav", 5)?;
    let samples = asr::load_wav_as_16k_mono("test.wav")?;
    println!("Transcribing...");
    let text = asr::transcribe("models/ggml-base.en.bin", &samples)?;
    println!("You said: {text}");
    Ok(())
}