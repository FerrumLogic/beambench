use futures_util::{SinkExt, StreamExt};
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use std::time::Duration;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
mod preferences;
use preferences::Preferences;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};
use url::Url;

#[derive(Clone)]
pub struct RemoteClient {
    base: Url,
    token: String,
    http: Client,
    preferences: Arc<Mutex<Preferences>>,
}

impl RemoteClient {
    pub fn new(base: &str, token: &str) -> Result<Self, String> {
        let mut base = Url::parse(base).map_err(|_| "Invalid API address")?;
        if !matches!(base.scheme(), "http" | "https")
            || !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
            || !matches!(base.path().trim_end_matches('/'), "" | "/api/v1")
        {
            return Err("Use an HTTP(S) API address without credentials, query or fragment".into());
        }
        if base.scheme() == "http"
            && !matches!(base.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
        {
            return Err("Plain HTTP requires a loopback SSH tunnel; use HTTPS otherwise".into());
        }
        if token.len() < 32
            || !token.is_ascii()
            || token
                .bytes()
                .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
        {
            return Err("Invalid API token".into());
        }
        base.set_path("/api/v1/");
        let http = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| "Cannot initialize HTTP client")?;
        Ok(Self {
            base,
            token: token.into(),
            http,
            preferences: Arc::new(Mutex::new(Preferences::default())),
        })
    }

    pub fn with_preferences_file(mut self, path: PathBuf) -> Result<Self, String> {
        self.preferences = Arc::new(Mutex::new(Preferences::load(path)?));
        Ok(self)
    }

    fn sanitize(&self, error: impl std::fmt::Display) -> String {
        error.to_string().replace(&self.token, "[redacted]")
    }

    async fn get(&self, path: &str, missing_project_is_null: bool) -> Result<Value, String> {
        let response = self
            .http
            .get(self.base.join(path).map_err(|_| "Invalid API route")?)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| self.sanitize(e))?;
        let status = response.status();
        let body: Value = response
            .json()
            .await
            .map_err(|_| format!("HTTP {status}: invalid JSON response"))?;
        if status == StatusCode::NOT_FOUND
            && missing_project_is_null
            && body.pointer("/error/code").and_then(Value::as_str) == Some("not_found")
            && body.pointer("/error/message").and_then(Value::as_str) == Some("No project open")
        {
            return Ok(Value::Null);
        }
        if !status.is_success() {
            return Err(self.sanitize(format!(
                "HTTP {status}: {}",
                body.pointer("/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("API request failed")
            )));
        }
        Ok(body)
    }

    pub async fn invoke(&self, command: &str, args: Value) -> Result<Value, String> {
        if command == "update_app_settings" {
            let patch = Preferences::validate_patch(&args)?;
            let cached = self
                .preferences
                .lock()
                .map_err(|_| "UI settings lock failed")?
                .cached_remote();
            let remote = match cached {
                Some(value) => value,
                None => self.get("app/settings", false).await?,
            };
            return self
                .preferences
                .lock()
                .map_err(|_| "UI settings lock failed")?
                .update(patch, remote);
        }
        let (path, field) = match command {
            "get_app_status" => ("app/status", ""),
            "get_app_settings" => ("app/settings", ""),
            "get_macros" => ("macros", "/macros"),
            "get_camera_agent_state" => ("camera/state", ""),
            "remote_agent_state" => ("agent/state", ""),
            "get_project" => ("projects/current", ""),
            "get_project_layers" => ("projects/layers", "/layers"),
            "get_project_objects" => ("projects/objects", "/objects"),
            "get_undo_state" => ("agent/state", "/project/undo"),
            "get_machine_profiles" => ("profiles", "/profiles"),
            "get_active_profile_id" => ("profiles", "/active_profile_id"),
            "get_machine_runtime_state" => ("machine/session", ""),
            "get_session_state" => ("machine/session", "/session_state"),
            "get_machine_status" => ("machine/session", "/machine_status"),
            "get_machine_coordinates_valid" => ("machine/session", "/machine_coordinates_valid"),
            "get_job_progress" => ("machine/session", "/job_progress"),
            "get_current_project_path" => ("agent/state", "/project/path"),
            // These are client-local startup hooks. The remote client's camera
            // must not register itself as the server desktop's capture bridge.
            "mark_frontend_ready"
            | "agent_sync_selection"
            | "register_camera_agent_bridge"
            | "unregister_camera_agent_bridge" => return Ok(Value::Null),
            "take_pending_notices" | "check_recovery_files" => return Ok(json!([])),
            "get_asset_data" => {
                let id = args
                    .get("assetId")
                    .and_then(Value::as_str)
                    .ok_or("Missing assetId")?;
                if id.len() != 36 || !id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
                    return Err("Invalid assetId".into());
                }
                let response = self
                    .http
                    .get(self.base.join(&format!("assets/{id}")).unwrap())
                    .bearer_auth(&self.token)
                    .send()
                    .await
                    .map_err(|e| self.sanitize(e))?;
                if !response.status().is_success() {
                    return Err(format!("Asset HTTP {}", response.status()));
                }
                let data = response.bytes().await.map_err(|e| self.sanitize(e))?;
                return Ok(json!(data.to_vec()));
            }
            _ => {
                return Err(format!(
                    "Remote prototype is read-only; command '{command}' is not available"
                ));
            }
        };
        let body = self.get(path, command == "get_project").await?;
        if command == "get_app_settings" {
            return self
                .preferences
                .lock()
                .map_err(|_| "UI settings lock failed")?
                .merge(body);
        }
        if command == "get_app_status" {
            let version = body
                .get("app_version")
                .and_then(Value::as_str)
                .ok_or("Missing API app_version")?;
            if version != "0.2.33" {
                return Err(format!(
                    "Unsupported server version {version}; expected 0.2.33"
                ));
            }
            return Ok(json!({"version":version,"state":body["state"]}));
        }
        if field.is_empty() {
            return Ok(body);
        }
        body.pointer(field)
            .cloned()
            .ok_or_else(|| format!("API response is missing {field}"))
    }

    /// One event session. Reconnection belongs to the shell, never to command
    /// dispatch: no failed command can be replayed when this reconnects.
    pub async fn events(&self, sender: tokio::sync::mpsc::Sender<Value>) -> Result<(), String> {
        let mut url = self.base.join("events").unwrap();
        url.set_scheme(if self.base.scheme() == "https" {
            "wss"
        } else {
            "ws"
        })
        .unwrap();
        let mut request = url
            .as_str()
            .into_client_request()
            .map_err(|_| "Invalid event URL")?;
        request.headers_mut().insert(
            "Authorization",
            format!("Bearer {}", self.token)
                .parse()
                .map_err(|_| "Invalid token")?,
        );
        let (mut socket, _) = tokio::time::timeout(Duration::from_secs(10), connect_async(request))
            .await
            .map_err(|_| "Event connection timed out")?
            .map_err(|e| self.sanitize(e))?;
        loop {
            let message = tokio::time::timeout(Duration::from_secs(25), socket.next())
                .await
                .map_err(|_| "Event heartbeat timed out")?;
            match message {
                Some(Ok(Message::Text(text))) => {
                    let event: Value =
                        serde_json::from_str(&text).map_err(|_| "Invalid event JSON")?;
                    if !event["type"].is_string() || !event["payload"].is_object() {
                        return Err("Invalid event envelope".into());
                    }
                    tokio::time::timeout(Duration::from_secs(5), sender.send(event))
                        .await
                        .map_err(|_| "Event consumer is too slow; refresh remote state")?
                        .map_err(|_| "Event consumer closed")?;
                }
                Some(Ok(Message::Ping(_))) => socket.flush().await.map_err(|e| self.sanitize(e))?,
                Some(Ok(Message::Close(_))) | None => {
                    return Err("Remote event connection closed".into());
                }
                Some(Err(e)) => return Err(self.sanitize(e)),
                _ => {}
            }
        }
    }
}
