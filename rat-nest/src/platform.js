'use strict';

const { UnsupportedPlatformError } = require('./errors');

/**
 * Platforms this package ships a prebuilt binary for - must match the
 * release workflow's build matrix (rats/.github/workflows/release.yml).
 */
const SUPPORTED = new Set(['linux-x64', 'linux-arm64', 'darwin-x64', 'darwin-arm64', 'win32-x64']);

/**
 * @returns {{ platform: string, arch: string, exe: boolean }}
 * @throws {UnsupportedPlatformError}
 */
function detectPlatform() {
  const platform = process.platform;
  const arch = process.arch;
  if (!SUPPORTED.has(`${platform}-${arch}`)) {
    throw new UnsupportedPlatformError(platform, arch);
  }
  return { platform, arch, exe: platform === 'win32' };
}

module.exports = { detectPlatform, SUPPORTED };
