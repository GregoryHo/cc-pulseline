#!/usr/bin/env node
const fs = require('fs');
const path = require('path');

const version = process.env.GITHUB_REF?.replace('refs/tags/v', '') || process.argv[2];
if (!version) {
  console.error('Error: Version not provided');
  console.error('Usage: GITHUB_REF=refs/tags/v1.0.0 node prepare-packages.js');
  console.error('   or: node prepare-packages.js 1.0.0');
  process.exit(1);
}

console.log(`Preparing packages for version ${version}`);

const platforms = [
  'darwin-x64',
  'darwin-arm64',
  'linux-x64',
  'linux-x64-musl',
  'linux-arm64',
  'linux-arm64-musl',
  'win32-x64'
];

// Prepare platform packages
platforms.forEach(platform => {
  const sourceDir = path.join(__dirname, '..', 'platforms', platform);
  const targetDir = path.join(__dirname, '..', '..', 'npm-publish', platform);

  fs.mkdirSync(targetDir, { recursive: true });

  const templatePath = path.join(sourceDir, 'package.json');
  const packageJson = JSON.parse(fs.readFileSync(templatePath, 'utf8'));

  packageJson.version = version;

  fs.writeFileSync(
    path.join(targetDir, 'package.json'),
    JSON.stringify(packageJson, null, 2) + '\n'
  );

  console.log(`  Prepared @cc-pulseline/${platform} v${version}`);
});

// Prepare main package
const mainSource = path.join(__dirname, '..', 'main');
const mainTarget = path.join(__dirname, '..', '..', 'npm-publish', 'main');

fs.cpSync(mainSource, mainTarget, { recursive: true });

const mainPackageJsonPath = path.join(mainTarget, 'package.json');
const mainPackageJson = JSON.parse(fs.readFileSync(mainPackageJsonPath, 'utf8'));

mainPackageJson.version = version;

if (mainPackageJson.optionalDependencies) {
  Object.keys(mainPackageJson.optionalDependencies).forEach(dep => {
    if (dep.startsWith('@cc-pulseline/')) {
      mainPackageJson.optionalDependencies[dep] = version;
    }
  });
}

fs.writeFileSync(
  mainPackageJsonPath,
  JSON.stringify(mainPackageJson, null, 2) + '\n'
);

console.log(`  Prepared @cc-pulseline/cc-pulseline v${version}`);
console.log(`\nAll packages prepared for version ${version}`);
