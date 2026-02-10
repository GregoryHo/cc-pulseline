#!/usr/bin/env node
const { spawnSync } = require('child_process');
const path = require('path');
const fs = require('fs');
const os = require('os');
const { resolvePackageName } = require('../lib/platform');

// 1. Priority: Use ~/.claude/pulseline/cc-pulseline if exists
const claudePath = path.join(
  os.homedir(),
  '.claude',
  'pulseline',
  process.platform === 'win32' ? 'cc-pulseline.exe' : 'cc-pulseline'
);

if (fs.existsSync(claudePath)) {
  const result = spawnSync(claudePath, process.argv.slice(2), {
    stdio: 'inherit',
    shell: false
  });
  process.exit(result.status || 0);
}

// 2. Fallback: Use npm package binary
const packageName = resolvePackageName();
if (!packageName) {
  const platformKey = `${process.platform}-${process.arch}`;
  console.error(`Error: Unsupported platform ${platformKey}`);
  console.error('Supported platforms: darwin (x64/arm64), linux (x64/arm64), win32 (x64)');
  console.error('Please visit https://github.com/GregoryHo/cc-pulseline for manual installation');
  process.exit(1);
}

const binaryName = process.platform === 'win32' ? 'cc-pulseline.exe' : 'cc-pulseline';
const binaryPath = path.join(__dirname, '..', 'node_modules', packageName, binaryName);

if (!fs.existsSync(binaryPath)) {
  console.error(`Error: Binary not found at ${binaryPath}`);
  console.error('This might indicate a failed installation or unsupported platform.');
  console.error('Please try reinstalling: npm install -g @cc-pulseline/cc-pulseline');
  console.error(`Expected package: ${packageName}`);
  process.exit(1);
}

const result = spawnSync(binaryPath, process.argv.slice(2), {
  stdio: 'inherit',
  shell: false
});

process.exit(result.status || 0);
