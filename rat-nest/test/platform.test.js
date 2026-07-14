'use strict';

const { test } = require('node:test');
const assert = require('node:assert/strict');

function withProcessPlatformArch(platform, arch, fn) {
  const originalPlatform = process.platform;
  const originalArch = process.arch;
  Object.defineProperty(process, 'platform', { value: platform, configurable: true });
  Object.defineProperty(process, 'arch', { value: arch, configurable: true });
  try {
    fn();
  } finally {
    Object.defineProperty(process, 'platform', { value: originalPlatform, configurable: true });
    Object.defineProperty(process, 'arch', { value: originalArch, configurable: true });
  }
}

test('detectPlatform returns exe: true only for win32', () => {
  delete require.cache[require.resolve('../src/platform')];
  const { detectPlatform } = require('../src/platform');

  withProcessPlatformArch('win32', 'x64', () => {
    assert.deepEqual(detectPlatform(), { platform: 'win32', arch: 'x64', exe: true });
  });

  withProcessPlatformArch('linux', 'arm64', () => {
    assert.deepEqual(detectPlatform(), { platform: 'linux', arch: 'arm64', exe: false });
  });

  withProcessPlatformArch('darwin', 'x64', () => {
    assert.deepEqual(detectPlatform(), { platform: 'darwin', arch: 'x64', exe: false });
  });
});

test('detectPlatform throws UnsupportedPlatformError for unknown combos', () => {
  const { detectPlatform } = require('../src/platform');
  const { UnsupportedPlatformError } = require('../src/errors');

  withProcessPlatformArch('freebsd', 'x64', () => {
    assert.throws(() => detectPlatform(), UnsupportedPlatformError);
  });

  withProcessPlatformArch('linux', 'ia32', () => {
    assert.throws(() => detectPlatform(), UnsupportedPlatformError);
  });
});
