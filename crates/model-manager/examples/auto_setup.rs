fn main() -> anyhow::Result<()> {
    let profile = hardware::detect();
    let tier = hardware::recommend_tier(&profile);
    let model_id = tier.recommended_model();
    println!("Detected tier: {tier:?} -> {model_id}");

    let entry = model_manager::find_model(model_id).expect("model in manifest");
    println!("Fetching {} ({} MB)...", entry.id, entry.size_mb);
    let path = model_manager::download_model(entry)?;
    println!("Ready at {path:?}");
    Ok(())
}