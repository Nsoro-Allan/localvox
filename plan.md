# Localvox — full execution plan

A local-first voice dictation app (Wispr Flow-style) that transcribes speech and types it into whatever app is focused, using an on-device speech model so it works fully offline. Built with Rust + Tauri, targeting Windows, macOS, and Linux from one codebase.

---

## 1. Tech stack

| Layer | Choice | Why |
|---|---|---|
| App shell / GUI | Tauri 2.x | One webview-based UI renders on Win/Mac/Linux; small binaries, native performance |
| Core logic | Rust | Audio, ASR, hotkeys, and text injection all need OS-level access and speed |
| Audio capture | `cpal` | Cross-platform microphone input |
| Voice activity detection | Silero VAD (via `whisper-cpp-plus`) | Chunks speech in real time for streaming transcription |
| Local ASR runtime | `whisper.cpp` via `whisper-rs` / `whisper-cpp-plus` | GPU backends for Metal (Mac), CUDA/Vulkan (Win/Linux), CPU fallback everywhere |
| Hardware detection | `sysinfo` + `gpu-probe` | Cross-platform CPU/RAM/GPU/VRAM detection |
| Model downloads | `hf-hub` (Hugging Face Hub client) | Typed Rust client with resumable transfers |
| Global hotkeys | `global-hotkey` / evdev-based listener | System-wide push-to-talk, Wayland-compatible on Linux |
| Text injection | Per-OS: Win32 `SendInput`, macOS Accessibility API, X11 `XTest`, Wayland virtual-keyboard/clipboard fallback | No single cross-platform API exists for this |
| Frontend UI | Plain HTML/CSS/JS or Svelte inside Tauri's webview | Settings window, onboarding wizard, tray menu |

---

## 2. Repository structure

```
localvox/
├── Cargo.toml                 # workspace root
├── crates/
│   ├── audio/                 # cpal capture + VAD chunking
│   ├── asr/                   # whisper.cpp wrapper, model loading/swapping
│   ├── hardware/               # HardwareProfile detection + tier scoring
│   ├── model-manager/          # manifest, resumable download, checksum verify
│   ├── injector/                # trait + per-OS implementations
│   └── hotkeys/                # global hotkey registration
├── src-tauri/                  # Tauri app: commands, tray, windows, build config
│   ├── tauri.conf.json
│   └── src/main.rs
├── ui/                          # frontend: onboarding, settings, HUD overlay
└── .github/workflows/release.yml
```

Splitting the Rust logic into its own crates (outside `src-tauri`) means the audio/ASR pipeline can be built and tested standalone from the command line, without launching the GUI — useful for the early phases below.

---

## 3. Build phases

### Phase 0 — Environment setup
- Install Rust via `rustup`, Node.js LTS, and platform prerequisites (see Section 6).
- Scaffold the Cargo workspace and an empty Tauri shell (`cargo create-tauri-app`).
- Confirm a "hello world" Tauri window builds and runs on your Linux machine.

**Done when:** empty Tauri app launches locally.

### Phase 1 — Core pipeline (Linux, no UI)
- `audio` crate: capture microphone input with `cpal`, write raw PCM.
- `asr` crate: load a whisper.cpp model, transcribe a fixed audio clip, print the text.
- Wire the two together: record N seconds → transcribe → print.

**Done when:** you can speak into your mic and see accurate text in the terminal.

### Phase 2 — Hardware detection
- `hardware` crate: build a `HardwareProfile` struct (RAM, CPU cores/AVX2 support, GPU vendor, VRAM, Apple Silicon flag).
- Implement tier-scoring logic (see Section 4 for tiers).
- Test standalone: running the crate prints your detected profile and recommended tier.

**Done when:** the tool correctly reports your own machine's specs and picks a sensible tier.

### Phase 3 — Model manager
- Define the model manifest (name, size, checksum, source repo, tier — see Section 4).
- `model-manager` crate: resumable download via `hf-hub`, SHA-256 verification, cache in the OS-standard app data directory.
- Emit progress events (bytes downloaded / total) that a UI can subscribe to later.
- Wire `asr` to load whichever model is currently cached, selected by tier or user override.

**Done when:** running the tool on a clean machine downloads the recommended model automatically and transcribes with it.

### Phase 4 — Streaming / VAD
- Integrate Silero VAD so audio is chunked into speech segments as you talk, rather than waiting for a fixed recording window.
- Feed chunks into whisper.cpp's streaming mode for partial results.

**Done when:** transcription starts appearing within roughly a second of speaking, without waiting for you to stop.

### Phase 5 — Text injection (Linux first) + hotkey
- `injector` crate: define a trait (`fn inject(text: &str)`), implement the Linux X11 backend via XTest.
- `hotkeys` crate: register a global push-to-talk key.
- Wire it all together: hold hotkey → record → transcribe → inject text into the focused window.

**Done when:** you can dictate into a text editor or browser on your Linux desktop by holding a key.

### Phase 6 — Tauri GUI shell
- Onboarding wizard: hardware scan → recommended tier shown → model download progress → ready screen.
- System tray icon with idle/listening/transcribing states.
- Settings window: hotkey rebinding, model switching, input device picker, custom vocabulary list.
- Recording HUD: small always-on-top indicator shown while dictating.

**Done when:** a first-time user can install the app, get guided through setup, and start dictating without touching a terminal.

### Phase 7 — macOS and Windows support
- Port the `injector` trait: macOS via the Accessibility API + `CGEventPost` (needs user-granted Accessibility permission); Windows via `SendInput`.
- Add the Linux Wayland path: compositor virtual-keyboard protocol where supported, clipboard-copy + simulated paste as the universal fallback.
- Verify GPU acceleration picks the right backend per OS (Metal / CUDA / Vulkan / CPU).

**Done when:** the same feature set works on all three OSes.

### Phase 8 — Polish
- Custom vocabulary / dictionary for names and jargon the model tends to mishear.
- In-app model switching without restarting the app.
- Auto-updater.
- No telemetry by default, consistent with the offline/local-first pitch.

---

## 4. Model tiers and manifest

| Tier | Trigger | Model |
|---|---|---|
| Light | Under 8GB RAM, no discrete GPU | Moonshine small (English-only) |
| Balanced (default) | 8–16GB RAM, or any Apple Silicon Mac | Whisper large-v3-turbo |
| Performance | 16GB+ RAM, discrete NVIDIA GPU with 6GB+ VRAM | Parakeet TDT 0.6B (English) or Whisper turbo with CUDA |
| Max accuracy | 16GB+ VRAM | Whisper large-v3 (full) |

Apple Silicon Macs are special-cased into "Balanced" regardless of the VRAM check, since unified memory plus Metal acceleration handles Whisper turbo comfortably even on base-model chips.

Example manifest entry:

```json
{
  "whisper-turbo": {
    "size_mb": 1600,
    "tier": "balanced",
    "sha256": "…",
    "repo": "ggerganov/whisper.cpp",
    "file": "ggml-large-v3-turbo.bin"
  }
}
```

Models are cached in:
- Windows: `%APPDATA%\Localvox\models`
- macOS: `~/Library/Application Support/Localvox/models`
- Linux: `~/.local/share/localvox/models`

---

## 5. The hard platform-specific problems

Flag these early so they don't surprise you mid-project:

- **Text injection on Wayland**: no single API works everywhere due to Wayland's app isolation model. Plan for a virtual-keyboard-protocol implementation for GNOME/KDE, with clipboard+paste as a fallback for everything else.
- **Linux input permissions**: reading raw keyboard events (for reliable hotkeys across X11 and Wayland) needs read access to `/dev/input`, and re-injecting keystrokes needs write access to `/dev/uinput`. Ship a udev rule with the package rather than asking users to join the `input` group manually.
- **macOS permissions**: both microphone access and Accessibility API access require explicit user grants, prompted on first use. Budget UI/UX time for explaining why the permission is needed.
- **Windows SmartScreen**: an unsigned installer will be flagged as untrusted. Code signing (Section 7) avoids this.

---

## 6. Packaging and compiling per OS

Tauri does not support true cross-compilation — each platform's installer needs to be built on that platform's native toolchain (or a CI runner running that OS). The standard approach is a CI matrix that builds all three in parallel per release (Section 8).

### Windows
- **Prerequisites**: Rust with the `x86_64-pc-windows-msvc` target, Visual Studio Build Tools (C++ workload), WebView2 runtime (bundled automatically by the Tauri bundler if missing on the user's machine).
- **Output**: `.msi` (via WiX) or `.exe` (via NSIS) installer.
- **Code signing**: as of mid-2023, certificate authorities stopped issuing exportable code-signing certificates — new ones must live on a hardware security module. For an indie project, Azure's cloud-based signing service (Azure Artifact Signing, formerly Azure Trusted Signing) is the most accessible option and is what Tauri's own signing docs recommend.

### macOS
- **Prerequisites**: Xcode command line tools, Rust targets for both `x86_64-apple-darwin` and `aarch64-apple-darwin` if you want a universal binary.
- **Output**: `.app` bundle, then wrapped into a `.dmg` for distribution.
- **Permissions**: declare microphone and Accessibility usage in the app's entitlements/Info.plist so macOS shows the right permission prompts.
- **Code signing**: an Apple Developer ID Application certificate, plus notarization via `notarytool` — without notarization, Gatekeeper will block the app on other people's Macs.

### Linux
- **Prerequisites**: `build-essential`, `webkit2gtk` development libraries, `libayatana-appindicator` for the tray icon.
- **Output**: `.deb`, `.rpm`, and `.AppImage` — Tauri's bundler produces all three from one build.
- **Packaging extras**: include the udev rule for `/dev/uinput` access in the `.deb`/`.rpm` post-install scripts so hotkeys and Wayland injection work without manual setup.

---

## 7. CI/CD: automated cross-platform releases

Use GitHub Actions with a three-OS build matrix and the official `tauri-action`, triggered by pushing a version tag. Each OS runner builds its own native installer in parallel; signing credentials are pulled from repository secrets.

```yaml
name: Release
on:
  push:
    tags:
      - 'v*'

jobs:
  publish-tauri:
    permissions:
      contents: write
    strategy:
      matrix:
        include:
          - platform: 'macos-latest'
          - platform: 'ubuntu-22.04'
          - platform: 'windows-latest'
    runs-on: ${{ matrix.platform }}
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: lts/*
      - uses: dtolnay/rust-toolchain@stable
      - uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
          APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
          APPLE_ID: ${{ secrets.APPLE_ID }}
          APPLE_PASSWORD: ${{ secrets.APPLE_PASSWORD }}
        with:
          tagName: localvox-v__VERSION__
          releaseName: 'Localvox v__VERSION__'
          releaseDraft: true
```

Windows signing needs an extra custom sign step wired to your Azure Artifact Signing credentials, since the default signer only runs on a real Windows machine — the CI runner counts, but cross-compiling from another OS would not.

This pipeline gives you, from a single `git tag` push: a signed `.dmg` for macOS, a signed `.msi`/`.exe` for Windows, and `.deb`/`.rpm`/`.AppImage` for Linux, all attached to one GitHub Release.

---

## 8. Testing checklist before each release

- [ ] Fresh install on a machine with no model cached — onboarding wizard downloads the right tier
- [ ] Push-to-talk hotkey works with no other app focused, and doesn't leak keystrokes to other apps
- [ ] Text injection tested in: a browser text field, a native text editor, a terminal, a chat app
- [ ] Linux: tested on both an X11 session and a Wayland session (GNOME and KDE if possible)
- [ ] macOS: permission prompts appear correctly on first run; app is not blocked by Gatekeeper
- [ ] Windows: installer does not trigger a SmartScreen warning
- [ ] Model switch in settings takes effect without a restart
- [ ] App idles at low CPU/RAM when not actively transcribing

---

## 9. Suggested pacing

These are relative sizes, not fixed deadlines — adjust to your actual available time.

| Phase | Relative effort |
|---|---|
| 0. Environment setup | S |
| 1. Core pipeline (Linux) | M |
| 2. Hardware detection | S |
| 3. Model manager | M |
| 4. VAD streaming | S–M |
| 5. Linux injector + hotkey | M |
| 6. Tauri GUI shell | L |
| 7. macOS + Windows + Wayland | L |
| 8. Polish | M |

Phases 0–5 are all achievable on your Linux machine alone and get you a genuinely working (if ugly) dictation tool before you touch cross-platform concerns at all — worth treating as the first milestone.

