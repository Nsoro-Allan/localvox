use anyhow::Result;
use arboard::Clipboard;
use std::thread::sleep;
use std::time::Duration;

pub struct Injector {
    clipboard: Clipboard,
    #[cfg(not(target_os = "linux"))]
    enigo: enigo::Enigo,
}

impl Injector {
    pub fn new() -> Result<Self> {
        let clipboard = Clipboard::new()?;
        #[cfg(not(target_os = "linux"))]
        let enigo = enigo::Enigo::new(&enigo::Settings::default())?;

        Ok(Self {
            clipboard,
            #[cfg(not(target_os = "linux"))]
            enigo,
        })
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

        self.simulate_paste()?;

        sleep(Duration::from_millis(200));
        if let Some(prev) = previous {
            self.clipboard.set_text(prev).ok();
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn simulate_paste(&mut self) -> Result<()> {
        use anyhow::{anyhow, Context};
        use std::process::Command;
        let status = Command::new("ydotool")
            .env("YDOTOOL_SOCKET", "/tmp/.ydotool_socket")
            .args(["key", "29:1", "47:1", "47:0", "29:0"])
            .status()
            .context("failed to run ydotool — is it installed and is the service running?")?;
        if !status.success() {
            return Err(anyhow!("ydotool exited with a non-zero status"));
        }
        Ok(())
    }

    // NEEDS REAL-DEVICE TESTING. Cmd+V — macOS uses Cmd, not Ctrl, for paste.
    #[cfg(target_os = "macos")]
    fn simulate_paste(&mut self) -> Result<()> {
        use enigo::{Direction, Key, Keyboard};
        self.enigo.key(Key::Meta, Direction::Press)?;
        self.enigo.key(Key::Unicode('v'), Direction::Click)?;
        self.enigo.key(Key::Meta, Direction::Release)?;
        Ok(())
    }

    // NEEDS REAL-DEVICE TESTING.
    #[cfg(target_os = "windows")]
    fn simulate_paste(&mut self) -> Result<()> {
        use enigo::{Direction, Key, Keyboard};
        self.enigo.key(Key::Control, Direction::Press)?;
        self.enigo.key(Key::Unicode('v'), Direction::Click)?;
        self.enigo.key(Key::Control, Direction::Release)?;
        Ok(())
    }
}