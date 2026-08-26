use anyhow::Result;
use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::thread::sleep;
use std::time::Duration;

/// Types text into whatever window currently has focus by pasting it,
/// rather than simulating individual keystrokes — avoids a class of
/// bug where fast per-character typing drops shifted (capital) letters.
pub struct Injector {
    enigo: Enigo,
    clipboard: Clipboard,
}

impl Injector {
    pub fn new() -> Result<Self> {
        let enigo = Enigo::new(&Settings::default())?;
        let clipboard = Clipboard::new()?;
        Ok(Self { enigo, clipboard })
    }

    pub fn inject(&mut self, text: &str) -> Result<()> {
    let previous = self.clipboard.get_text().ok();

    self.clipboard.set_text(text.to_string())?;

    // Confirm the clipboard actually holds our text before pasting.
    // Claiming clipboard ownership on Linux isn't always instant —
    // especially the very first time — and pasting too soon can grab
    // whatever was there before instead of what we just set.
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

    self.enigo.key(Key::Control, Direction::Press)?;
    self.enigo.key(Key::Unicode('v'), Direction::Click)?;
    self.enigo.key(Key::Control, Direction::Release)?;

    sleep(Duration::from_millis(200));
    if let Some(prev) = previous {
        self.clipboard.set_text(prev).ok();
    }

    Ok(())
    }
}