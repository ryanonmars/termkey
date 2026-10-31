import assert from 'node:assert/strict';
import { chmod, mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';

test('macOS launcher treats metacharacters in its bundle path as literal text', {
  skip: process.platform !== 'darwin',
}, async (t) => {
  const root = await mkdtemp(path.join(tmpdir(), 'termkey-launcher-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const source = await readFile(new URL('../apps/cli/packaging/macos/Launcher.swift', import.meta.url), 'utf8');
  // Exercise production command construction without opening Terminal.
  const harness = source.slice(0, source.indexOf('var error: NSDictionary?'))
    .replace('let bundleURL = Bundle.main.bundleURL',
      'let bundleURL = URL(fileURLWithPath: CommandLine.arguments[1])') + '\nprint(command)\n';
  const harnessPath = path.join(root, 'main.swift');
  const harnessBinary = path.join(root, 'launcher-test');
  await writeFile(harnessPath, harness);
  const build = spawnSync('swiftc', [harnessPath, '-o', harnessBinary], { encoding: 'utf8' });
  assert.equal(build.status, 0, build.stderr);

  for (const name of ["TermKey space's.app", 'TermKey$(touch injected).app', 'TermKey`touch injected`.app', 'TermKey;touch injected;.app']) {
    const bundle = path.join(root, name);
    const binary = path.join(bundle, 'Contents/Resources/bin/termkey');
    await mkdir(path.dirname(binary), { recursive: true });
    await writeFile(binary, '#!/bin/sh\nprintf "launcher executed\\n"\n');
    await chmod(binary, 0o755);
    const generated = spawnSync(harnessBinary, [bundle], { encoding: 'utf8' });
    assert.equal(generated.status, 0, generated.stderr);
    const launched = spawnSync('/bin/sh', ['-c', generated.stdout.trim()], { cwd: root, encoding: 'utf8' });
    assert.equal(launched.status, 0, launched.stderr);
    assert.match(launched.stdout, /launcher executed/);
    await assert.rejects(readFile(path.join(root, 'injected')), { code: 'ENOENT' });
  }
});

test('macOS uninstaller supplies a standalone cleanup command that preserves backups', {
  skip: process.platform !== 'darwin',
}, async (t) => {
  const root = await mkdtemp(path.join(tmpdir(), 'termkey-uninstaller-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const source = await readFile(new URL('../apps/cli/packaging/macos/Uninstaller.swift', import.meta.url), 'utf8');
  const harness = source.slice(0, source.indexOf('let app = NSApplication.shared'))
    + '\nprint(vaultCleanupCommand(for: retainedVaultDirectory()))\n';
  const harnessPath = path.join(root, 'main.swift');
  const harnessBinary = path.join(root, 'uninstaller-test');
  await writeFile(harnessPath, harness);
  const build = spawnSync('swiftc', [harnessPath, '-o', harnessBinary], { encoding: 'utf8' });
  assert.equal(build.status, 0, build.stderr);
  const vault = path.join(root, "vault '$(touch injected)' folder");
  await mkdir(vault);
  const dataNames = ['vault.ck', 'vault.tmp', 'vault.lock', 'config.json', 'config.tmp', 'config.lock', '.vault.ck.tmp-test', '.config.json.tmp-test'];
  for (const name of dataNames) await writeFile(path.join(vault, name), 'data');
  const backup = path.join(vault, 'backup.termkey');
  await writeFile(backup, 'backup sentinel');
  const generated = spawnSync(harnessBinary, ['--vault-dir', vault], { encoding: 'utf8' });
  assert.equal(generated.status, 0, generated.stderr);
  const cleanup = spawnSync('/bin/sh', ['-c', generated.stdout.trim()], { cwd: root, encoding: 'utf8' });
  assert.equal(cleanup.status, 0, cleanup.stderr);
  for (const name of dataNames) {
    await assert.rejects(readFile(path.join(vault, name)), { code: 'ENOENT' });
  }
  assert.equal(await readFile(backup, 'utf8'), 'backup sentinel');
  await assert.rejects(readFile(path.join(root, 'injected')), { code: 'ENOENT' });
});
