const { spawnSync } = require('child_process');

const PACKAGE_MAP = {
  'darwin-x64': '@cc-pulseline/darwin-x64',
  'darwin-arm64': '@cc-pulseline/darwin-arm64',
  'linux-x64': '@cc-pulseline/linux-x64',
  'linux-x64-musl': '@cc-pulseline/linux-x64-musl',
  'linux-arm64': '@cc-pulseline/linux-arm64',
  'linux-arm64-musl': '@cc-pulseline/linux-arm64-musl',
  'win32-x64': '@cc-pulseline/win32-x64',
  'win32-ia32': '@cc-pulseline/win32-x64',
};

function getLibcInfo() {
  try {
    const result = spawnSync('ldd', ['--version'], {
      encoding: 'utf8',
      timeout: 1000,
      stdio: ['pipe', 'pipe', 'pipe']
    });
    const lddOutput = (result.stdout || '') + (result.stderr || '');

    if (lddOutput.includes('musl')) {
      return { type: 'musl' };
    }

    const match = lddOutput.match(/(?:GNU libc|GLIBC).*?(\d+)\.(\d+)/);
    if (match) {
      return { type: 'glibc', major: parseInt(match[1]), minor: parseInt(match[2]) };
    }

    return { type: 'musl' };
  } catch (e) {
    return { type: 'musl' };
  }
}

function needsMusl(libcInfo) {
  return libcInfo.type === 'musl' ||
    (libcInfo.type === 'glibc' && (libcInfo.major < 2 || (libcInfo.major === 2 && libcInfo.minor < 35)));
}

function resolvePlatformKey() {
  const platform = process.platform;
  const arch = process.arch;
  let platformKey = `${platform}-${arch}`;

  if (platform === 'linux') {
    const libcInfo = getLibcInfo();
    if (arch === 'arm64') {
      platformKey = needsMusl(libcInfo) ? 'linux-arm64-musl' : 'linux-arm64';
    } else if (needsMusl(libcInfo)) {
      platformKey = 'linux-x64-musl';
    }
  }

  return platformKey;
}

function resolvePackageName() {
  return PACKAGE_MAP[resolvePlatformKey()] || null;
}

module.exports = { PACKAGE_MAP, resolvePlatformKey, resolvePackageName };
