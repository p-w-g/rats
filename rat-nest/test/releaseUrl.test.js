'use strict';

const { test } = require('node:test');
const assert = require('node:assert/strict');
const { assetName, buildAssetUrl, buildChecksumsUrl, OWNER, REPO } = require('../src/releaseUrl');

test('assetName picks .tar.gz for unix, .zip for windows, no version', () => {
  assert.equal(assetName({ platform: 'linux', arch: 'x64', exe: false }), 'rat-linux-x64.tar.gz');
  assert.equal(assetName({ platform: 'darwin', arch: 'arm64', exe: false }), 'rat-darwin-arm64.tar.gz');
  assert.equal(assetName({ platform: 'win32', arch: 'x64', exe: true }), 'rat-win32-x64.zip');
});

test('buildAssetUrl points at the monorepo release tagged v<version>', () => {
  const url = buildAssetUrl('1.2.3', { platform: 'linux', arch: 'x64', exe: false });
  assert.equal(url, `https://github.com/${OWNER}/${REPO}/releases/download/v1.2.3/rat-linux-x64.tar.gz`);
});

test('buildChecksumsUrl points at SHA256SUMS on the same release', () => {
  const url = buildChecksumsUrl('1.2.3');
  assert.equal(url, `https://github.com/${OWNER}/${REPO}/releases/download/v1.2.3/SHA256SUMS`);
});
