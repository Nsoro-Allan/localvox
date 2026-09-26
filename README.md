# LocalVox

**Type at the speed you speak — fully local, offline-capable voice dictation for the desktop.**

LocalVox is a local-first voice dictation application. Hold a global hotkey, speak, and the transcribed text is typed into whatever application currently has focus. All audio capture, speech recognition, and text injection happen on-device. No audio or text is ever sent over the network. Once the speech model is downloaded, the app works completely offline.

Built with **Rust** (audio, transcription, system integration) and **Tauri 2** (application shell + settings UI).

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange)](https://www.rust-lang.org)
[![Tauri](https://img.shields.io/badge/Tauri-2-purple)](https://tauri.app)

---

## Features

- **Push-to-talk dictation** — Hold a global hotkey (default `Ctrl+F9`), speak, release. Text is injected into the focused application.
- **Fully local transcription** — Powered by [whisper.cpp](https://github.com/ggerganov/whisper.cpp) via `whisper-rs`. No cloud services, no API keys, no internet after initial model download.
- **Hardware-aware model selection** — On first launch the app inspects CPU, RAM and GPU, then downloads and uses a Whisper model sized for the available hardware.
- **Live model switching** — Change models from Settings; the new model downloads and loads without restarting the app.
- **Configurable hotkey** — Change it in Settings; the new combination is applied immediately.
- **Custom vocabulary** — Improve recognition of names, acronyms and domain-specific terms.
- **Custom text corrections** — Exact find-and-replace rules applied after transcription.
- **Automatic digit-sequence correction** — Fixes common Whisper artifacts on spoken number sequences (phone numbers, etc.).
- **Recording indicator** — Lightweight on-screen HUD that shows listening / transcribing state.
- **System tray integration** — Live status + quick access to Settings.
- **Automatic audio recovery** — Detects microphone disconnects/changes and reconnects.
- **Visible error reporting** — Failures surface as in-app notifications instead of failing silently.
- **Persistent settings** — Model choice, hotkey, vocabulary and corrections survive restarts.

---

## Architecture

LocalVox is a Cargo workspace. Core logic lives in independent crates; the Tauri shell only wires them together and provides the UI/tray/HUD.

```
localvox/
├── Cargo.toml                     # workspace root
├── crates/
│   ├── audio/                     # microphone capture, resampling, VAD segmentation
│   ├── asr/                       # speech-to-text (whisper-rs / whisper.cpp)
│   ├── hardware/                  # CPU / RAM / GPU detection + model-tier recommendation
│   ├── model-manager/             # model catalog, download (hf-hub), checksum verification
│   ├── hotkeys/                   # global push-to-talk hotkey listener
│   └── injector/                  # text injection into the focused application
├── src-tauri/                     # Tauri application shell
│   └── src/lib.rs                 # tray, HUD, settings backend, pipeline orchestration
├── src/                           # settings frontend (Vite + TypeScript)
│   ├── index.html
│   ├── main.ts
│   └── styles.css
├── hud.html                       # recording indicator
└── vite.config.ts
```

### Technology stack

| Component              | Library / Tool                          |
|------------------------|-----------------------------------------|
| Application shell      | Tauri 2                                 |
| Audio capture          | `cpal`                                  |
| Speech recognition     | `whisper-rs` (whisper.cpp)              |
| Hardware detection     | `sysinfo`, `nvidia-smi`                 |
| Model distribution     | `hf-hub`                                |
| Global hotkeys (Linux) | `hotkey-listener`                       |
| Global hotkeys (macOS/Windows) | `global-hotkey`                   |
| Text injection (Linux) | `ydotool`                               |
| Text injection (macOS/Windows) | `enigo`                          |
| Clipboard              | `arboard`                               |
| Settings persistence   | `serde_json` + `dirs`                   |

### Supported models

The model catalog currently ships these Whisper GGML models (English-focused except the large turbo):

| ID                       | File                        | Approx. size |
|--------------------------|-----------------------------|--------------|
| `whisper-tiny-en`        | `ggml-tiny.en.bin`          | 75 MB        |
| `whisper-base-en`        | `ggml-base.en.bin`          | 148 MB       |
| `whisper-small-en`       | `ggml-small.en.bin`         | 488 MB       |
| `whisper-medium-en`      | `ggml-medium.en.bin`        | 1.5 GB       |
| `whisper-large-v3-turbo` | `ggml-large-v3-turbo.bin`   | 1.6 GB       |

Models are downloaded from the official `ggerganov/whisper.cpp` Hugging Face repo and cached under the OS data directory.

---

## Prerequisites

### All platforms

- [Rust](https://rustup.rs) (stable toolchain)
- [Node.js](https://nodejs.org) (LTS)

### Linux

```bash
sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev \
  libx11-dev libxi-dev libxtst-dev cmake clang libclang-dev \
  libasound2-dev pkg-config libopenblas-dev ydotool
```

**ydotool (text injection)** must be running as a service. Create `/etc/systemd/system/ydotool.service` (replace `1000:1000` with your own UID:GID from `id -u` / `id -g` if different):

```ini
[Unit]
Description=ydotool daemon

[Service]
ExecStart=/usr/bin/ydotoold --socket-path=/tmp/.ydotool_socket --socket-own=1000:1000
Restart=always

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now ydotool
```

**Global hotkeys** require membership in the `input` group:

```bash
sudo usermod -aG input $USER
```

Log out and back in (or reboot) for the group change to take effect.

**Build tips**

- `whisper-rs` may need `BINDGEN_EXTRA_CLANG_ARGS` pointing at your system C headers.
- OpenBLAS acceleration may need `BLAS_INCLUDE_DIRS` set.
- Both are distribution-specific; check your distro docs if the build fails on missing headers.

### macOS & Windows

Code paths for Metal / Vulkan acceleration, global hotkeys and text injection exist, but they have **not been validated on real hardware**. Development and testing so far have been Linux-only. See [Known Limitations](#known-limitations).

> **macOS note:** Official release builds are currently **not code-signed or notarized**. See the [Troubleshooting](#troubleshooting) section for how to open them.

---

## Building & Running

```bash
git clone https://github.com/Nsoro-Allan/localvox.git
cd localvox
npm install
npm run tauri dev
```

On first launch LocalVox will:

1. Detect hardware and recommend a model tier
2. Download the appropriate Whisper model
3. Register the default hotkey
4. Run in the system tray

To produce a release build:

```bash
npm run tauri build
```

CI also builds on Ubuntu, macOS and Windows via the GitHub Actions release workflow (triggered on `v*` tags).

---

## Usage

1. Focus any text field in any application.
2. Hold the configured hotkey (`Ctrl+F9` by default).
3. Speak.
4. Release the hotkey → transcribed text is typed at the cursor.

Open **Settings** from the system tray icon to:

- View detected hardware + recommended model tier
- Switch / download models
- Rebind the hotkey
- Manage custom vocabulary
- Manage find-and-replace correction rules

### Configuration locations

| Platform | Settings                         | Models cache                          |
|----------|----------------------------------|---------------------------------------|
| Linux    | `~/.local/share/localvox/settings.json` | `~/.local/share/localvox/models/` |
| macOS    | `~/Library/Application Support/localvox/settings.json` | `~/Library/Application Support/localvox/models/` |
| Windows  | `%APPDATA%\localvox\settings.json` | `%APPDATA%\localvox\models\`       |

---

## Known Limitations

- **Recording indicator on Linux/Wayland** is best-effort. Wayland does not allow arbitrary window positioning and GNOME does not implement the relevant extension, so placement can be inconsistent.
- **macOS and Windows support is implemented but unverified** on physical hardware.
- **macOS builds are not code-signed or notarized.** Gatekeeper will block downloaded builds with a “damaged” warning (see Troubleshooting for the workaround). An Apple Developer ID is required for proper distribution.
- **No mobile support.** The interaction model (system-wide hotkey + text injection) does not map cleanly to mobile sandboxing.
- **No automated post-transcription rewriting** of spoken false starts / self-corrections.
- **Model support is currently limited to the Whisper family** (whisper.cpp GGML). Adding other architectures would require a new ASR backend.

---

## Troubleshooting

| Symptom | Likely cause / fix |
|---------|--------------------|
| `pkg-config ... alsa was not found` | Install ALSA development headers (`libasound2-dev`) |
| `fatal error: 'stdbool.h' file not found` | Set `BINDGEN_EXTRA_CLANG_ARGS` (see build notes) |
| Hotkey does nothing | Confirm you are in the `input` group and have logged out/in |
| Transcribed text is never typed | Check `systemctl status ydotool` — the daemon must be running |
| App stops responding to dictation | Open Settings; most failures appear as notifications there |

### macOS: “localvox is damaged and can’t be opened”

Official release builds are currently **not code-signed or notarized** (no Apple Developer ID yet).  
macOS Gatekeeper therefore blocks the app after download and shows a misleading “damaged” warning.

**Temporary fix** (safe for builds downloaded from this repository):

```bash
# For the .app
xattr -cr /path/to/localvox.app

# Or for the .dmg before opening it
xattr -cr /path/to/localvox.dmg
```

Alternative methods:

- Right-click the app → **Open**
- System Settings → Privacy & Security → click **Open Anyway** after trying to launch it once

After the first successful launch the app should open normally.

---

## Contributing

Contributions are welcome — bug reports, feature requests, platform testing (especially macOS/Windows), and code.

### Development workflow

1. Fork the repository and create a feature branch.
2. Make your changes. Prefer keeping logic inside the appropriate crate (`audio`, `asr`, `hardware`, `model-manager`, `hotkeys`, `injector`) rather than inside `src-tauri`.
3. Test locally with `npm run tauri dev`.
4. Open a pull request against `main` with a clear description of the change and any platform notes.

### Extending / upgrading the project

Useful entry points for common upgrades:

| Goal | Where to look |
|------|---------------|
| Add a new Whisper model | `crates/model-manager/src/lib.rs` → `MANIFEST` |
| Change model recommendation logic | `crates/hardware` |
| Improve / replace ASR backend | `crates/asr` (currently only whisper-rs) |
| Support a new platform for hotkeys | `crates/hotkeys` |
| Support a new platform for text injection | `crates/injector` |
| Change the recording HUD | `hud.html` + related Tauri window code in `src-tauri` |
| Settings UI / new options | `src/` (frontend) + commands in `src-tauri/src/lib.rs` |
| Pipeline orchestration | `src-tauri/src/lib.rs` (`run_pipeline` and surrounding state) |

The crates are deliberately independent so you can experiment with a single subsystem without touching the whole application.

### Suggested contribution areas

- Real-hardware testing and bug reports for **macOS** and **Windows**
- Additional language models or multilingual Whisper variants
- Better Wayland window positioning for the HUD
- Optional post-processing / LLM polishing layer
- Packaging improvements (AppImage, Flatpak, deb, etc.)

---

## License

Licensed under the [MIT License](LICENSE).

---

## Acknowledgments

- [whisper.cpp](https://github.com/ggerganov/whisper.cpp) and [whisper-rs](https://github.com/tazz4843/whisper-rs) — on-device speech recognition
- [Tauri](https://tauri.app) — cross-platform application shell
- [ydotool](https://github.com/ReimuNotMoe/ydotool) — display-server-independent input simulation on Linux
