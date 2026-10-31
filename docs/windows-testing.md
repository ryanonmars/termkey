# Windows portable ZIP check

Run as an ordinary user, first in Windows Terminal → PowerShell, then in a
regular PowerShell console. No repository, Rust, Cargo, Git, installer, runtime
download, or administrator access should be needed. Use only the synthetic
values below. Record the ZIP's version/checksum and Windows/terminal versions.

## Download and isolate

Download `termkey-windows-x64.zip` and `SHA256SUMS` from the same release into
Downloads. Before publication, use the Windows ZIP and checksum manifest from
the same validated workflow's complete release bundle. Do not mix runs.

For the already validated build, open [run 41230994032](https://github.com/ryanonmars/termkey/actions/runs/41230994032)
in your Windows browser, scroll to **Artifacts**, and download
`termkey-ready-unsigned-dry-run`. Extract that outer artifact ZIP, then place its
`termkey-windows-x64.zip` and `SHA256SUMS` in Downloads for the commands below.
The inner ZIP contains the Windows executable; no repository checkout is needed.

Keep this PowerShell session open for all steps. These commands create a fresh
test folder and override **both** vault and configuration paths for this process;
your existing `%USERPROFILE%\.termkey` is not used. Do not run a bare `termkey`
command during this test. Stay outside the test vault directory.

```powershell
$previousVaultOverride = [Environment]::GetEnvironmentVariable('TERMKEY_VAULT_DIR', 'Process')
$testRootCreated = $false
$testZip = Join-Path $env:USERPROFILE 'Downloads\termkey-windows-x64.zip'
$testManifest = Join-Path $env:USERPROFILE 'Downloads\SHA256SUMS'
$expected = @(Get-Content -LiteralPath $testManifest | Where-Object {
    $_ -cmatch '^[a-f0-9]{64}  termkey-windows-x64\.zip$'
})
if ($expected.Count -ne 1) { throw 'Missing or duplicate Windows checksum' }
if ((Get-FileHash -LiteralPath $testZip -Algorithm SHA256).Hash.ToLowerInvariant() -cne
    $expected[0].Substring(0, 64)) { throw 'Checksum mismatch; stop' }

$testRoot = Join-Path $env:TEMP ('TermKey-validation-' + [guid]::NewGuid().ToString('N'))
if (Test-Path -LiteralPath $testRoot) { throw 'Test directory already exists' }
New-Item -ItemType Directory -Path $testRoot -ErrorAction Stop | Out-Null
$testRootCreated = $true
$testPackage = Join-Path $testRoot 'package'
$testVault = Join-Path $testRoot 'vault'
Expand-Archive -LiteralPath $testZip -DestinationPath $testPackage -ErrorAction Stop
$testExe = Join-Path $testPackage 'termkey.exe'
$env:TERMKEY_VAULT_DIR = $testVault
function Invoke-TermKeyTest {
    if (!$testVault -or $env:TERMKEY_VAULT_DIR -cne $testVault -or
        $testVault -cne (Join-Path $testRoot 'vault')) { throw 'Isolation lost; stop' }
    & $testExe @args
    if ($LASTEXITCODE -ne 0) { throw "TermKey failed: $LASTEXITCODE" }
}
Get-ChildItem -LiteralPath $testPackage -Name
Invoke-TermKeyTest --version
Invoke-TermKeyTest --help
```

Expect exactly `termkey.exe`, `LICENSE`, `README.txt`, and
`THIRD-PARTY-LICENSES.txt`. Stop on missing-DLL/runtime prompts, unexpected
administrator prompts, security warnings you cannot verify, or launch errors.
Do not disable Windows security features to make the test pass.

## Create, reopen, and inspect

```powershell
Invoke-TermKeyTest init
Invoke-TermKeyTest add
Invoke-TermKeyTest list
Invoke-TermKeyTest config --show
Get-ChildItem -LiteralPath $testVault -Name
Get-Acl -LiteralPath $testVault | Format-List Owner, AreAccessRulesProtected, Access
Get-Acl -LiteralPath (Join-Path $testVault 'vault.ck') | Format-List Owner, AreAccessRulesProtected, Access
Get-Acl -LiteralPath (Join-Path $testVault 'config.json') | Format-List Owner, AreAccessRulesProtected, Access
```

- At `init`, enter `Synthetic-master-Only!42` twice **at the hidden prompts**;
  do not put it in a command. Decline optional recovery for this smoke test.
- At `add`, name the entry `WindowsSmoke`, choose **Password → Enter manually**,
  and enter `Synthetic-entry-Only!42` twice. Leave optional metadata blank.
- Master-password and manually entered secret prompts must not echo characters.
  Reopening with `list` must show the saved entry after unlocking.
- `config --show` must point into `$testVault`; vault/config files must exist
  there. ACLs should be protected against inheritance and grant access to your
  current user, without grants to other users/groups. Record unexpected grants
  as a failure; do not print file contents or security descriptors into a public log.

## TUI and clipboard

```powershell
Invoke-TermKeyTest config --clipboard-timeout 10
Invoke-TermKeyTest
```

Unlock with the synthetic master password. Check readable layout, arrow keys,
`?` help, Escape, and entry selection. Use **Shift+C** on `WindowsSmoke`; within
10 seconds, paste into a disposable Notepad window and confirm the synthetic
secret. Clear that Notepad text. After the timeout, a fresh paste should no
longer insert the secret. Copy again, then copy unrelated text in Notepad before
the timeout: TermKey must retain that newer clipboard content. Keep TermKey
running while checking timed clearing. Exit with **Ctrl+Q** and confirm normal
PowerShell typing/echo returns. Reopen and exit once more.

Clipboard clearing is best effort and cannot remove clipboard history, cloud
clipboard, or copies retained by other applications. Use no real secrets here.
Repeat this section in the regular PowerShell console using a fresh setup block;
environment overrides and helper functions do not transfer between sessions.

## Update and uninstall

```powershell
Invoke-TermKeyTest update
$testVaultFile = Join-Path $testVault 'vault.ck'
$testVaultHash = (Get-FileHash -LiteralPath $testVaultFile -Algorithm SHA256).Hash
Invoke-TermKeyTest uninstall
```

The update command should report the current status or Windows ZIP instructions;
it must not ask for Homebrew, Rust, an installer, or administrator privileges.
For a future upgrade, close all copies, validate the new ZIP, extract into a
fresh folder, and replace only the executable in your chosen installation
directory. Never replace vault/config files with files from an archive.

Uninstall should print a PowerShell removal command and explicitly preserve
vault/settings. It should not delete the running executable. Test removal only
of this isolated copy after all TUI sessions have closed:

```powershell
if ($testExe -cne (Join-Path $testRoot 'package\termkey.exe')) { throw 'Wrong executable; stop' }
Remove-Item -LiteralPath $testExe -ErrorAction Stop
if ((Get-FileHash -LiteralPath $testVaultFile -Algorithm SHA256).Hash -cne $testVaultHash) {
    throw 'Vault changed during executable removal'
}
```

Restore the process override even if a test failed, then remove **only** the
generated test folder. This cleanup intentionally removes synthetic test data;
normal uninstall retains vault data. Exported backups elsewhere are separate.

```powershell
[Environment]::SetEnvironmentVariable('TERMKEY_VAULT_DIR', $previousVaultOverride, 'Process')
Remove-Item Function:\Invoke-TermKeyTest -ErrorAction SilentlyContinue
if ($testRootCreated -and $testRoot -and (Split-Path -Leaf $testRoot) -match '^TermKey-validation-[a-f0-9]{32}$' -and
    (Split-Path -Parent $testRoot) -eq $env:TEMP) {
    Remove-Item -LiteralPath $testRoot -Recurse -ErrorAction Stop
} else { throw 'Unexpected cleanup path; remove no files' }
```

No Windows execution is implied by this checklist. Report pass/fail for launch,
no runtime installation, hidden input, vault reopen, ACLs, both console UIs,
clipboard clearing/ownership, update guidance, and removal with retained vault.
