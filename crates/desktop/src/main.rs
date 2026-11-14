mod overlay;

use rafiflow_core::{
    engine::{Action, Engine},
    store,
};
use std::{
    io::{Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    sync::Mutex,
};
use tauri::Manager;

#[tauri::command]
fn snapshot(engine: tauri::State<Engine>) -> serde_json::Value {
    engine.snapshot()
}
#[tauri::command]
async fn action(
    engine: tauri::State<'_, Engine>,
    request: Action,
) -> Result<serde_json::Value, String> {
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || engine.action(request))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn copy_text(text: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        rafiflow_core::platform::copy(&text).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn renderer_error(message: String) {
    eprintln!("Frontend: {message}");
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = store::data_dir()?;
    let socket = dir.join("control.sock");
    if let Some(command) = std::env::args().nth(1) {
        if command == "--control" {
            let action = std::env::args()
                .nth(2)
                .ok_or("Expected start, stop, or cancel")?;
            if !["start", "stop", "cancel", "show"].contains(&action.as_str()) {
                return Err("Invalid command".into());
            }
            return send_control(&socket, &action, std::time::Duration::from_secs(420));
        }
    }
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("instance.lock"))?;
    if let Err(error) = fs2::FileExt::try_lock_exclusive(&lock) {
        if error.kind() != std::io::ErrorKind::WouldBlock {
            return Err(error.into());
        }
        if std::env::args().any(|argument| argument == "--background") {
            return Ok(());
        }
        return send_control(&socket, "show", std::time::Duration::from_secs(2));
    }
    if socket.exists() {
        std::fs::remove_file(&socket)?;
    }
    let listener = UnixListener::bind(&socket)?;
    rafiflow_core::platform::configure_overlay()?;
    let engine = Engine::start(dir)?;
    let control = engine.clone();
    let mut context = tauri::generate_context!();
    if std::env::args().any(|argument| argument == "--background") {
        if let Some(window) = context
            .config_mut()
            .app
            .windows
            .iter_mut()
            .find(|window| window.label == "main")
        {
            window.visible = false;
        }
    }
    tauri::Builder::default()
        .manage(engine)
        .manage(Mutex::new(lock))
        .invoke_handler(tauri::generate_handler![snapshot, action, copy_text, renderer_error])
        .on_page_load(|webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Started {
                let _ = webview.eval(r#"window.addEventListener('error', e => window.__TAURI__.core.invoke('renderer_error',{message:String(e.message)}));window.addEventListener('unhandledrejection', e => window.__TAURI__.core.invoke('renderer_error',{message:String(e.reason)}));window.addEventListener('securitypolicyviolation', e => window.__TAURI__.core.invoke('renderer_error',{message:e.violatedDirective+': '+e.blockedURI}));"#);
            }
        })
        .setup(move |app| {
            serve_control(listener, control, app.handle().clone());
            overlay::configure(app)?;
            let show =
                tauri::menu::MenuItem::with_id(app, "show", "Open rafiFlow", true, None::<&str>)?;
            let quit = tauri::menu::MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = tauri::menu::Menu::with_items(app, &[&show, &quit])?;
            tauri::tray::TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("rafiFlow")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            if let Some(window) = app.get_webview_window("main") {
                let handle = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = handle.hide();
                    }
                });
            }
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let mut shown = false;
                loop {
                    let engine = handle.state::<Engine>();
                    let phase = engine.status.lock().unwrap().phase.clone();
                    let visible = phase == "recording";
                    if visible != shown {
                        if let Some(window) = handle.get_webview_window("overlay") {
                            let result = if visible {
                                let _ = window.set_max_size(Some(tauri::LogicalSize::new(340.0, 90.0)));
                                let result = window.show();
                                let _ = window.set_size(tauri::LogicalSize::new(340.0, 90.0));
                                result
                            } else {
                                window.hide()
                            };
                            if let Err(error) = result {
                                eprintln!("Overlay: {error}");
                            }
                        }
                        shown = visible;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            });
            Ok(())
        })
        .run(context)?;
    let _ = std::fs::remove_file(socket);
    Ok(())
}

fn send_control(
    socket: &std::path::Path,
    action: &str,
    timeout: std::time::Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.write_all(action.as_bytes())?;
    stream.shutdown(std::net::Shutdown::Write)?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    if !response.starts_with("OK") {
        return Err(response.into());
    }
    println!("{response}");
    Ok(())
}

fn serve_control(listener: UnixListener, control: Engine, handle: tauri::AppHandle) {
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let engine = control.clone();
            let handle = handle.clone();
            std::thread::spawn(move || {
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
                let mut input = String::new();
                let result = (&mut stream)
                    .take(32)
                    .read_to_string(&mut input)
                    .map_err(|e| e.to_string())
                    .and_then(|_| {
                        if input == "show" {
                            let window = handle
                                .get_webview_window("main")
                                .ok_or("Main window unavailable")?;
                            window.show().map_err(|e| e.to_string())?;
                            window.set_focus().map_err(|e| e.to_string())?;
                            return Ok(serde_json::json!({"ok":true}));
                        }
                        let action = match input.as_str() {
                            "start" => Action::Start,
                            "stop" => Action::Stop,
                            "cancel" => Action::Cancel,
                            _ => return Err("Invalid command".to_string()),
                        };
                        engine.action(action)
                    });
                let reply = match result {
                    Ok(_) => "OK".into(),
                    Err(e) => format!("ERROR {e}"),
                };
                let _ = stream.write_all(reply.as_bytes());
            });
        }
    });
}
