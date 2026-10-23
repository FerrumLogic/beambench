$ErrorActionPreference = 'Stop'
if (-not [Environment]::Is64BitOperatingSystem) { throw 'Windows x64 is required.' }
Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    & npm ci
    if ($LASTEXITCODE -ne 0) { throw 'npm ci failed' }
    & npm run build
    if ($LASTEXITCODE -ne 0) { throw 'Frontend build failed' }
    Push-Location $PSScriptRoot
    try {
        & cargo build --release --target x86_64-pc-windows-msvc
        if ($LASTEXITCODE -ne 0) { throw 'Remote shell build failed' }
        Write-Host "Executable: $PSScriptRoot\target\x86_64-pc-windows-msvc\release\beambench-remote-shell.exe"
    } finally { Pop-Location }
} finally { Pop-Location }
