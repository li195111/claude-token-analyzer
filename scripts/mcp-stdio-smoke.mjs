#!/usr/bin/env node

// Adapted from CodeKB mcp.stdio-transport-health-contract@1.0.0.
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";

const separator = process.argv.indexOf("--");
if (separator === -1 || separator === process.argv.length - 1) {
  console.error("usage: mcp-stdio-smoke.mjs -- <command> [args...]");
  process.exit(2);
}

const command = process.argv[separator + 1];
const args = process.argv.slice(separator + 2);
const timeoutMs = Number(process.env.MCP_SMOKE_TIMEOUT_MS || 8000);
const child = spawn(command, args, {
  stdio: ["pipe", "pipe", "pipe"],
  env: process.env,
});

const responses = new Map();
let stdoutProtocolViolation = "";
let exited = false;
let requestedStop = false;

const lines = createInterface({ input: child.stdout });
lines.on("line", (line) => {
  if (!line.trim()) return;
  try {
    const message = JSON.parse(line);
    if (Object.prototype.hasOwnProperty.call(message, "id")) {
      responses.set(message.id, message);
    }
  } catch {
    stdoutProtocolViolation = line;
  }
});

child.stderr.on("data", (chunk) => process.stderr.write(chunk));
child.on("exit", (code, signal) => {
  exited = true;
  if (!requestedStop && code !== 0) {
    console.error(`MCP server exited with code=${code} signal=${signal}`);
  }
});

function send(message) {
  child.stdin.write(`${JSON.stringify(message)}\n`);
}

async function waitFor(id) {
  const started = Date.now();
  while (Date.now() - started < timeoutMs) {
    if (stdoutProtocolViolation) {
      throw new Error(`stdout contained non-JSON MCP data: ${stdoutProtocolViolation}`);
    }
    if (responses.has(id)) return responses.get(id);
    if (exited) throw new Error(`server exited before response id=${id}`);
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
  throw new Error(`timed out waiting for response id=${id}`);
}

try {
  send({
    jsonrpc: "2.0",
    id: 1,
    method: "initialize",
    params: {
      protocolVersion: "2025-06-18",
      capabilities: {},
      clientInfo: { name: "cta-mcp-stdio-smoke", version: "1.0.0" },
    },
  });
  const init = await waitFor(1);
  if (init.error) throw new Error(`initialize error: ${JSON.stringify(init.error)}`);

  const server = init.result?.serverInfo;
  if (server?.name !== "cta-mcp-server" || server?.version !== "0.3.0") {
    throw new Error(`unexpected server identity: ${JSON.stringify(server)}`);
  }

  send({ jsonrpc: "2.0", method: "notifications/initialized" });
  send({ jsonrpc: "2.0", id: 2, method: "tools/list", params: {} });
  const tools = await waitFor(2);
  if (tools.error) throw new Error(`tools/list error: ${JSON.stringify(tools.error)}`);

  const count = Array.isArray(tools.result?.tools) ? tools.result.tools.length : 0;
  if (count !== 8) throw new Error(`expected 8 tools, received ${count}`);

  console.log(JSON.stringify({ ok: true, tools: count, version: server.version }));
  child.stdin.end();
  requestedStop = true;
  child.kill("SIGTERM");
} catch (error) {
  console.error(error.message);
  child.kill("SIGTERM");
  process.exit(1);
}
