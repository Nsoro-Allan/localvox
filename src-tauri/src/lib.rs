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
fn switch_model(app: tauri::AppHandle, state: tauri::State<Arc<AppState>>, model_id: String) -> Result<(), String> {
    let entry = model_manager::find_model(&model_id).ok_or_else(|| "unknown model".to_string())?;
    app.emit("model-switch-progress", format!("Downloading {}...", entry.id)).ok();
    let path = model_manager::download_model(entry).map_err(|e| e.to_string())?;
    app.emit("model-switch-progress", "Loading model...".to_string()).ok();
    let transcriber = asr::Transcriber::load(path.to_str().unwrap()).map_err(|e| e.to_string())?;
    *state.transcriber.lock().unwrap() = Some(transcriber);
    *state.current_model.lock().unwrap() = model_id;
    app.emit("model-switch-progress", "Ready".to_string()).ok();
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = Arc::new(AppState {
        current_model: Mutex::new(String::new()),
        transcriber: Mutex::new(None),
    });

    tauri::Builder::default()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            get_hardware_info,
            list_models,
            get_current_model,
            switch_model
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
                if let Err(e) = run_pipeline(handle, pipeline_state) {
                    eprintln!("pipeline error: {e:?}");
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
}

fn run_pipeline(app: tauri::AppHandle, state: Arc<AppState>) -> anyhow::Result<()> {
    set_status(&app, "detecting hardware...");
    let profile = hardware::detect();
    let tier = hardware::recommend_tier(&profile);
    let model_id = tier.recommended_model();

    set_status(&app, "checking model...");
    let entry = model_manager::find_model(model_id).expect("model in manifest");
    let model_path = model_manager::download_model(entry)?;

    set_status(&app, "loading model...");
    let transcriber = asr::Transcriber::load(model_path.to_str().unwrap())?;
    *state.transcriber.lock().unwrap() = Some(transcriber);
    *state.current_model.lock().unwrap() = model_id.to_string();

    let mut injector = injector::Injector::new()?;
    let ptt = hotkeys::PushToTalk::new()?;
    let live = audio::start_stream()?;

    set_status(&app, "idle (hold Ctrl+F9 to talk)");

    let mut buffer: Vec<f32> = Vec::new();
    let mut recording = false;

    loop {
        if let Some(event) = ptt.try_recv() {
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
                        guard.as_ref().map(|t| t.transcribe(&resampled))
                    };
                    if let Some(Ok(text)) = text_result {
                        if !text.is_empty() {
                            injector.inject(&text).ok();
                        }
                    }
                    set_status(&app, "idle (hold Ctrl+F9 to talk)");
                }
            }
        }

        match live.rx.recv_timeout(Duration::from_millis(50)) {
            Ok(chunk) => {
                if recording {
                    let mono = audio::downmix(&chunk, live.channels);
                    buffer.extend_from_slice(&mono);
                }
            }
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    Ok(())
}