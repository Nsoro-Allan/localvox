use anyhow::{anyhow, Context, Result};
use arboard::Clipboard;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

/// Types text into whatever window currently has focus by putting it
/// on the clipboard, then simulating Ctrl+V via ydotool — which
/// injects input at the kernel (uinput) level, bypassing X11 and
/// Wayland entirely. This avoids GNOME's RemoteDesktop portal prompt
/// that a display-server-level tool like enigo triggers on Wayland.
pub struct Injector {
    clipboard: Clipboard,
}

impl Injector {
    pub fn new() -> Result<Self> {
        Ok(Self { clipboard: Clipboard::new()? })
    }

    pub fn inject(&mut self, text: &str) -> Result<()> {
        let previous = self.clipboard.get_text().ok();

        self.clipboard.set_text(text.to_string())?;

        let mut confirmed = false;
        for _ in 0..20 {
            if self.clipboard.get_text().map(|s| s == text).unwrap_or(false) {
                confirmed = true;
                break;
            }
            sleep(Duration::from_millis(20));
        }
        if !confirmed {
            eprintln!("warning: clipboard didn't confirm new content in time, pasting anyway");
        }

        // Ctrl+V via ydotool: 29 = KEY_LEFTCTRL, 47 = KEY_V
        let status = Command::new("ydotool")
            .env("YDOTOOL_SOCKET", "/tmp/.ydotool_socket")
            .args(["key", "29:1", "47:1", "47:0", "29:0"])
            .status()
            .context("failed to run ydotool — is it installed and is the service running?")?;
        if !status.success() {
            return Err(anyhow!("ydotool exited with a non-zero status"));
        }

        sleep(Duration::from_millis(200));
        if let Some(prev) = previous {
            self.clipboard.set_text(prev).ok();
        }

        Ok(())
    }
}