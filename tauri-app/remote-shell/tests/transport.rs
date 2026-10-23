use beambench_remote_shell::RemoteClient;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

fn backend(status: &str, body: Value) -> (String, Arc<Mutex<String>>, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/api/v1", listener.local_addr().unwrap());
    let received = Arc::new(Mutex::new(String::new()));
    let capture = received.clone();
    let status = status.to_owned();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut data = Vec::new();
        let mut buf = [0; 1024];
        while !data.windows(4).any(|w| w == b"\r\n\r\n") {
            let n = stream.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            data.extend_from_slice(&buf[..n]);
        }
        *capture.lock().unwrap() = String::from_utf8(data).unwrap();
        let body = body.to_string();
        write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    });
    (base, received, handle)
}

#[tokio::test]
async fn status_is_authenticated_and_matches_the_frontend_contract() {
    let (base, received, handle) = backend(
        "200 OK",
        json!({"app_version":"0.2.33","state":"ready","connected":false}),
    );
    let client = RemoteClient::new(&base, TOKEN).unwrap();
    assert_eq!(
        client.invoke("get_app_status", json!({})).await.unwrap(),
        json!({"version":"0.2.33","state":"ready"})
    );
    handle.join().unwrap();
    let request = received.lock().unwrap().to_lowercase();
    assert!(request.starts_with("get /api/v1/app/status http/1.1"));
    assert!(request.contains(&format!("authorization: bearer {TOKEN}")));
}

#[tokio::test]
async fn current_project_passes_through_without_a_second_local_project() {
    let project = json!({"metadata":{"project_name":"Remote"},"layers":[],"objects":[]});
    let (base, _, handle) = backend("200 OK", project.clone());
    let client = RemoteClient::new(&base, TOKEN).unwrap();
    assert_eq!(
        client.invoke("get_project", json!({})).await.unwrap(),
        project
    );
    handle.join().unwrap();
}

#[tokio::test]
async fn authorization_failure_is_reported_without_revealing_the_token() {
    let (base, _, handle) = backend("401 Unauthorized", json!({"error":{"message":TOKEN}}));
    let client = RemoteClient::new(&base, TOKEN).unwrap();
    let error = client.invoke("get_project", json!({})).await.unwrap_err();
    assert!(error.contains("401"));
    assert!(!error.contains(TOKEN));
    handle.join().unwrap();
}

#[tokio::test]
async fn missing_project_is_null_but_authentication_failure_is_not() {
    let (base, _, handle) = backend(
        "404 Not Found",
        json!({"error":{"code":"not_found","message":"No project open"}}),
    );
    let client = RemoteClient::new(&base, TOKEN).unwrap();
    assert_eq!(
        client.invoke("get_project", json!({})).await.unwrap(),
        Value::Null
    );
    handle.join().unwrap();
}

#[tokio::test]
async fn machine_commands_are_rejected_before_any_connection() {
    let client = RemoteClient::new("http://127.0.0.1:1/api/v1", TOKEN).unwrap();
    for command in [
        "start_job",
        "machine_jog",
        "send_gcode_line",
        "resume_job",
        "create_project",
        "replace_project",
    ] {
        let error = client.invoke(command, json!({})).await.unwrap_err();
        assert!(error.contains("read-only"), "{command}: {error}");
    }
}

#[test]
fn rejects_embedded_credentials_and_plain_remote_http() {
    for base in [
        "http://user:password@127.0.0.1:5900",
        "http://192.168.0.106:5900",
        "file:///tmp/server",
        "http://127.0.0.1:5900?token=secret",
    ] {
        assert!(RemoteClient::new(base, TOKEN).is_err());
    }
    assert!(RemoteClient::new("http://127.0.0.1:5900/api/v1", "bad\ntoken").is_err());
}

#[tokio::test]
async fn startup_macros_are_read_as_an_array_without_executing_them() {
    let (base, received, handle) = backend("200 OK", json!({"macros":[],"count":0}));
    let client = RemoteClient::new(&base, TOKEN).unwrap();
    assert_eq!(
        client.invoke("get_macros", json!({})).await.unwrap(),
        json!([])
    );
    handle.join().unwrap();
    assert!(received.lock().unwrap().starts_with("GET /api/v1/macros "));
}

#[tokio::test]
async fn startup_camera_state_passes_through_without_capture() {
    let camera = json!({"selected_camera_id":null,"frame":null,"overlay_visible":false});
    let (base, received, handle) = backend("200 OK", camera.clone());
    let client = RemoteClient::new(&base, TOKEN).unwrap();
    assert_eq!(
        client
            .invoke("get_camera_agent_state", json!({}))
            .await
            .unwrap(),
        camera
    );
    handle.join().unwrap();
    assert!(
        received
            .lock()
            .unwrap()
            .starts_with("GET /api/v1/camera/state ")
    );
}

#[tokio::test]
async fn panel_layout_changes_are_client_local() {
    let (base, received, handle) =
        backend("200 OK", json!({"panel_layout":null,"api_enabled":true}));
    let client = RemoteClient::new(&base, TOKEN).unwrap();
    let layout =
        json!({"layout_version":1,"zones":{},"hidden_panel_ids":[],"right_panel_width":320});
    let result = client
        .invoke("update_app_settings", json!({"panelLayout":layout}))
        .await
        .unwrap();
    assert_eq!(result["panel_layout"], layout);
    assert_eq!(result["api_enabled"], true);
    handle.join().unwrap();
    assert!(
        received
            .lock()
            .unwrap()
            .starts_with("GET /api/v1/app/settings ")
    );
}

#[tokio::test]
async fn panel_layout_survives_a_new_client_process_without_server_writes() {
    let directory = std::env::temp_dir().join(format!(
        "beambench-ui-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = directory.join("ui-settings.json");
    let layout = json!({"layout_version":8,"zones":{},"right_panel_width":380});
    let (base, received, server) =
        backend("200 OK", json!({"panel_layout":null,"api_enabled":true}));
    let client = RemoteClient::new(&base, TOKEN)
        .unwrap()
        .with_preferences_file(path.clone())
        .unwrap();
    client
        .invoke("update_app_settings", json!({"panelLayout":layout}))
        .await
        .unwrap();
    server.join().unwrap();
    assert!(received.lock().unwrap().starts_with("GET "));
    // A second save exercises replacement of the existing preferences file,
    // including Windows rename semantics, without a second server request.
    client
        .invoke("update_app_settings", json!({"panelLayout":layout}))
        .await
        .unwrap();
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(!saved.contains(TOKEN));
    assert!(!saved.contains("api_enabled"));
    let (base, _, server) = backend("200 OK", json!({"panel_layout":null,"api_enabled":true}));
    let restarted = RemoteClient::new(&base, TOKEN)
        .unwrap()
        .with_preferences_file(path)
        .unwrap();
    let settings = restarted
        .invoke("get_app_settings", json!({}))
        .await
        .unwrap();
    assert_eq!(settings["panel_layout"], layout);
    assert_eq!(settings["api_enabled"], true);
    server.join().unwrap();
}

#[tokio::test]
async fn layout_can_be_saved_after_the_host_connection_disappears() {
    let (base, _, server) = backend("200 OK", json!({"panel_layout":null}));
    let client = RemoteClient::new(&base, TOKEN).unwrap();
    client.invoke("get_app_settings", json!({})).await.unwrap();
    server.join().unwrap();
    let layout = json!({"layout_version":8,"zones":{}});
    assert_eq!(
        client
            .invoke("update_app_settings", json!({"panelLayout":layout}))
            .await
            .unwrap()["panel_layout"],
        layout
    );
}

#[tokio::test]
async fn local_preferences_do_not_allow_api_or_laser_settings_to_be_changed() {
    let client = RemoteClient::new("http://127.0.0.1:1", TOKEN).unwrap();
    for args in [
        json!({"apiEnabled":false}),
        json!({"apiPort":1}),
        json!({"panelLayout":{},"machineProfiles":[]}),
    ] {
        assert!(
            client
                .invoke("update_app_settings", args)
                .await
                .unwrap_err()
                .contains("read-only")
        );
    }
    assert!(
        client
            .invoke("update_app_settings", json!({"panelLayout":"bad"}))
            .await
            .unwrap_err()
            .contains("panelLayout")
    );
}

#[tokio::test]
async fn session_reply_is_projected_to_the_existing_machine_service_contract() {
    let (base, _, handle) = backend(
        "200 OK",
        json!({"session_state":"disconnected","job_progress":null}),
    );
    let client = RemoteClient::new(&base, TOKEN).unwrap();
    assert_eq!(
        client.invoke("get_session_state", json!({})).await.unwrap(),
        json!("disconnected")
    );
    handle.join().unwrap();
}

#[tokio::test]
// Tungstenite fixes the callback error type to an unboxed HTTP response.
#[allow(clippy::result_large_err)]
async fn event_stream_uses_bearer_auth_and_delivers_the_server_snapshot() {
    use futures_util::SinkExt;
    use tokio_tungstenite::{
        accept_hdr_async,
        tungstenite::{
            Message,
            handshake::server::{Request, Response},
        },
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client =
        RemoteClient::new(&format!("http://{}", listener.local_addr().unwrap()), TOKEN).unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_hdr_async(stream, |request: &Request, response: Response| {
            assert_eq!(request.uri().path(), "/api/v1/events");
            assert_eq!(
                request.headers()["authorization"],
                format!("Bearer {TOKEN}")
            );
            assert!(!request.headers().contains_key("origin"));
            Ok(response)
        })
        .await
        .unwrap();
        socket.send(Message::Text(json!({"type":"system.snapshot","id":1,"payload":{"machine":{"session_state":"disconnected"}}}).to_string().into())).await.unwrap();
        socket.close(None).await.unwrap();
    });
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    let events = tokio::spawn(async move { client.events(tx).await });
    let snapshot = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(snapshot["type"], "system.snapshot");
    assert!(events.await.unwrap().is_err());
    server.await.unwrap();
}
