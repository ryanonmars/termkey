#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const tag = process.argv[2] ?? process.env.GITHUB_REF_NAME;

if (!tag) {
  console.error("usage: node scripts/validate-release-version.mjs <tag>");
  process.exit(1);
}

const expectedVersion = tag.replace(/^v/i, "");

function read(relativePath) {
  return readFileSync(resolve(repoRoot, relativePath), "utf8");
}

function readJsonVersion(relativePath) {
  return JSON.parse(read(relativePath)).version;
}

function readCargoVersion(relativePath) {
  const match = read(relativePath).match(/^version = "([^"]+)"/m);
  if (!match) {
    throw new Error(`Could not find Cargo version in ${relativePath}`);
  }
  return match[1];
}

const checks = [
  ["apps/cli/Cargo.toml", readCargoVersion("apps/cli/Cargo.toml")],
  ["packages/core/package.json", readJsonVersion("packages/core/package.json")],
  ["packages/types/package.json", readJsonVersion("packages/types/package.json")],
  ["apps/extension/package.json", readJsonVersion("apps/extension/package.json")],
  ["apps/extension/manifest.json", readJsonVersion("apps/extension/manifest.json")],
  [
    "apps/extension/public/manifest.json",
    readJsonVersion("apps/extension/public/manifest.json"),
  ],
];

const extensionPackage = JSON.parse(read("apps/extension/package.json"));
checks.push([
  "apps/extension/package.json:@termkey/core",
  extensionPackage.dependencies["@termkey/core"],
]);
checks.push([
  "apps/extension/package.json:@termkey/types",
  extensionPackage.dependencies["@termkey/types"],
]);

const mismatches = checks.filter(([, actualVersion]) => actualVersion !== expectedVersion);

if (mismatches.length > 0) {
  console.error(`Release tag ${tag} does not match checked-in versions:`);
  for (const [path, actualVersion] of mismatches) {
    console.error(`- ${path}: expected ${expectedVersion}, found ${actualVersion}`);
  }
  process.exit(1);
}

console.log(`Release versions match ${expectedVersion}.`);
