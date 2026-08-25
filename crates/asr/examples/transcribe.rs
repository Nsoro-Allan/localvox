fn main() -> anyhow::Result<()> {
    let samples = asr::load_wav_as_16k_mono("test.wav")?;
    println!("Loaded {} samples, transcribing...", samples.len());
    let text = asr::transcribe("models/ggml-base.en.bin", &samples)?;
    println!("Transcript: {text}");
    Ok(())
}