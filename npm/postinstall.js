#!/usr/bin/env node
import { createHash } from "node:crypto";
import { createWriteStream, existsSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { chmod, lstat, mkdtemp, rename } from "node:fs/promises";
import { get } from "node:https";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");
const packageJson = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const dryRun = process.argv.includes("--dry-run") || process.env.HM_NPM_DRY_RUN === "1";
const skip = process.env.HM_NPM_SKIP_DOWNLOAD === "1";
const repo = process.env.HM_INSTALL_REPO ?? "INONONO66/harness-manager";
const version = process.env.HM_INSTALL_VERSION ?? `v${packageJson.version}`;

const targets = {
  "linux:x64": "hm-x86_64-linux.tar.gz",
  "linux:arm64": "hm-aarch64-linux.tar.gz",
  "darwin:x64": "hm-x86_64-darwin.tar.gz",
  "darwin:arm64": "hm-aarch64-darwin.tar.gz"
};

const asset = targets[`${process.platform}:${process.arch}`];
if (!asset) {
  console.error(`unsupported platform for hm npm package: ${process.platform}/${process.arch}`);
  process.exit(1);
}

const baseUrl =
  version === "latest"
    ? `https://github.com/${repo}/releases/latest/download`
    : `https://github.com/${repo}/releases/download/${version}`;
const archiveUrl = `${baseUrl}/${asset}`;
const checksumUrl = `${archiveUrl}.sha256`;
const vendorDir = join(root, "npm", "vendor");
const binary = join(vendorDir, process.platform === "win32" ? "hm.exe" : "hm");

if (dryRun) {
  console.log(`dry-run: would install ${archiveUrl} to ${binary}`);
  process.exit(0);
}

if (skip) {
  console.log("hm npm postinstall skipped by HM_NPM_SKIP_DOWNLOAD=1");
  process.exit(0);
}

const tmp = await mkdtemp(join(tmpdir(), "hm-npm-"));
try {
  const archive = join(tmp, asset);
  const checksum = join(tmp, `${asset}.sha256`);
  await download(archiveUrl, archive);
  await download(checksumUrl, checksum);
  verifyChecksum(archive, checksum);
  await installArchive(archive, tmp);
  await chmod(binary, 0o755);
  console.log(`installed hm ${version} for ${process.platform}/${process.arch}`);
} finally {
  rmSync(tmp, { recursive: true, force: true });
}

function download(url, destination) {
  return new Promise((resolve, reject) => {
    const request = get(url, response => {
      if (
        response.statusCode &&
        response.statusCode >= 300 &&
        response.statusCode < 400 &&
        response.headers.location
      ) {
        response.resume();
        download(response.headers.location, destination).then(resolve, reject);
        return;
      }
      if (response.statusCode !== 200) {
        response.resume();
        reject(new Error(`download failed ${response.statusCode}: ${url}`));
        return;
      }
      const file = createWriteStream(destination);
      response.pipe(file);
      file.on("finish", () => file.close(resolve));
      file.on("error", reject);
    });
    request.on("error", reject);
  });
}

function verifyChecksum(archive, checksumFile) {
  const expected = readFileSync(checksumFile, "utf8").trim().split(/\s+/)[0];
  const actual = createHash("sha256").update(readFileSync(archive)).digest("hex");
  if (actual !== expected) {
    throw new Error(`checksum mismatch for ${archive}`);
  }
}

async function installArchive(archive, workDir) {
  const extractDir = join(workDir, "extract");
  mkdirSync(extractDir, { recursive: true });
  const result = spawnSync("tar", ["-xzf", archive, "-C", extractDir, "hm"], {
    stdio: "inherit"
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`tar exited with status ${result.status}`);
  }
  const extracted = join(extractDir, "hm");
  const metadata = await lstat(extracted);
  if (!metadata.isFile() || metadata.isSymbolicLink()) {
    throw new Error("release archive did not contain a regular hm binary");
  }
  mkdirSync(vendorDir, { recursive: true });
  if (existsSync(binary)) {
    rmSync(binary, { force: true });
  }
  await rename(extracted, binary);
}
