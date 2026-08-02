#!/usr/bin/env node

import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const configDir = mkdtempSync(join(tmpdir(), "cta-marketplace-install-"));
const claudeBin = process.env.CLAUDE_BIN || "claude";
const pluginId = "claude-token-analyzer@claude-token-analyzer";

function run(args) {
  const result = spawnSync(claudeBin, args, {
    cwd: repoRoot,
    encoding: "utf8",
    env: { ...process.env, CLAUDE_CONFIG_DIR: configDir },
  });
  if (result.error || result.status !== 0) {
    const detail = [result.stdout, result.stderr].filter(Boolean).join("\n");
    throw new Error(`${claudeBin} ${args.join(" ")} failed: ${detail}`);
  }
  return result.stdout;
}

try {
  run(["plugin", "marketplace", "add", repoRoot]);
  run(["plugin", "install", pluginId, "--config", "output_language=en"]);

  const installed = JSON.parse(run(["plugin", "list", "--json"]));
  const plugin = installed.find((candidate) => candidate.id === pluginId);
  if (!plugin || plugin.version !== "0.3.0" || plugin.enabled !== true) {
    throw new Error(`unexpected installed plugin: ${JSON.stringify(plugin)}`);
  }

  const installedSkill = join(plugin.installPath, "skills", "cta", "SKILL.md");
  if (!existsSync(installedSkill)) {
    throw new Error(`installed skill is missing: ${installedSkill}`);
  }

  const settings = JSON.parse(readFileSync(join(configDir, "settings.json"), "utf8"));
  const language = settings.pluginConfigs?.[pluginId]?.options?.output_language;
  if (language !== "en") {
    throw new Error(`output_language was not persisted: ${JSON.stringify(language)}`);
  }

  console.log(JSON.stringify({ ok: true, version: plugin.version, output_language: language }));
} finally {
  rmSync(configDir, { recursive: true, force: true });
}
