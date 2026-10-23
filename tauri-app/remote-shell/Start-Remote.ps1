param([string]$ApiUrl = 'http://127.0.0.1:15900')
$ErrorActionPreference = 'Stop'
$executable = Join-Path $PSScriptRoot 'target\x86_64-pc-windows-msvc\release\beambench-remote-shell.exe'
if (Test-Path (Join-Path $PSScriptRoot 'beambench-remote-shell.exe')) {
    $executable = Join-Path $PSScriptRoot 'beambench-remote-shell.exe'
}
if (-not (Test-Path $executable)) { throw 'Build the Windows executable first with Build-Windows.ps1.' }
$secret = Read-Host 'BeamBench API token' -AsSecureString
$pointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secret)
$process = New-Object System.Diagnostics.Process
try {
    $process.StartInfo.FileName = $executable
    $process.StartInfo.WorkingDirectory = Split-Path $executable -Parent
    $process.StartInfo.UseShellExecute = $false
    $process.StartInfo.RedirectStandardError = $true
    $process.StartInfo.EnvironmentVariables['BEAMBENCH_REMOTE_URL'] = $ApiUrl
    $process.StartInfo.EnvironmentVariables['BEAMBENCH_REMOTE_TOKEN'] = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($pointer)
    if (-not $process.Start()) { throw 'Could not start the remote client.' }
    $stderr = $process.StandardError.ReadToEndAsync()
    $process.WaitForExit()
    $diagnostic = $stderr.GetAwaiter().GetResult()
    $exitCode = $process.ExitCode
    if ($exitCode -ne 0) {
        throw "Remote client exited with code $exitCode. $diagnostic"
    }
} finally {
    $process.Dispose()
    [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($pointer)
    $secret.Dispose()
}
