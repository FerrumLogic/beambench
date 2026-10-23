use beambench_remote_shell::RemoteClient;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), String> {
    let client = RemoteClient::new(
        &std::env::var("BEAMBENCH_REMOTE_URL").unwrap_or_else(|_| "http://127.0.0.1:5900".into()),
        &std::env::var("BEAMBENCH_REMOTE_TOKEN").map_err(|_| "Set BEAMBENCH_REMOTE_TOKEN")?,
    )?;
    let mut results = serde_json::Map::new();
    for command in ["get_macros", "get_camera_agent_state"] {
        client.invoke(command, json!({})).await?;
        results.insert(command.into(), json!({"ok":true}));
    }
    for command in [
        "get_app_status",
        "get_app_settings",
        "get_project",
        "get_project_layers",
        "get_project_objects",
        "get_undo_state",
        "get_machine_profiles",
        "get_session_state",
        "get_machine_runtime_state",
        "get_machine_coordinates_valid",
        "get_job_progress",
    ] {
        let value = client.invoke(command, json!({})).await?;
        results.insert(command.into(), json!({"ok":true,"shape":if value.is_null() {"null"} else if value.is_array() {"array"} else if value.is_object() {"object"} else {"scalar"}}));
    }
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    let event_client = client.clone();
    let task = tokio::spawn(async move { event_client.events(tx).await });
    let event = tokio::time::timeout(std::time::Duration::from_secs(15), rx.recv())
        .await
        .map_err(|_| "Missing event snapshot")?
        .ok_or("Event connection closed")?;
    task.abort();
    if event["type"] != "system.snapshot" {
        return Err("First event is not a state snapshot".into());
    }
    println!(
        "{}",
        json!({"commands":results,"event":event["type"],"machine_commands_sent":false})
    );
    Ok(())
}
