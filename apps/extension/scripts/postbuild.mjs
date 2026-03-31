import { cp, mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const extensionRoot = resolve(here, "..");
const repoRoot = resolve(extensionRoot, "..", "..");
const coreDist = resolve(repoRoot, "packages", "core", "dist", "index.js");
const vendoredCore = resolve(extensionRoot, "dist", "vendor", "core", "index.js");
const backgroundPath = resolve(extensionRoot, "dist", "background.js");
const manifestSource = resolve(extensionRoot, "public", "manifest.json");
const manifestTarget = resolve(extensionRoot, "manifest.json");

await mkdir(dirname(vendoredCore), { recursive: true });
await cp(coreDist, vendoredCore);
await cp(manifestSource, manifestTarget);

const background = await readFile(backgroundPath, "utf8");
const rewritten = background.replace(
  /from\s+["']@termkey\/core["']/,
  'from "./vendor/core/index.js"'
);

if (rewritten === background && !background.includes('./vendor/core/index.js')) {
  throw new Error("Expected to rewrite @termkey/core import in dist/background.js");
}

if (rewritten !== background) {
  await writeFile(backgroundPath, rewritten);
}
