'use strict';

const { UnsupportedPlatformError } = require('./errors');

/**
 * Maps Node's `${process.platform}-${process.arch}` to the Rust target triple
 * used both by the release asset filename and (implicitly) by the GitHub Actions
 * build matrix that produces it.
 *
 * To support a new platform: add one entry here. Nothing else in this package
 * needs to change, as long as the release workflow publishes a matching asset
 * (see releaseAsset.js for the exact filename format).
 */
const PLATFORM_MAP = {
  'linux-x64': { targetTriple: 'x86_64-unknown-linux-gnu', exe: false },
  'linux-arm64': { targetTriple: 'aarch64-unknown-linux-gnu', exe: false },
  'darwin-x64': { targetTriple: 'x86_64-apple-darwin', exe: false },
  'darwin-arm64': { targetTriple: 'aarch64-apple-darwin', exe: false },
  'win32-x64': { targetTriple: 'x86_64-pc-windows-msvc', exe: true },
};

/**
 * @returns {{ targetTriple: string, exe: boolean }}
 * @throws {UnsupportedPlatformError}
 */
function detectPlatform() {
  const key = `${process.platform}-${process.arch}`;
  const entry = PLATFORM_MAP[key];
  if (!entry) {
    throw new UnsupportedPlatformError(process.platform, process.arch);
  }
  return entry;
}

module.exports = { detectPlatform, PLATFORM_MAP };
