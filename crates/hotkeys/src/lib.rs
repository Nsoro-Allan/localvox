use anyhow::Result;
use hotkey_listener::{HotkeyEvent, HotkeyListenerBuilder, HotkeyListenerHandle};
use std::time::Duration;

pub enum PushToTalkEvent {
    Pressed,
    Released,
}

/// Registers Ctrl+F9 as a global push-to-talk hotkey by reading raw
/// keyboard events via evdev — works on both X11 and Wayland, since it
/// reads /dev/input directly instead of going through the display
/// server. Requires your user to be in the `input` group.
pub struct PushToTalk {
    handle: HotkeyListenerHandle,
}

impl PushToTalk {
    pub fn new() -> Result<Self> {
        let hotkey = hotkey_listener::parse_hotkey("Ctrl+F9")?;
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