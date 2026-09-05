# Localvox

Localvox is a local-first voice dictation application for the desktop. Hold a hotkey, speak, and the transcribed text is typed wherever the cursor is currently focused — in any application. All processing happens on-device. No audio or text is transmitted over a network at any point, and the application works fully offline once its speech model has been downloaded.

Built with **Rust** for audio capture, transcription, and system integration, and **Tauri** for the application shell and settings interface.

---

## Features

- **Push-to-talk dictation.** Hold a global hotkey (default `Ctrl+F9`), speak, and release. The transcribed text is typed into whatever application currently has focus.
- **Fully local transcription**, powered by [whisper.cpp](https://github.com/ggerganov/whisper.cpp). No cloud services, no API keys, no internet connection required after setup.
- **Hardware-aware model selection.** On first launch, Localvox evaluates the system's CPU, RAM, and GPU, then downloads and uses a speech model sized appropriately for the available hardware.
- **Live model switching.** Models can be changed at any time from Settings; the new model downloads and loads without restarting the application.
- **Configurable hotkey**, changeable from Settings and applied immediately.
- **Custom vocabulary**, to improve recognition of names, acronyms, or domain-specific terms.
- **Custom text corrections**, applied as exact find-and-replace rules after transcription.
- **Automatic digit-sequence correction**, addressing a common transcription artifact where spoken number sequences (such as phone numbers) are rendered with incorrect punctuation.
- **Recording indicator.** A small on-screen indicator confirms that Localvox is listening or transcribing, and disappears automatically once complete.
- **System tray integration**, with live status and quick access to Settings.
- **Automatic audio recovery**, detecting and reconnecting to the microphone if it is disconnected or changed.
- **Visible error reporting.** Failures are surfaced as an in-app notification rather than failing silently.
- **Persistent settings.** Model selection, hotkey configuration, vocabulary, and corrections all persist across restarts.

---

## Architecture

Localvox is organized as a Cargo workspace. Core functionality is implemented as a set of independent crates, separate from the application shell:

```
localvox/
├── Cargo.toml                   # workspace root
├── crates/
│   ├── audio/                   # microphone capture, resampling, voice-activity segmentation
│   ├── asr/                     # speech-to-text engine (whisper.cpp bindings)
│   ├── hardware/                 # CPU/RAM/GPU detection and model-tier recommendation
│   ├── model-manager/             # model manifest, download, and checksum verification
│   ├── hotkeys/                 # global push-to-talk hotkey listener
│   └── injector/                 # text injection into the focused application
├── src-tauri/                    # application shell: tray icon, recording indicator, settings backend
│   └── src/lib.rs
├── src/                           # settings interface (frontend)
│   ├── index.html
│   ├── main.ts
│   ├── styles.css
│   └── hud.html                  # recording indicator
└── vite.config.ts
```

### Technology stack

| Component | Library |
|---|---|
| Application shell | Tauri 2 |
| Audio capture | `cpal` |
| Speech recognition | `whisper-rs` (whisper.cpp) |
| Hardware detection | `sysinfo`, `nvidia-smi` |
| Model distribution | `hf-hub` |
| Global hotkeys (Linux) | `hotkey-listener` |
| Global hotkeys (macOS/Windows) | `global-hotkey` |
| Clipboard access | `arboard` |
| Text injection (Linux) | `ydotool` |
| Text injection (macOS/Windows) | `enigo` |
| Settings persistence | `serde_json`, `dirs` |

---

## Prerequisites

### All platforms

- [Rust](https://rustup.rs) (stable)
- [Node.js](https://nodejs.org) (LTS)

### Linux

```bash
sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev \
  libx11-dev libxi-dev libxtst-dev cmake clang libclang-dev \
  libasound2-dev pkg-config libopenblas-dev ydotool
```

Text injection requires the `ydotool` service to be configured and running. Create `/etc/systemd/system/ydotool.service` (substitute your own user/group ID from `id -u` / `id -g` if not `1000`):

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

Global hotkey support reads keyboard input directly and requires membership in the `input` group:

```bash
sudo usermod -aG input $USER
```

A logout/login (or reboot) is required for this change to take effect.

**Build environment notes:** compiling `whisper-rs` from source may require pointing `bindgen` at your system's C standard library headers, and enabling OpenBLAS acceleration may require setting `BLAS_INCLUDE_DIRS`. Both are environment-specific; consult your distribution's documentation if the build fails on a missing header.

### macOS and Windows

Implementations exist for hardware-accelerated transcription (Metal / Vulkan), global hotkeys, and text injection on both platforms, but have not been validated on physical hardware — this project has been developed and tested exclusively on Linux. See [Known Limitations](#known-limitations).

---

## Building and Running

```bash
git clone <repository-url>
cd localvox
npm install
npm run tauri dev
```

On first launch, Localvox will detect the system's hardware, download an appropriately sized speech model, register the default hotkey, and run in the system tray.

To produce a distributable build:

```bash
npm run tauri build
```

---

## Usage

1. Select any text field in any application.
2. Hold the configured hotkey (`Ctrl+F9` by default).
3. Speak.
4. Release the hotkey. The transcribed text is typed at the current cursor position.

The settings window, accessible from the system tray icon, provides:

- Detected hardware specifications and recommended model tier
- Model selection and switching
- Hotkey configuration
- Custom vocabulary
- Custom correction rules

Configuration is stored at `~/.local/share/localvox/settings.json` on Linux; models are cached at `~/.local/share/localvox/models/`.

---

## Known Limitations

- **Recording indicator placement on Linux/Wayland is best-effort.** The Wayland protocol does not allow applications to set their own window position, and the GNOME compositor does not implement the extension that would normally address this. Positioning may be inconsistent under GNOME/Wayland as a result.
- **macOS and Windows support is implemented but unverified.** These code paths have not been exercised on real hardware.
- **No mobile support.** Localvox's interaction model — a system-wide hotkey and text injection into the focused application — has no direct equivalent on Android's application sandboxing model. Mobile support would require a substantially different application design.
- **No automated post-transcription rewriting.** Correction of spoken false starts or self-corrections (e.g., restating a sentence mid-thought) is not currently implemented.
- **Model support is limited to the Whisper family.** All model tiers use whisper.cpp-compatible models; alternative architectures would require a separate inference backend.

---

## Troubleshooting

| Symptom | Likely Cause |
|---|---|
| `pkg-config ... alsa was not found` | Missing ALSA development headers |
| `fatal error: 'stdbool.h' file not found` during build | See build environment notes above |
| Hotkey has no effect | Confirm membership in the `input` group and that the session has been restarted since joining |
| Transcribed text is not typed anywhere | Confirm the `ydotool` service is running: `systemctl status ydotool` |
| Application stops responding to dictation | Check the settings window for an error notification; most failures are surfaced there |

---

## License

Licensed under the [MIT License](LICENSE).

## Acknowledgments

- [whisper.cpp](https://github.com/ggerganov/whisper.cpp) and [whisper-rs](https://github.com/tazz4843/whisper-rs) for on-device speech recognition
- [Tauri](https://tauri.app) for the cross-platform application shell
- [ydotool](https://github.com/ReimuNotMoe/ydotool) for display-server-independent input simulation on Linux