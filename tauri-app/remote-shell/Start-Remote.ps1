param([string]$ApiUrl = 'http://127.0.0.1:15900')
$ErrorActionPreference = 'Stop'
$executable = Join-Path $PSScriptRoot 'target\x86_64-pc-windows-msvc\release\beambench-remote-shell.exe'
if (Test-Path (Join-Path $PSScriptRoot 'beambench-remote-shell.exe')) {
    $executable = Join-Path $PSScriptRoot 'beambench-remote-shell.exe'
}
if (-not (Test-Path $executable)) { throw 'Build the Windows executable first with Build-Windows.ps1.' }
$secret = Read-Host 'BeamBench API token' -AsSecureString
$pointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secret)
$previousUrl = $env:BEAMBENCH_REMOTE_URL
$previousToken = $env:BEAMBENCH_REMOTE_TOKEN
try {
    $env:BEAMBENCH_REMOTE_URL = $ApiUrl
    $env:BEAMBENCH_REMOTE_TOKEN = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($pointer)
    & $executable
    if ($LASTEXITCODE -ne 0) { throw "Remote client exited with code $LASTEXITCODE. Check the API address and token." }
} finally {
    $env:BEAMBENCH_REMOTE_URL = $previousUrl
    $env:BEAMBENCH_REMOTE_TOKEN = $previousToken
    [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($pointer)
    $secret.Dispose()
}
