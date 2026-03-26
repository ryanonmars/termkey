$ErrorActionPreference = "Stop"

$setupUrl = "https://github.com/ryanonmars/CryptoKeeper/releases/latest/download/TermKey-Setup.exe"
$setupPath = "$env:TEMP\TermKey-Setup.exe"

Invoke-WebRequest $setupUrl -OutFile $setupPath

$process = Start-Process -FilePath $setupPath -ArgumentList "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART" -Wait -PassThru

if ($process.ExitCode -ne 0) {
    throw "TermKey installer exited with code $($process.ExitCode)."
}

Remove-Item $setupPath -Force -ErrorAction SilentlyContinue

Write-Host "TermKey installed successfully."
Write-Host "Restart your terminal or run: refreshenv"
