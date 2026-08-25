fn main() {
    let profile = hardware::detect();
    println!("{profile:#?}");
    let tier = hardware::recommend_tier(&profile);
    println!("Recommended tier: {tier:?}");
    println!("Recommended model: {}", tier.recommended_model());
}