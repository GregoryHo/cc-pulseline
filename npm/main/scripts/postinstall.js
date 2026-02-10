const fs = require('fs');
const path = require('path');
const os = require('os');
const { resolvePackageName } = require('../lib/platform');

const silent = process.env.npm_config_loglevel === 'silent' ||
               process.env.CC_PULSELINE_SKIP_POSTINSTALL === '1';

if (!silent) {
  console.log('Setting up cc-pulseline for Claude Code...');
}

try {
  const platform = process.platform;
  const homeDir = os.homedir();
  const claudeDir = path.join(homeDir, '.claude', 'pulseline');

  fs.mkdirSync(claudeDir, { recursive: true });

  const packageName = resolvePackageName();
  if (!packageName) {
    if (!silent) {
      console.log(`Platform ${process.platform}-${process.arch} not supported for auto-setup`);
    }
    process.exit(0);
  }

  const binaryName = platform === 'win32' ? 'cc-pulseline.exe' : 'cc-pulseline';
  const targetPath = path.join(claudeDir, binaryName);

  const findBinaryPath = () => {
    const possiblePaths = [
      path.join(__dirname, '..', 'node_modules', packageName, binaryName),
      (() => {
        try {
          const packagePath = require.resolve(packageName + '/package.json');
          return path.join(path.dirname(packagePath), binaryName);
        } catch {
          return null;
        }
      })(),
      (() => {
        const currentPath = __dirname;
        const pnpmMatch = currentPath.match(/(.+\.pnpm)[/\\]([^/\\]+)[/\\]/);
        if (pnpmMatch) {
          const pnpmRoot = pnpmMatch[1];
          const packageNameEncoded = packageName.replace('/', '+');

          try {
            const pnpmContents = fs.readdirSync(pnpmRoot);
            const re = new RegExp('^' + packageNameEncoded.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '@');
            const matchingPackage = pnpmContents.find(dir => re.test(dir));

            if (matchingPackage) {
              return path.join(pnpmRoot, matchingPackage, 'node_modules', packageName, binaryName);
            }
          } catch {
            // Fallback
          }
        }
        return null;
      })()
    ].filter(p => p !== null);

    for (const testPath of possiblePaths) {
      if (fs.existsSync(testPath)) {
        return testPath;
      }
    }
    return null;
  };

  const sourcePath = findBinaryPath();
  if (!sourcePath) {
    if (!silent) {
      console.log('Binary package not installed, skipping Claude Code setup');
      console.log('The global cc-pulseline command will still work via npm');
    }
    process.exit(0);
  }

  if (platform === 'win32') {
    fs.copyFileSync(sourcePath, targetPath);
  } else {
    try {
      if (fs.existsSync(targetPath)) {
        fs.unlinkSync(targetPath);
      }
      fs.linkSync(sourcePath, targetPath);
    } catch {
      fs.copyFileSync(sourcePath, targetPath);
    }
    fs.chmodSync(targetPath, '755');
  }

  if (!silent) {
    console.log('cc-pulseline is ready for Claude Code!');
    console.log(`Location: ${targetPath}`);
  }
} catch (error) {
  if (!silent) {
    console.log('Note: Could not auto-configure for Claude Code');
    console.log('The global cc-pulseline command will still work.');
    console.log('You can manually copy cc-pulseline to ~/.claude/pulseline/ if needed');
  }
}
