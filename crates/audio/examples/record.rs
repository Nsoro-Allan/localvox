fn main() -> anyhow::Result<()> {
    println!("Recording 5 seconds... speak into your mic now.");
    audio::record_to_wav("test.wav", 5)?;
    println!("Saved to test.wav");
    Ok(())
}