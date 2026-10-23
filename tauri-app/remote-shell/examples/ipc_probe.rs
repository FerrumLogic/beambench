//! Test bridge for running the stock frontend in a disposable browser.
use beambench_remote_shell::RemoteClient;
use serde_json::{Value, json};
use std::io::Read;

#[tokio::main]
async fn main() {
    let result = async {
        let mut input = String::new();
        std::io::stdin()
            .read_to_string(&mut input)
            .map_err(|e| e.to_string())?;
        let input: Value = serde_json::from_str(&input).map_err(|e| e.to_string())?;
        let token = std::env::var("BEAMBENCH_REMOTE_TOKEN").map_err(|_| "Missing token")?;
        let client = RemoteClient::new("http://127.0.0.1:5900", &token)?;
        let client = if let Some(path) = std::env::var_os("BEAMBENCH_TEST_PREFERENCES") {
            client.with_preferences_file(path.into())?
        } else {
            client
        };
        client
            .invoke(
                input["command"].as_str().ok_or("Missing command")?,
                input["args"].clone(),
            )
            .await
    }
    .await;
    println!(
        "{}",
        match result {
            Ok(value) => json!({"ok":true,"value":value}),
            Err(error) => json!({"ok":false,"error":error}),
        }
    );
}
