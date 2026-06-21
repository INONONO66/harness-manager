#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const binary = join(here, "..", "vendor", process.platform === "win32" ? "hm.exe" : "hm");

if (!existsSync(binary)) {
  console.error("hm binary is missing from this npm package installation.");
  console.error("Run `npm rebuild harness-manager` or install from GitHub releases.");
  process.exit(1);
}

const result = spawnSync(binary, process.argv.slice(2), { stdio: "inherit" });
if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}
process.exit(result.status ?? 1);
