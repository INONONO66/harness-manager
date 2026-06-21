#!/usr/bin/env node
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";

const tmp = await mkdtemp(join(tmpdir(), "hm-npm-smoke-"));
try {
  const pack = run("npm", ["pack", "--json", "--pack-destination", tmp], {});
  const packed = JSON.parse(pack.stdout.toString());
  const [artifact] = packed;
  if (!artifact?.filename) {
    throw new Error("npm pack did not report a tarball filename");
  }
  const tarball = join(tmp, artifact.filename);
  const prefix = join(tmp, "prefix");
  const cache = join(tmp, "cache");
  run("npm", ["install", "-g", tarball, "--prefix", prefix, "--cache", cache], {});
  const version = run(join(prefix, "bin", "hm"), ["--version"], {});
  const output = version.stdout.toString().trim();
  if (!/^hm \d+\.\d+\.\d+/.test(output)) {
    throw new Error(`unexpected hm --version output: ${output}`);
  }
  console.log(output);
} finally {
  await rm(tmp, { recursive: true, force: true });
}

function run(program, args, env) {
  const result = spawnSync(program, args, {
    env: { ...process.env, ...env },
    stdio: ["ignore", "pipe", "inherit"]
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`${program} ${args.join(" ")} exited with status ${result.status}`);
  }
  return result;
}
