# Release verification and publication

Release builds use Rust 1.93.0 and the five targets in
`scripts/release-targets.json`. The Apple Silicon ZIP keeps its existing
`termkey-macos-aarch64.zip` name. Intel macOS, Linux x64/ARM64, and Windows x64
use their corresponding native ZIPs. Apple Silicon also retains its DMG with
`Install TermKey.pkg`. Each ZIP contains exactly its executable, `README.txt`,
`LICENSE`, and `THIRD-PARTY-LICENSES.txt`.

## Current evidence and remaining checks

[Run 41230994032](https://github.com/ryanonmars/termkey/actions/runs/41230994032)
at commit `9154be8` passed all 19 validation jobs; the two publication jobs were
skipped. It ran five native test/build jobs, all 25 encrypted-vault exchanges,
native clipboard/console/security tests, package architecture/integrity checks,
checksums, and formula generation. These were **unsigned validation artifacts**.
Do not repeat the full platform suite solely for documentation changes.

Local Apple Silicon verification also exercised a generated formula against
that run's ZIP: install, `brew test`, revision upgrade, `brew test`, and uninstall
in a separately named, keg-only test formula. Synthetic vault/config sentinels
were unchanged. No production tap or existing installation was modified. Tests
evaluate all four formula OS/CPU selections against their exact ZIP/checksum;
native Homebrew install/upgrade tests on the other three hosts remain unexecuted.

Still required: the [standard-user Windows checklist](windows-testing.md),
protected signing/notarization validation, and final downloaded-package checks
on clean native hosts. macOS CI uses macOS 15; it does not prove runtime behavior
on the declared macOS 11 minimum. Record the oldest tested OS in release notes;
do not describe the deployment target alone as a runtime test.

## Protected signing rehearsal — no publication

Only the protected `apple-release-validation` environment supplies Apple
credentials. Its existing allowed refs are `master` and `v*` tags. Do not add a
feature-branch exception, copy its secrets elsewhere, or create a production tag
to test signing. An authorized maintainer must first approve and integrate the
reviewed changes into `master` before this no-publication rehearsal:

```bash
gh workflow run release.yml --repo ryanonmars/termkey --ref master -f trusted_build=true
gh run list --repo ryanonmars/termkey --workflow release.yml --limit 5
# Select the new run, verify its exact head SHA, then inspect every job:
gh run view RUN_ID --repo ryanonmars/termkey
gh run download RUN_ID --repo ryanonmars/termkey -n termkey-ready-trusted-validation -D signed-validation
gh run download RUN_ID --repo ryanonmars/termkey -n termkey-formula-trusted-validation -D formula-validation
```

The manual trusted mode runs the release gates, signs/notarizes, and uploads CI
artifacts. It never publishes a GitHub Release or changes the tap. The unsigned
manual mode (`trusted_build=false`) never accesses Apple credentials.

Both Mac executables must be Developer ID Application signed, use hardened
runtime and a secure timestamp, and match `APPLE_TEAM_ID`. The final ZIPs are
submitted to Apple after packaging. Native ZIP verification requires the
extracted executable's signature **and notarization ticket**, then runs its
version/help commands with temporary isolated storage. This online check must
fail if the ticket cannot be verified. ZIPs/standalone tools cannot be stapled.
The PKG and DMG are separately notarized, stapled, and Gatekeeper-assessed;
the DMG embeds the already notarized/stapled installer.

Required secret names: `APPLE_CERTIFICATE_BASE64`, `APPLE_CERTIFICATE_PASSWORD`,
`APPLE_APPLICATION_SIGNING_IDENTITY`, `APPLE_INSTALLER_SIGNING_IDENTITY`,
`APPLE_NOTARY_KEY_BASE64`, `APPLE_NOTARY_KEY_ID`, `APPLE_NOTARY_ISSUER_ID`, and
`APPLE_TEAM_ID`. Their presence does not prove certificate/key validity. Inspect
rejected notarization logs privately. Do not print credentials. Keep the existing
environment restrictions and narrowly scoped `TAP_GITHUB_TOKEN`.

## Final package checks

- Verify all six archives against `SHA256SUMS` from the **same run**. There must
  be five ZIPs and the Apple Silicon DMG; never combine artifacts from runs.
- On matching clean native hosts, check version/help, a synthetic vault and
  settings, reopening, TUI/password input, clipboard behavior, and upgrades with
  retained synthetic data. Linux CI already covers static MUSL linkage, X11,
  Wayland, and safe headless failure; verify the downloaded package's launch.
- On macOS, preserve download quarantine and Gatekeeper. Extract and launch the
  CLI from Terminal. Check its ticket with
  `codesign --verify --strict -R=notarized --check-notarization ./termkey`.
  This needs Apple network access. A bare CLI is not an application bundle;
  Finder double-click and app-bundle assessments are not its installation path.
  See [Apple's CLI guidance](https://developer.apple.com/forums/thread/706379).
- Test the signed DMG installer/uninstaller on a disposable Apple Silicon Mac
  or VM with synthetic data, including upgrade preservation. The installer writes
  `/Applications` and `/usr/local/bin`; do not install over a real working setup
  for release testing. Run `xcrun stapler validate` on PKG/DMG; Gatekeeper checks
  use `spctl -a -vv -t install` for PKG and `spctl -a -vv -t open --context
  context:primary-signature` for DMG. These are already automated in trusted CI.
- Review the generated formula's four URLs/checksums and Ruby syntax. They must
  use these finalized ZIPs, never hashes from the unsigned rehearsal. Test
  Homebrew on the available native hosts without linking over another TermKey
  installation. Formula installation has no vault migration or reset hook.
- Complete the Windows checklist without a repository checkout or runtime
  installation. Windows binaries are unsigned; reputation warnings are possible.

Use only synthetic vaults in fresh absolute `TERMKEY_VAULT_DIR` directories.
Never overwrite a real vault or reset/uninstall a user's working installation.

## Authorized publication sequence

1. Finish manual Windows checks and review the changes/commits. Obtain explicit
   authorization to integrate into `master`; this preparation task does not
   authorize that integration.
2. Run the protected trusted rehearsal from that exact integrated commit.
   Inspect every job, download its complete bundle/formula, and finish the
   signed/downloaded-package checks above. Resolve failures before proceeding.
3. Confirm the Cargo version is the intended unused release version (currently
   `1.1.0`) and no tag, release, or draft already exists for it. Obtain explicit
   authorization for the tag, public release, and live tap update.
4. Create and push `v1.1.0` at the approved commit. This automatically starts
   production release: tests/native builds, fixture exchange, signed Mac ZIPs
   and DMG, portable ZIPs, verification, exact archive-set checks and checksums.
5. The workflow creates a draft only after all gates pass. It verifies the
   draft's tag, ID, and exact seven asset names before publishing. Failed jobs
   block publication; failed uploads leave an unpublished draft for inspection.
   Never force publication around a failed gate.
6. After publication, the separate tap job commits only `Formula/termkey.rb` to
   `ryanonmars/homebrew-termkey`. Verify its four SHA-256 values match the public
   manifest. If this job fails, the release can already be public; repair/rerun
   only the tap update after authorization rather than republishing assets.
7. Download the public archives, recheck checksums, and verify
   `brew install ryanonmars/termkey/termkey` / `brew upgrade termkey`. Record
   tested OS versions and any remaining limits in release notes.

An actual release is not reproducible byte-for-byte once signing timestamps and
notarization/stapling are included. ZIPs are deterministic for identical input
binaries; pinned Rust/dependencies/container constrain builds, while hosted SDKs
and runner images can still change. Checksums always describe finalized assets.
