#!/usr/bin/env node
"use strict";

const { spawn } = require("node:child_process");
const path = require("node:path");

const targets = {
  "darwin-arm64": "aarch64-apple-darwin",
  "darwin-x64": "x86_64-apple-darwin",
  "linux-arm64": "aarch64-unknown-linux-gnu",
  "linux-x64": "x86_64-unknown-linux-gnu",
};
const target = targets[`${process.platform}-${process.arch}`];
if (!target) {
  console.error(`token-speed: unsupported platform ${process.platform}/${process.arch}. Build from source: https://github.com/libukai/token-speed#quick-start`);
  process.exit(1);
}

const binary = path.join(__dirname, "..", "vendor", target, "token-speed");
const child = spawn(binary, process.argv.slice(2), { stdio: "inherit" });
const handlers = new Map();
for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"]) {
  const handler = () => child.kill(signal);
  handlers.set(signal, handler);
  process.on(signal, handler);
}
child.on("error", (error) => {
  console.error(`token-speed: cannot run native binary (${error.message}). Linux requires glibc 2.35+. Try reinstalling or building from source.`);
  process.exitCode = 1;
});
child.on("exit", (code, signal) => {
  for (const [name, handler] of handlers) process.removeListener(name, handler);
  if (signal) process.kill(process.pid, signal);
  else process.exitCode = code ?? 1;
});
