$ErrorActionPreference = "Stop"

$InstallDir = "$env:LOCALAPPDATA\termkey"
$zipUrl = "https://github.com/ryanonmars/CryptoKeeper/releases/latest/download/termkey-windows-x86_64.zip"
$zipPath = "$env:TEMP\termkey.zip"

New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null

Invoke-WebRequest $zipUrl -OutFile $zipPath
Expand-Archive $zipPath -DestinationPath $InstallDir -Force

$normalizedInstallDir = $InstallDir.TrimEnd("\")
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$userEntries = @()

if (-not [string]::IsNullOrWhiteSpace($userPath)) {
    $userEntries = $userPath -split ";" | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
}

$hasUserEntry = $userEntries | Where-Object { $_.TrimEnd("\") -ieq $normalizedInstallDir }

if (-not $hasUserEntry) {
    $newUserPath = if ([string]::IsNullOrWhiteSpace($userPath)) {
        $InstallDir
    } else {
        (($userEntries + $InstallDir) -join ";")
    }

    [Environment]::SetEnvironmentVariable("Path", $newUserPath, "User")
}

$sessionEntries = $env:PATH -split ";" | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
$hasSessionEntry = $sessionEntries | Where-Object { $_.TrimEnd("\") -ieq $normalizedInstallDir }

if (-not $hasSessionEntry) {
    $env:PATH = if ([string]::IsNullOrWhiteSpace($env:PATH)) {
        $InstallDir
    } else {
        (($sessionEntries + $InstallDir) -join ";")
    }
}

Remove-Item $zipPath -Force -ErrorAction SilentlyContinue

Write-Host "TermKey installed successfully."
Write-Host "Restart your terminal or run: refreshenv"
