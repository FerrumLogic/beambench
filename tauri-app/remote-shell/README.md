# BeamBench Remote Preview — Windows 10 x64

First, read-only slice of the remote Tauri client. Uses the **unmodified React frontend from BeamBench v0.2.33** and a separate Tauri shell; there is no local machine backend. The stock server remains unchanged.

Implemented: authenticated HTTP reads, frontend response adapters, project and image asset retrieval, server events, reconnect without replaying commands, local window lifecycle. All editing, save/import and machine-control commands fail explicitly. This is not a finished remote editor/controller.

## Connect

Keep a tunnel open on the laptop (the local port can be changed):

```powershell
ssh -N -L 15900:127.0.0.1:5900 ferrumlogic@192.168.0.106
```

The shell reads `BEAMBENCH_REMOTE_URL` (default localhost:5900) and `BEAMBENCH_REMOTE_TOKEN` from its process environment. `Start-Remote.ps1` prompts for the token without echo and uses localhost:15900. The server token is stored in the server's `beambench/.env`; do not commit or share it. Plain HTTP is limited to loopback tunnel addresses; direct connections require HTTPS. Redirects and environment proxy inheritance are disabled. HTTP commands are never automatically retried.

The stock frontend's file pickers remain local and are not usable for server imports in this slice. Open the project on the host before starting the viewer. The title identifies the read-only prototype and connection loss. Reconnecting reloads the viewer, which reads the current project again; changes made in the server editor while connected are not yet synchronized into the viewer. Closing the client does not stop or disconnect the host's machine.

## Build locally on Windows

Requirements: Node.js 24, Rust stable with `x86_64-pc-windows-msvc`, Visual Studio C++ build tools/Windows SDK, and WebView2 Runtime. Run `Build-Windows.ps1` in PowerShell from the source checkout. It builds the original frontend and the remote shell. Run `Start-Remote.ps1` after creating the SSH tunnel.

`.github/workflows/remote-preview-windows.yml` is a manual Windows build job. It requires no server token, machine access or release credentials. It builds a test executable artifact, not an upstream release. A native Windows run succeeded: https://github.com/FerrumLogic/beambench/actions/runs/40029908728 .

## Verification and limits

Transport contract tests use local HTTP/WebSocket fixtures. A live Rust probe reads eleven command shapes from the installed stock server and receives `system.snapshot`, without changing the project or sending machine commands. The original frontend builds unchanged and its targeted service tests pass.

Linux transport tests do not prove that the Windows shell builds or that the GUI renders correctly. A native Windows x64 build and all eight transport tests passed in GitHub Actions. GUI startup on the target laptop remains untested. The frontend's remote snapshot/reconnect integration and complete command mapping also need UI verification. Do not use this prototype to run a laser job.

License: GPL-3.0-or-later, like upstream. This is an unofficial development prototype; no signing or automatic updates are configured.

## 0.1.1 startup fixes

Macro listing and camera state use their read-only API endpoints. Panel layout and display preferences are stored only on the laptop, in `%LOCALAPPDATA%/FerrumLogic/BeamBenchRemotePreview/ui-settings.json`, and are overlaid on server settings. UI preferences can be saved while disconnected after initial settings have loaded. Tokens, machine profiles and API settings are never written to this file.
