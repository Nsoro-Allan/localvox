use std::process::Command;
use sysinfo::System;

#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub name: String,
    pub vram_mb: u64,
}

#[derive(Debug, Clone)]
pub struct HardwareProfile {
    pub total_ram_gb: f64,
    pub cpu_cores: usize,
    pub is_apple_silicon: bool,
    pub nvidia_gpu: Option<GpuInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Light,
    Balanced,
    Performance,
    MaxAccuracy,
}

impl Tier {
    pub fn recommended_model(&self) -> &'static str {
    match self {
        Tier::Light => "whisper-tiny-en",
        Tier::Balanced => "whisper-base-en",
        Tier::Performance => "whisper-medium-en",
        Tier::MaxAccuracy => "whisper-large-v3-turbo",
    }
}
}

/// Detects an NVIDIA GPU and its VRAM by shelling out to `nvidia-smi`,
/// which ships with the NVIDIA driver on both Linux and Windows.
/// Returns None if there's no NVIDIA driver installed (including on
/// Macs, which never have one).
fn detect_nvidia_gpu() -> Option<GpuInfo> {
    let output = Command::new("nvidia-smi")
        .args(["--query-gpu=name,memory.total", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let first_line = text.lines().next()?;
    let mut parts = first_line.split(',').map(|s| s.trim());
    let name = parts.next()?.to_string();
    let vram_mb: u64 = parts.next()?.parse().ok()?;

    Some(GpuInfo { name, vram_mb })
}

pub fn detect() -> HardwareProfile {
    let mut sys = System::new_all();
    sys.refresh_all();

    let total_ram_gb = sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
    let cpu_cores = sys.cpus().len();
    let is_apple_silicon = cfg!(target_os = "macos") && cfg!(target_arch = "aarch64");
    let nvidia_gpu = detect_nvidia_gpu();

    HardwareProfile { total_ram_gb, cpu_cores, is_apple_silicon, nvidia_gpu }
}

pub fn recommend_tier(profile: &HardwareProfile) -> Tier {
    // Apple Silicon goes straight to Balanced regardless of the RAM/GPU
    // math below — unified memory + Metal handles Whisper turbo
    // comfortably even on base M-series chips.
    if profile.is_apple_silicon {
        return Tier::Balanced;
    }

    if let Some(gpu) = &profile.nvidia_gpu {
        if gpu.vram_mb >= 16_000 {
            return Tier::MaxAccuracy;
        }
        if gpu.vram_mb >= 6_000 && profile.total_ram_gb >= 16.0 {
            return Tier::Performance;
        }
    }

    if profile.total_ram_gb >= 8.0 {
        return Tier::Balanced;
    }

    Tier::Light
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_tier_for_low_ram_no_gpu() {
        let profile = HardwareProfile { total_ram_gb: 4.0, cpu_cores: 4, is_apple_silicon: false, nvidia_gpu: None };
        assert_eq!(recommend_tier(&profile), Tier::Light);
    }

    #[test]
    fn performance_tier_for_good_gpu() {
        let profile = HardwareProfile {
            total_ram_gb: 16.0, cpu_cores: 16, is_apple_silicon: false,
            nvidia_gpu: Some(GpuInfo { name: "RTX 4070".into(), vram_mb: 12_000 }),
        };
        assert_eq!(recommend_tier(&profile), Tier::Performance);
    }

    #[test]
    fn apple_silicon_always_balanced() {
        let profile = HardwareProfile { total_ram_gb: 8.0, cpu_cores: 8, is_apple_silicon: true, nvidia_gpu: None };
        assert_eq!(recommend_tier(&profile), Tier::Balanced);
    }
}