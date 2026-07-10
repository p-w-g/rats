'use strict';

const fs = require('fs');
const os = require('os');
const path = require('path');
const { execFileSync } = require('child_process');
const { detectPlatform } = require('./platform');
const { buildAssetUrl, buildChecksumsUrl, assetName } = require('./releaseUrl');
const { downloadToFile, downloadText, verifyChecksum } = require('./download');
const { ExtractionError } = require('./errors');

const packageJson = require('../package.json');

/**
 * Resolves the per-user OS cache directory. XDG_CACHE_HOME is honored first
 * on every platform (not just Linux) since it's the explicit user override.
 */
function resolveCacheRoot() {
  if (process.env.XDG_CACHE_HOME) {
    return process.env.XDG_CACHE_HOME;
  }
  if (process.platform === 'darwin') {
    return path.join(os.homedir(), 'Library', 'Caches');
  }
  if (process.platform === 'win32') {
    return process.env.LOCALAPPDATA || path.join(os.homedir(), 'AppData', 'Local');
  }
  return path.join(os.homedir(), '.cache');
}

// The version is baked into the cache path (not just the binary filename) as
// a defensive safeguard against a stale binary being reused after a manual
// downgrade - each version gets its own directory, so there's no eviction
// logic to write, only ever-growing disk usage across upgrades, an acceptable
// trade given how small these binaries are.
function binaryDir(version, platform) {
  return path.join(resolveCacheRoot(), 'rat-nest', version, `${platform.platform}-${platform.arch}`);
}

function binaryPath(version, platform) {
  return path.join(binaryDir(version, platform), platform.exe ? 'rat.exe' : 'rat');
}

// bsdtar (Windows' built-in tar.exe since 10 1803, and macOS/Linux's tar)
// autodetects gzip vs zip by content, so one code path extracts both archive
// formats without branching per OS or adding an archive-extraction dependency.
function extractArchive(archivePath, destDir) {
  fs.mkdirSync(destDir, { recursive: true });
  try {
    execFileSync('tar', ['xf', archivePath, '-C', destDir], { stdio: 'ignore' });
  } catch (cause) {
    throw new ExtractionError(archivePath, cause.message);
  }
}

/**
 * Ensures the platform-appropriate rat binary is present in the cache,
 * downloading and verifying it on demand if necessary, and returns its
 * absolute path.
 *
 * @returns {Promise<string>}
 */
async function ensureBinary() {
  const platform = detectPlatform();
  const version = packageJson.version;
  const dest = binaryPath(version, platform);

  if (fs.existsSync(dest)) {
    return dest;
  }

  const asset = assetName(platform);
  const archivePath = path.join(os.tmpdir(), `rat-nest-${version}-${asset}`);

  await downloadToFile(buildAssetUrl(version, platform), archivePath);
  const checksums = await downloadText(buildChecksumsUrl(version));
  await verifyChecksum(archivePath, asset, checksums);

  extractArchive(archivePath, binaryDir(version, platform));
  fs.rmSync(archivePath, { force: true });

  if (!platform.exe) {
    fs.chmodSync(dest, 0o755);
  }

  return dest;
}

module.exports = { ensureBinary, binaryPath, resolveCacheRoot };
