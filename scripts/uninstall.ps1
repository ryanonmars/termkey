$ErrorActionPreference = "Stop"

$InstallDir = "$env:LOCALAPPDATA\termkey"
$UninstallerPath = Join-Path $InstallDir "unins000.exe"

if (Test-Path $UninstallerPath) {
    $process = Start-Process -FilePath $UninstallerPath -ArgumentList "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART" -Wait -PassThru

    if ($process.ExitCode -ne 0) {
        throw "TermKey uninstaller exited with code $($process.ExitCode)."
    }
} elseif (Test-Path $InstallDir) {
    Remove-Item $InstallDir -Recurse -Force
}

Write-Host "TermKey uninstalled successfully."
Write-Host "Restart your terminal or run: refreshenv"
