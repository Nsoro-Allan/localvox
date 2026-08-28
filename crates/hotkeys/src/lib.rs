use anyhow::Result;
use hotkey_listener::{HotkeyEvent, HotkeyListenerBuilder, HotkeyListenerHandle};
use std::time::Duration;

pub enum PushToTalkEvent {
    Pressed,
    Released,
}

pub struct PushToTalk {
    handle: HotkeyListenerHandle,
}

impl PushToTalk {
    /// `combo` looks like "Ctrl+F9" or "Shift+Alt+Insert" — any
    /// combination of Ctrl/Alt/Shift plus one of F1–F12, ScrollLock,
    /// Pause, or Insert.
    pub fn new(combo: &str) -> Result<Self> {
        let hotkey = hotkey_listener::parse_hotkey(combo)?;
        let handle = HotkeyListenerBuilder::new()
            .add_hotkey(hotkey)
            .build()?
            .start()?;
        Ok(Self { handle })
    }

    pub fn try_recv(&self) -> Option<PushToTalkEvent> {
        match self.handle.recv_timeout(Duration::from_millis(10)) {
            Ok(HotkeyEvent::Pressed(_)) => Some(PushToTalkEvent::Pressed),
            Ok(HotkeyEvent::Released(_)) => Some(PushToTalkEvent::Released),
            Err(_) => None,
        }
    }
}