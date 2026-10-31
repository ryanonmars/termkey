import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

test('third-party notices cover the current locked dependencies offline', () => {
  const result = spawnSync('python3', ['scripts/generate-third-party-licenses.py', '--check'], { cwd: root, encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  const notices = fs.readFileSync(path.join(root, 'apps/cli/packaging/THIRD-PARTY-LICENSES.txt'), 'utf8');
  assert.match(notices, /depend\/secp256k1\/COPYING/);
  assert.match(notices, /LICENSE-BoringSSL/);
  assert.match(notices, /third_party\/fiat\/LICENSE/);
});

test('stale lock or omitted dependency notices prevent packaging validation', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'termkey-licenses-'));
  try {
    fs.mkdirSync(path.join(directory, 'scripts'));
    fs.mkdirSync(path.join(directory, 'apps/cli/packaging'), { recursive: true });
    for (const file of ['scripts/generate-third-party-licenses.py', 'Cargo.lock', 'apps/cli/packaging/THIRD-PARTY-LICENSES.txt']) {
      fs.copyFileSync(path.join(root, file), path.join(directory, file));
    }
    const execute = () => spawnSync('python3', ['scripts/generate-third-party-licenses.py', '--check'], { cwd: directory, encoding: 'utf8' });
    fs.appendFileSync(path.join(directory, 'Cargo.lock'), '\n# changed dependency lock\n');
    assert.notEqual(execute().status, 0);
    fs.copyFileSync(path.join(root, 'Cargo.lock'), path.join(directory, 'Cargo.lock'));
    const output = path.join(directory, 'apps/cli/packaging/THIRD-PARTY-LICENSES.txt');
    fs.writeFileSync(output, fs.readFileSync(output, 'utf8').replace(/^argon2 [^\n]+\n/m, ''));
    assert.notEqual(execute().status, 0);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});
