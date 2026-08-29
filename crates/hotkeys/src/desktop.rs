use crate::PushToTalkEvent;
use anyhow::{bail, Result};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

pub struct PushToTalk {
    _manager: GlobalHotKeyManager,
}

impl PushToTalk {
    pub fn new(combo: &str) -> Result<Self> {
        let manager = GlobalHotKeyManager::new()?;
        let hotkey = parse_combo(combo)?;
        manager.register(hotkey)?;
        Ok(Self { _manager: manager })
    }

    pub fn try_recv(&self) -> Option<PushToTalkEvent> {
        let event = GlobalHotKeyEvent::receiver().try_recv().ok()?;
        Some(match event.state {
            HotKeyState::Pressed => PushToTalkEvent::Pressed,
            HotKeyState::Released => PushToTalkEvent::Released,
        })
    }
}

/// Parses the same "Ctrl+F9" style strings the Linux backend uses, and
/// deliberately supports the exact same key whitelist — so the settings
/// UI dropdown we already built needs no changes across platforms.
fn parse_combo(combo: &str) -> Result<HotKey> {
    let parts: Vec<&str> = combo.split('+').collect();
    let Some((key_str, mod_strs)) = parts.split_last() else {
        bail!("empty hotkey combo");
    };

    let mut modifiers = Modifiers::empty();
    for m in mod_strs {
        match m.to_uppercase().as_str() {
            "CTRL" | "CONTROL" => modifiers |= Modifiers::CONTROL,
            "ALT" => modifiers |= Modifiers::ALT,
            "SHIFT" => modifiers |= Modifiers::SHIFT,
            other => bail!("unknown modifier: {other}"),
        }
    }

    let code = match key_str.to_uppercase().as_str() {
        "F1" => Code::F1, "F2" => Code::F2, "F3" => Code::F3, "F4" => Code::F4,
        "F5" => Code::F5, "F6" => Code::F6, "F7" => Code::F7, "F8" => Code::F8,
        "F9" => Code::F9, "F10" => Code::F10, "F11" => Code::F11, "F12" => Code::F12,
        "SCROLLLOCK" => Code::ScrollLock,
        "PAUSE" => Code::Pause,
        "INSERT" => Code::Insert,
        other => bail!("unsupported key: {other}"),
    };

    let mods = if modifiers.is_empty() { None } else { Some(modifiers) };
    Ok(HotKey::new(mods, code))
}