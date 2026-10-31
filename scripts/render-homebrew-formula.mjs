import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

const versionPattern = /^\d+\.\d+\.\d+$/;
const sha256Pattern = /^[a-f0-9]{64}$/;

const targets = JSON.parse(await readFile(new URL('./release-targets.json', import.meta.url), 'utf8'));

function readChecksums(contents) {
  const checksums = new Map();
  for (const line of contents.trim().split(/\r?\n/)) {
    const match = line.match(/^([a-f0-9]{64})  ([A-Za-z0-9._-]+)$/);
    if (!match || checksums.has(match[2])) throw new Error('invalid or duplicate SHA-256 manifest entry');
    checksums.set(match[2], match[1]);
  }
  for (const target of targets.filter((item) => item.os !== 'windows')) {
    if (!sha256Pattern.test(checksums.get(target.archive) ?? '')) {
      throw new Error(`missing valid sha256 for ${target.archive}`);
    }
  }
  return checksums;
}

function renderFormula(version, checksums) {
  const download = (os, arch) => {
    const target = targets.find((item) => item.os === os && item.arch === arch);
    if (!target) throw new Error(`missing release target ${os}/${arch}`);
    return `      url "https://github.com/ryanonmars/termkey/releases/download/v${version}/${target.archive}"
      sha256 "${checksums.get(target.archive)}"`;
  };
  return `# typed: false
# frozen_string_literal: true

class Termkey < Formula
  desc "CLI encrypted storage for private keys and seed phrases"
  homepage "https://github.com/ryanonmars/termkey"
  version "${version}"
  license "MIT"

  on_macos do
    on_arm do
${download('macos', 'arm64')}
    end
    on_intel do
${download('macos', 'x64')}
    end
  end

  on_linux do
    on_arm do
${download('linux', 'arm64')}
    end
    on_intel do
${download('linux', 'x64')}
    end
  end

  def install
    bin.install "termkey"
  end

  test do
    assert_match "Encrypted storage for", shell_output("#{bin}/termkey --help")
  end
end
`;
}

async function main() {
  const [version, checksumsPath, outputPath] = process.argv.slice(2);
  if (!versionPattern.test(version ?? '')) {
    throw new Error(`invalid version: ${version ?? ''}`);
  }
  if (!checksumsPath) throw new Error('missing SHA256SUMS path');
  const checksums = readChecksums(await readFile(checksumsPath, 'utf8'));
  if (!outputPath) {
    throw new Error('missing output path');
  }

  await mkdir(path.dirname(path.resolve(outputPath)), { recursive: true });
  await writeFile(outputPath, renderFormula(version, checksums));
}

main().catch((error) => {
  console.error(`render-homebrew-formula: ${error.message}`);
  process.exitCode = 1;
});
