import assert from 'node:assert/strict';
import { access, mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const scriptPath = new URL('./render-homebrew-formula.mjs', import.meta.url);
const targets = JSON.parse(await readFile(new URL('./release-targets.json', import.meta.url), 'utf8'));
const brewTargets = targets.filter((item) => item.os !== 'windows');
const validManifest = brewTargets.map((item, index) => `${String(index + 1).repeat(64)}  ${item.archive}`).join('\n') + '\n';

async function createFixture(t, contents = validManifest) {
  const root = await mkdtemp(path.join(tmpdir(), 'termkey-homebrew-formula-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const checksums = path.join(root, 'SHA256SUMS');
  await writeFile(checksums, contents);
  return { root, checksums, output: path.join(root, 'Formula', 'termkey.rb') };
}

function render(version, sha256, output) {
  return spawnSync(process.execPath, [fileURLToPath(scriptPath), version, sha256, output], {
    encoding: 'utf8',
  });
}

async function outputExists(output) {
  try {
    await access(output);
    return true;
  } catch {
    return false;
  }
}

test('formula selects the exact public ZIP and checksum for all four Homebrew hosts', async (t) => {
  const { output, checksums } = await createFixture(t);

  const result = render('1.0.1', checksums, output);

  assert.equal(result.status, 0, result.stderr);
  const formula = await readFile(output, 'utf8');
  assert.match(formula, /url "https:\/\/github\.com\/ryanonmars\/termkey\/releases\/download\/v1\.0\.1\/termkey-macos-aarch64\.zip"/);
  assert.match(formula, /version "1\.0\.1"/);
  assert.match(formula, /license "MIT"/);
  assert.match(formula, /bin\.install "termkey"/);
  assert.doesNotMatch(formula, /browser-extension|pkgshare|native-host|depends_on :macos|depends_on arch/);
  assert.match(formula, /assert_match "Encrypted storage for", shell_output\("#\{bin\}\/termkey --help"\)/);
  assert.match(formula, /\n$/);

  // Evaluate Homebrew's conditional DSL, rather than matching URLs without
  // proving their association with an operating system and architecture.
  for (const [index, target] of brewTargets.entries()) {
    const ruby = `
      class Formula
        def self.desc(*); end
        def self.homepage(*); end
        def self.version(*); end
        def self.license(*); end
        def self.on_macos(&b); class_eval(&b) if ENV.fetch('TEST_OS') == 'macos'; end
        def self.on_linux(&b); class_eval(&b) if ENV.fetch('TEST_OS') == 'linux'; end
        def self.on_arm(&b); class_eval(&b) if ENV.fetch('TEST_ARCH') == 'arm64'; end
        def self.on_intel(&b); class_eval(&b) if ENV.fetch('TEST_ARCH') == 'x64'; end
        def self.url(v); puts v; end
        def self.sha256(v); puts v; end
        def self.test(&b); end
      end
      load ARGV.fetch(0)
    `;
    const selected = spawnSync('ruby', ['-e', ruby, output], {
      encoding: 'utf8', env: { ...process.env, TEST_OS: target.os, TEST_ARCH: target.arch },
    });
    assert.equal(selected.status, 0, selected.stderr);
    assert.deepEqual(selected.stdout.trim().split('\n'), [
      `https://github.com/ryanonmars/termkey/releases/download/v1.0.1/${target.archive}`,
      String(index + 1).repeat(64),
    ]);
  }
});

test('rejects an invalid version before writing the output', async (t) => {
  const { output, checksums } = await createFixture(t);

  const result = render('v1.0.1', checksums, output);

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /invalid version/i);
  assert.equal(await outputExists(output), false);
});

test('rejects invalid, missing, or duplicate checksums before writing the output', async (t) => {
  for (const contents of [
    validManifest.replace('1'.repeat(64), 'A'.repeat(64)),
    validManifest.split('\n').slice(1).join('\n'),
    validManifest + validManifest.split('\n')[0] + '\n',
  ]) {
    const { output, checksums } = await createFixture(t, contents);

    const result = render('1.0.1', checksums, output);

    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /sha-256|sha256/i);
    assert.equal(await outputExists(output), false);
  }
});
