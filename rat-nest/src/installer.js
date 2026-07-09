'use strict';

const fs = require('fs');
const path = require('path');
const { detectPlatform } = require('./platform');
const { buildAssetUrl, assetName } = require('./releaseAsset');
const { downloadToFile } = require('./download');

const packageJson = require('../package.json');

// Cached binaries live inside this package's own install directory rather than
// a shared OS-level cache dir: npm already owns the lifecycle of this
// directory (created on install, wiped on uninstall/reinstall), so there is
// no separate cache-eviction story to build or a per-OS cache path to
// compute. The version is baked into the filename purely as a defensive
// safeguard against a stale binary being reused after a manual downgrade.
const CACHE_DIR = path.join(__dirname, '..', '.bin-cache');

function cachedBinaryPath(version, platform) {
  return path.join(CACHE_DIR, `${version}-${assetName(platform)}`);
}

/**
 * Ensures the platform-appropriate rat binary is present on disk, downloading
 * it on demand if necessary, and returns its absolute path.
 *
 * @returns {Promise<string>}
 */
async function ensureBinary() {
  const platform = detectPlatform();
  const version = packageJson.version;
  const binaryPath = cachedBinaryPath(version, platform);

  if (fs.existsSync(binaryPath)) {
    return binaryPath;
  }

  const url = buildAssetUrl(version, platform);
  await downloadToFile(url, binaryPath);

  if (!platform.exe) {
    fs.chmodSync(binaryPath, 0o755);
  }

  return binaryPath;
}

module.exports = { ensureBinary, cachedBinaryPath, CACHE_DIR };
