$ErrorActionPreference = 'Stop'
$temporary = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $temporary | Out-Null
Copy-Item "$PSScriptRoot/../tauri-app/remote-shell/Start-Remote.ps1" "$temporary/Start-Remote.ps1"
$token = '0123456789abcdef0123456789abcdef'
function global:Read-Host {
    param([string]$Prompt, [switch]$AsSecureString)
    ConvertTo-SecureString '0123456789abcdef0123456789abcdef' -AsPlainText -Force
}
Add-Type -OutputAssembly "$temporary/beambench-remote-shell.exe" -OutputType WindowsApplication -TypeDefinition @'
using System;
public static class LauncherFixture {
    public static void Main() {
        if (Environment.GetEnvironmentVariable("BEAMBENCH_REMOTE_TOKEN") != "0123456789abcdef0123456789abcdef") Environment.Exit(91);
        if (Environment.GetEnvironmentVariable("BEAMBENCH_REMOTE_URL") != "http://127.0.0.1:15900") Environment.Exit(92);
        int code = int.Parse(Environment.GetEnvironmentVariable("BEAMBENCH_LAUNCHER_TEST_EXIT") ?? "0");
        if (code != 0) Console.Error.WriteLine("fixture diagnostic");
        Environment.Exit(code);
    }
}
'@
$env:BEAMBENCH_REMOTE_TOKEN = 'parent-token-must-stay-unchanged'
$env:BEAMBENCH_LAUNCHER_TEST_EXIT = '0'
$global:LASTEXITCODE = $null
& "$temporary/Start-Remote.ps1"
if ($env:BEAMBENCH_REMOTE_TOKEN -ne 'parent-token-must-stay-unchanged') { throw 'Parent environment changed' }
$env:BEAMBENCH_LAUNCHER_TEST_EXIT = '7'
$message = ''
try { & "$temporary/Start-Remote.ps1" } catch { $message = $_.Exception.Message }
if ($message -notmatch 'code 7' -or $message -notmatch 'fixture diagnostic') { throw "Unexpected diagnostic: $message" }
if ($message.Contains($token)) { throw 'Diagnostic contains token' }
Write-Host 'PASS: GUI exit 0, GUI exit 7, stderr, parent environment, no token leak.'
