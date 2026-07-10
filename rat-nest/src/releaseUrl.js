'use strict';

// rats is the monorepo containing both the Rust CLI and this npm wrapper -
// GitHub Releases (and their binary assets) live there.
const OWNER = 'p-w-g';
const REPO = 'rats';

/**
 * Asset naming contract (must match rats/.github/workflows/release.yml):
 *   rat-<platform>-<arch>.tar.gz   (linux, darwin)
 *   rat-<platform>-<arch>.zip      (win32)
 * No version in the filename - it's implied by the release tag.
 *
 * @param {{ platform: string, arch: string, exe: boolean }} platformInfo
 * @returns {string}
 */
function assetName({ platform, arch, exe }) {
  return `rat-${platform}-${arch}.${exe ? 'zip' : 'tar.gz'}`;
}

function releaseBaseUrl(version) {
  return `https://github.com/${OWNER}/${REPO}/releases/download/v${version}`;
}

/**
 * @param {string} version - the wrapper's own package.json version, which must
 *   always match the Rust CLI version being wrapped.
 * @param {{ platform: string, arch: string, exe: boolean }} platformInfo
 * @returns {string} the full GitHub release asset download URL
 */
function buildAssetUrl(version, platformInfo) {
  return `${releaseBaseUrl(version)}/${assetName(platformInfo)}`;
}

/**
 * @param {string} version
 * @returns {string} URL of the SHA256SUMS file covering every asset in this release
 */
function buildChecksumsUrl(version) {
  return `${releaseBaseUrl(version)}/SHA256SUMS`;
}

module.exports = { buildAssetUrl, buildChecksumsUrl, assetName, OWNER, REPO };
