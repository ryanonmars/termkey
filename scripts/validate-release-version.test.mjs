import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";

const validatorPath = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "validate-release-version.mjs",
);

function writeFixtureFile(root, relativePath, contents) {
  const path = resolve(root, relativePath);
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, contents);
}

function createFixture(t) {
  const root = mkdtempSync(resolve(tmpdir(), "termkey-release-version-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));

  writeFixtureFile(
    root,
    "Cargo.toml",
    `[workspace]
members = ["apps/*"]
resolver = "2"
`,
  );
  writeFixtureFile(
    root,
    "apps/cli/Cargo.toml",
    `[package]
name = "termkey"
version = "0.2.26"
edition = "2021"
`,
  );
  writeFixtureFile(
    root,
    "Cargo.lock",
    `version = 4

[[package]]
name = "termkey"
version = "0.2.26"
`,
  );
  return root;
}

function runValidator(root) {
  return spawnSync(
    process.execPath,
    [validatorPath, "v0.2.26", "--root", root],
    { encoding: "utf8" },
  );
}

test("accepts matching Rust workspace, lockfile, and tag versions", (t) => {
  const root = createFixture(t);

  const result = runValidator(root);

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /Release versions match 0\.2\.26\./);
});

test("rejects a source-less Cargo.lock workspace version mismatch", (t) => {
  const root = createFixture(t);
  writeFixtureFile(
    root,
    "Cargo.lock",
    `version = 4

[[package]]
name = "termkey"
version = "0.2.25"
`,
  );

  const result = runValidator(root);

  assert.equal(result.status, 1);
  assert.match(
    result.stderr,
    /Cargo\.lock:termkey: expected 0\.2\.26, found 0\.2\.25/,
  );
});

test("rejects a release tag version mismatch", (t) => {
  const root = createFixture(t);
  const result = spawnSync(
    process.execPath,
    [validatorPath, "v0.2.25", "--root", root],
    { encoding: "utf8" },
  );
  assert.equal(result.status, 1);
  assert.match(
    result.stderr,
    /release tag v0\.2\.25: expected 0\.2\.26, found 0\.2\.25/,
  );
});
