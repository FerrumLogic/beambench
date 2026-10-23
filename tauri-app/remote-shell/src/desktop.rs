use beambench_remote_shell::RemoteClient;
use serde_json::{Value, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tauri::{Emitter, Manager, ipc::InvokeBody};

pub fn run() {
    let client = match std::env::var("BEAMBENCH_REMOTE_TOKEN")
        .map_err(|_| "Set BEAMBENCH_REMOTE_TOKEN".to_string())
        .and_then(|token| {
            RemoteClient::new(
                &std::env::var("BEAMBENCH_REMOTE_URL")
                    .unwrap_or_else(|_| "http://127.0.0.1:5900".into()),
                &token,
            )
        })
        .and_then(|client| {
            let directory = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is not set")?;
            client.with_preferences_file(
                std::path::PathBuf::from(directory)
                    .join("FerrumLogic/BeamBenchRemotePreview/ui-settings.json"),
            )
        }) {
        Ok(client) => client,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let ready = Arc::new(AtomicBool::new(false));
    let command_client = client.clone();
    let command_ready = ready.clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(move |invoke| {
            let command = invoke.message.command().to_string();
            let args = match invoke.message.payload() {
                InvokeBody::Json(value) => value.clone(),
                InvokeBody::Raw(_) => { invoke.resolver.reject("Raw IPC is not supported"); return true; }
            };
            let app = invoke.message.webview().app_handle().clone();
            let client = command_client.clone();
            let ready = command_ready.clone();
            invoke.resolver.respond_async(async move {
                let response: Result<Value, String> = async move {
                match command.as_str() {
                    "mark_frontend_ready" => { ready.store(true, Ordering::Release); Ok(Value::Null) }
                    "set_window_title" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let title = args["title"].as_str().unwrap_or("Beam Bench Remote");
                            window.set_title(&format!("{title} — Remote preview (read-only)")).map_err(|e| e.to_string())?;
                        }
                        Ok(Value::Null)
                    }
                    "request_window_close" | "confirm_window_close" => {
                        // Closing this viewer must never shut down the host or its job.
                        if let Some(window) = app.get_webview_window("main") { window.close().map_err(|e| e.to_string())?; }
                        Ok(Value::Null)
                    }
                    _ => client.invoke(&command, args).await,
                }
                }.await;
                response.map_err(tauri::ipc::InvokeError::from)
            });
            true
        })
        .setup(move |app| {
            let app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while !ready.load(Ordering::Acquire) { tokio::time::sleep(Duration::from_millis(100)).await; }
                let mut connected_before = false;
                loop {
                    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
                    let event_client = client.clone();
                    let mut session = tokio::spawn(async move { event_client.events(tx).await });
                    let failure = loop {
                        tokio::select! {
                            event = rx.recv() => {
                                if let Some(event) = event {
                                    if event["type"] == "system.snapshot" {
                                        if let Some(window) = app.get_webview_window("main") {
                                            let _ = window.set_title("Beam Bench Remote — read-only prototype");
                                            // The stock frontend does not consume system.snapshot.
                                            // A reconnect reloads its read-only stores through GETs.
                                            if connected_before { let _ = window.eval("window.location.reload()"); }
                                        }
                                        connected_before = true;
                                    }
                                    let _ = app.emit("app-event", &event);
                                }
                                else { break "Event channel closed".to_string(); }
                            }
                            result = &mut session => {
                                break match result { Ok(Err(e)) => e, _ => "Event session ended".into() };
                            }
                        }
                    };
                    session.abort();
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.set_title("Beam Bench Remote — DISCONNECTED, reconnecting (read-only)");
                    }
                    // No command requests are queued or replayed here. The next
                    // successful connection begins with a fresh system.snapshot.
                    let _ = app.emit("app-event", json!({"type":"system.warning","payload":{"message":format!("Remote connection lost: {failure}. Reconnecting…")}}));
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Cannot start Beam Bench remote viewer");
}
