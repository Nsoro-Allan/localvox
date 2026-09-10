use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager};

struct StatusItem(MenuItem<tauri::Wry>);

struct AppState {
    current_model: Mutex<String>,
    transcriber: Mutex<Option<asr::Transcriber>>,
    hotkey_combo: Mutex<String>,
    hotkey_rebind_tx: Mutex<Option<std::sync::mpsc::Sender<String>>>,
    vocabulary: Mutex<Vec<String>>,
    replacements: Mutex<Vec<ReplacementRule>>,
}

#[derive(Serialize, Clone)]
struct HardwareInfo {
    total_ram_gb: f64,
    cpu_cores: usize,
    is_apple_silicon: bool,
    gpu_name: Option<String>,
    tier: String,
}

#[derive(Serialize, Clone)]
struct ModelInfo {
    id: String,
    size_mb: u64,
    downloaded: bool,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Settings {
    hotkey_combo: String,
    model_id: Option<String>,
    vocabulary: Vec<String>,
    replacements: Vec<ReplacementRule>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey_combo: "Ctrl+F9".to_string(),
            model_id: None,
            vocabulary: Vec::new(),
            replacements: Vec::new(),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct ReplacementRule {
    find: String,
    replace: String,
}


fn settings_path() -> Option<std::path::PathBuf> {
    let dir = dirs::data_dir()?.join("localvox");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("settings.json"))
}

fn load_settings() -> Settings {
    settings_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_settings(settings: &Settings) {
    if let Some(path) = settings_path() {
        if let Ok(json) = serde_json::to_string_pretty(settings) {
            std::fs::write(path, json).ok();
        }
    }
}

fn apply_replacements(text: &str, rules: &[ReplacementRule]) -> String {
    let mut result = text.to_string();
    for rule in rules {
        if !rule.find.is_empty() {
            result = replace_case_insensitive(&result, &rule.find, &rule.replace);
        }
    }
    result
}

fn replace_case_insensitive(text: &str, find: &str, replace: &str) -> String {
    let lower_text = text.to_ascii_lowercase();
    let lower_find = find.to_ascii_lowercase();
    let mut result = String::with_capacity(text.len());
    let mut last_end = 0;
    let mut search_start = 0;
    while let Some(pos) = lower_text[search_start..].find(&lower_find) {
        let start = search_start + pos;
        let end = start + find.len();
        result.push_str(&text[last_end..start]);
        result.push_str(replace);
        last_end = end;
        search_start = end;
    }
    result.push_str(&text[last_end..]);
    result
}

fn fix_digit_sequences(text: &str) -> String {
    let re = regex::Regex::new(r"\d+(?:,\s*\d+)+").unwrap();
    re.replace_all(text, |caps: &regex::Captures| {
        let matched = &caps[0];
        let digits_only: String = matched.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits_only.len() >= 7 {
            digits_only
        } else {
            matched.to_string()
        }
    })
    .to_string()
}

#[tauri::command]
fn get_hardware_info() -> HardwareInfo {
    let profile = hardware::detect();
    let tier = hardware::recommend_tier(&profile);
    HardwareInfo {
        total_ram_gb: profile.total_ram_gb,
        cpu_cores: profile.cpu_cores,
        is_apple_silicon: profile.is_apple_silicon,
        gpu_name: profile.nvidia_gpu.map(|g| g.name),
        tier: format!("{tier:?}"),
    }
}

#[tauri::command]
fn list_models() -> Vec<ModelInfo> {
    model_manager::MANIFEST
        .iter()
        .map(|m| {
            let downloaded = model_manager::models_dir()
                .map(|dir| dir.join(m.file).exists())
                .unwrap_or(false);
            ModelInfo { id: m.id.to_string(), size_mb: m.size_mb, downloaded }
        })
        .collect()
}

#[tauri::command]
fn get_current_model(state: tauri::State<Arc<AppState>>) -> String {
    state.current_model.lock().unwrap().clone()
}

#[tauri::command]
async fn switch_model(app: tauri::AppHandle, state: tauri::State<'_, Arc<AppState>>, model_id: String) -> Result<(), String> {
    let entry = model_manager::find_model(&model_id).ok_or_else(|| "unknown model".to_string())?;

    app.emit("model-switch-progress", format!("Downloading {}...", entry.id)).ok();

    let app_for_task = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<asr::Transcriber> {
        let path = model_manager::download_model(entry)?;
        app_for_task.emit("model-switch-progress", "Loading model...".to_string()).ok();
        let path_str = path.to_str().ok_or_else(|| anyhow::anyhow!("invalid model path"))?;
        asr::Transcriber::load(path_str)
    })
    .await
    .map_err(|e| format!("background task panicked: {e}"))?;

    match result {
        Ok(transcriber) => {
            *state.transcriber.lock().unwrap() = Some(transcriber);
            *state.current_model.lock().unwrap() = model_id.clone();

            let mut settings = load_settings();
            settings.model_id = Some(model_id);
            save_settings(&settings);

            app.emit("model-switch-progress", "Ready".to_string()).ok();
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
fn get_hotkey(state: tauri::State<Arc<AppState>>) -> String {
    state.hotkey_combo.lock().unwrap().clone()
}

#[tauri::command]
fn set_hotkey(state: tauri::State<Arc<AppState>>, combo: String) -> Result<(), String> {
    let tx_guard = state.hotkey_rebind_tx.lock().unwrap();
    if let Some(tx) = tx_guard.as_ref() {
        tx.send(combo.clone()).map_err(|e| e.to_string())?;
    }
    drop(tx_guard);

    let mut settings = load_settings();
    settings.hotkey_combo = combo;
    save_settings(&settings);

    Ok(())
}

#[tauri::command]
fn get_vocabulary(state: tauri::State<Arc<AppState>>) -> Vec<String> {
    state.vocabulary.lock().unwrap().clone()
}

#[tauri::command]
fn set_vocabulary(state: tauri::State<Arc<AppState>>, words: Vec<String>) {
    *state.vocabulary.lock().unwrap() = words.clone();
    let mut settings = load_settings();
    settings.vocabulary = words;
    save_settings(&settings);
}

#[tauri::command]
fn get_replacements(state: tauri::State<Arc<AppState>>) -> Vec<ReplacementRule> {
    state.replacements.lock().unwrap().clone()
}

#[tauri::command]
fn set_replacements(state: tauri::State<Arc<AppState>>, rules: Vec<ReplacementRule>) {
    *state.replacements.lock().unwrap() = rules.clone();
    let mut settings = load_settings();
    settings.replacements = rules;
    save_settings(&settings);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = Arc::new(AppState {
        current_model: Mutex::new(String::new()),
        transcriber: Mutex::new(None),
        hotkey_combo: Mutex::new(String::new()),
        hotkey_rebind_tx: Mutex::new(None),
        vocabulary: Mutex::new(Vec::new()),
        replacements: Mutex::new(Vec::new()),
    });

    tauri::Builder::default()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            get_hardware_info,
            list_models,
            get_current_model,
            switch_model,
            get_hotkey,
            set_hotkey,
            get_vocabulary,
            set_vocabulary,
            get_replacements,
            set_replacements
        ])
        .setup(move |app| {
            let status_item = MenuItem::with_id(app, "status", "Status: starting...", false, None::<&str>)?;
            let settings_item = MenuItem::with_id(app, "settings", "Open Settings", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&status_item, &settings_item, &quit_item])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "quit" => app.exit(0),
                    "settings" => {
                        if let Some(window) = app.get_webview_window("main") {
                            window.show().ok();
                            window.set_focus().ok();
                        }
                    }
                    _ => {}
                })
                .build(app)?;

            let hud_window = tauri::WebviewWindowBuilder::new(app, "hud", tauri::WebviewUrl::App("hud.html".into()))
                .decorations(false)
                .transparent(true)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .shadow(false)
                .inner_size(120.0, 44.0)
                .visible(false)
                .build()?;

            if let Ok(Some(monitor)) = hud_window.primary_monitor() {
                let size = monitor.size();
                let scale = monitor.scale_factor();
                let win_w = 120.0;
                let win_h = 44.0;
                let x = (size.width as f64 / scale - win_w) / 2.0;
                let y = size.height as f64 / scale - win_h - 40.0;
                hud_window.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(x, y))).ok();
            }

            app.manage(StatusItem(status_item));

            if let Some(window) = app.get_webview_window("main") {
                window.hide().ok();
                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        window_clone.hide().ok();
                    }
                });
            }

            let handle = app.handle().clone();
            let pipeline_state = state.clone();
            thread::spawn(move || {
                if let Err(e) = run_pipeline(handle.clone(), pipeline_state) {
                    eprintln!("pipeline error: {e:?}");
                    handle.emit("pipeline-warning", format!("Localvox failed to start: {e}")).ok();
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn set_status(app: &tauri::AppHandle, text: &str) {
    if let Some(status) = app.try_state::<StatusItem>() {
        status.0.set_text(format!("Status: {text}")).ok();
    }
    app.emit("pipeline-status", text.to_string()).ok();

    if let Some(hud) = app.get_webview_window("hud") {
        if text.contains("listening") || text.contains("transcribing") {
            hud.show().ok();
        } else {
            hud.hide().ok();
        }
    }
}

fn run_pipeline(app: tauri::AppHandle, state: Arc<AppState>) -> anyhow::Result<()> {
    let settings = load_settings();

    set_status(&app, "checking model...");
    let model_id = match &settings.model_id {
        Some(id) if model_manager::find_model(id).is_some() => id.clone(),
        _ => {
            let profile = hardware::detect();
            let tier = hardware::recommend_tier(&profile);
            tier.recommended_model().to_string()
        }
    };
    let entry = model_manager::find_model(&model_id).expect("model in manifest");
    let model_path = model_manager::download_model(entry)?;

    set_status(&app, "loading model...");
    let transcriber = asr::Transcriber::load(model_path.to_str().unwrap())?;
    *state.transcriber.lock().unwrap() = Some(transcriber);
    *state.current_model.lock().unwrap() = model_id;

    *state.vocabulary.lock().unwrap() = settings.vocabulary.clone();
    *state.replacements.lock().unwrap() = settings.replacements.clone();

    let (rebind_tx, rebind_rx) = std::sync::mpsc::channel::<String>();
    *state.hotkey_rebind_tx.lock().unwrap() = Some(rebind_tx);

    let combo = settings.hotkey_combo.clone();
    let mut ptt = hotkeys::PushToTalk::new(&combo)?;
    *state.hotkey_combo.lock().unwrap() = combo;
    app.emit("settings-ready", ()).ok();

    let mut injector = injector::Injector::new()?;
    let mut live = audio::start_stream()?;

    set_status(&app, "Running");

    let mut buffer: Vec<f32> = Vec::new();
    let mut recording = false;
    let mut last_chunk_at = std::time::Instant::now();

    loop {
        if let Ok(new_combo) = rebind_rx.try_recv() {
            match hotkeys::PushToTalk::new(&new_combo) {
                Ok(new_ptt) => {
                    ptt = new_ptt;
                    *state.hotkey_combo.lock().unwrap() = new_combo;
                }
                Err(e) => {
                    app.emit("pipeline-warning", format!("Failed to rebind hotkey: {e}")).ok();
                }
            }
        }

        let event = ptt.try_recv();

        if let Some(event) = event {
            match event {
                hotkeys::PushToTalkEvent::Pressed => {
                    recording = true;
                    buffer.clear();
                    set_status(&app, "listening...");
                }
                hotkeys::PushToTalkEvent::Released => {
                    recording = false;
                    set_status(&app, "transcribing...");
                    let resampled = audio::resample_linear(&buffer, live.sample_rate, 16_000);
                    let text_result = {
                        let guard = state.transcriber.lock().unwrap();
                        let prompt = state.vocabulary.lock().unwrap().join(", ");
                        guard.as_ref().map(|t| t.transcribe_with_prompt(&resampled, &prompt))
                    };

                    let final_text = match text_result {
                        Some(Ok(text)) if !text.is_empty() => {
                            let rules = state.replacements.lock().unwrap().clone();
                            let corrected = fix_digit_sequences(&text);
                            Some(apply_replacements(&corrected, &rules))
                        }
                        Some(Err(e)) => {
                            app.emit("pipeline-warning", format!("Transcription failed: {e}")).ok();
                            None
                        }
                        _ => None,
                    };

                    set_status(&app, "idle");

                    if let Some(text) = final_text {
                        if let Err(e) = injector.inject(&text) {
                            app.emit("pipeline-warning", format!("Couldn't type the text: {e}")).ok();
                        }
                    }
                }
            }
        }

        match live.rx.recv_timeout(Duration::from_millis(50)) {
            Ok(chunk) => {
                last_chunk_at = std::time::Instant::now();
                if recording {
                    let mono = audio::downmix(&chunk, live.channels);
                    buffer.extend_from_slice(&mono);
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if last_chunk_at.elapsed() > Duration::from_secs(5) {
                    reconnect_audio(&app, &mut live);
                    last_chunk_at = std::time::Instant::now();
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                reconnect_audio(&app, &mut live);
                last_chunk_at = std::time::Instant::now();
            }
        }
    }
}

fn reconnect_audio(app: &tauri::AppHandle, live: &mut audio::LiveStream) {
    app.emit("pipeline-warning", "Audio input lost — reconnecting...".to_string()).ok();
    set_status(app, "reconnecting audio...");
    loop {
        thread::sleep(Duration::from_secs(2));
        if let Ok(new_stream) = audio::start_stream() {
            *live = new_stream;
            set_status(app, "Running");
            app.emit("pipeline-warning", "Audio input reconnected.".to_string()).ok();
            return;
        }
    }
}