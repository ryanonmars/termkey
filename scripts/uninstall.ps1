$ErrorActionPreference = "Stop"

$InstallDir = "$env:LOCALAPPDATA\termkey"
$normalizedInstallDir = $InstallDir.TrimEnd("\")

if (Test-Path $InstallDir) {
    Remove-Item $InstallDir -Recurse -Force
}

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")

if (-not [string]::IsNullOrWhiteSpace($userPath)) {
    $newUserEntries = $userPath -split ";" |
        Where-Object { -not [string]::IsNullOrWhiteSpace($_) } |
        Where-Object { $_.TrimEnd("\") -ine $normalizedInstallDir }

    [Environment]::SetEnvironmentVariable("Path", ($newUserEntries -join ";"), "User")
}

$sessionEntries = $env:PATH -split ";" |
    Where-Object { -not [string]::IsNullOrWhiteSpace($_) } |
    Where-Object { $_.TrimEnd("\") -ine $normalizedInstallDir }

$env:PATH = $sessionEntries -join ";"

Write-Host "TermKey uninstalled successfully."
Write-Host "Restart your terminal or run: refreshenv"
