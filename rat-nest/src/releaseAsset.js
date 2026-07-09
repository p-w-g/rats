'use strict';

// The Rust CLI's repo — where GitHub Releases (and their binary assets) live.
// The npm package can be renamed/republished independently of this.
const OWNER = 'p-w-g';
const REPO = 'rat';

/**
 * Asset naming contract (must match what the rat release workflow uploads):
 *   rat-<target-triple>        for unix targets
 *   rat-<target-triple>.exe    for windows targets
 * attached to the GitHub Release tagged `v<version>`.
 *
 * @param {{ targetTriple: string, exe: boolean }} platform
 * @returns {string}
 */
function assetName({ targetTriple, exe }) {
  return `rat-${targetTriple}${exe ? '.exe' : ''}`;
}

/**
 * @param {string} version - the wrapper's own package.json version, which must
 *   always match the Rust CLI version being wrapped.
 * @param {{ targetTriple: string, exe: boolean }} platform
 * @returns {string} the full GitHub release asset download URL
 */
function buildAssetUrl(version, platform) {
  const tag = `v${version}`;
  return `https://github.com/${OWNER}/${REPO}/releases/download/${tag}/${assetName(platform)}`;
}

module.exports = { buildAssetUrl, assetName, OWNER, REPO };
