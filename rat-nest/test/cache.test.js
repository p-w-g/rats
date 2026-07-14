'use strict';

const { test } = require('node:test');
const assert = require('node:assert/strict');
const os = require('node:os');
const path = require('node:path');
const { resolveCacheRoot, binaryPath } = require('../src/cache');

function withEnvAndPlatform(env, platform, fn) {
  const originalPlatform = process.platform;
  const originalEnv = { ...process.env };
  Object.defineProperty(process, 'platform', { value: platform, configurable: true });
  for (const key of ['XDG_CACHE_HOME', 'LOCALAPPDATA']) delete process.env[key];
  Object.assign(process.env, env);
  try {
    fn();
  } finally {
    Object.defineProperty(process, 'platform', { value: originalPlatform, configurable: true });
    process.env = originalEnv;
  }
}

test('resolveCacheRoot honors XDG_CACHE_HOME on every platform', () => {
  withEnvAndPlatform({ XDG_CACHE_HOME: '/custom/cache' }, 'darwin', () => {
    assert.equal(resolveCacheRoot(), '/custom/cache');
  });
  withEnvAndPlatform({ XDG_CACHE_HOME: '/custom/cache' }, 'win32', () => {
    assert.equal(resolveCacheRoot(), '/custom/cache');
  });
});

test('resolveCacheRoot falls back to the OS-conventional dir per platform', () => {
  withEnvAndPlatform({}, 'darwin', () => {
    assert.equal(resolveCacheRoot(), path.join(os.homedir(), 'Library', 'Caches'));
  });
  withEnvAndPlatform({}, 'linux', () => {
    assert.equal(resolveCacheRoot(), path.join(os.homedir(), '.cache'));
  });
  withEnvAndPlatform({ LOCALAPPDATA: 'C:\\Users\\me\\AppData\\Local' }, 'win32', () => {
    assert.equal(resolveCacheRoot(), 'C:\\Users\\me\\AppData\\Local');
  });
});

test('binaryPath is namespaced by rat-nest, version, and platform-arch, and picks rat.exe on win32', () => {
  withEnvAndPlatform({ XDG_CACHE_HOME: '/cache' }, 'linux', () => {
    assert.equal(
      binaryPath('1.2.3', { platform: 'linux', arch: 'x64', exe: false }),
      path.join('/cache', 'rat-nest', '1.2.3', 'linux-x64', 'rat')
    );
  });
  withEnvAndPlatform({ XDG_CACHE_HOME: '/cache' }, 'win32', () => {
    assert.equal(
      binaryPath('1.2.3', { platform: 'win32', arch: 'x64', exe: true }),
      path.join('/cache', 'rat-nest', '1.2.3', 'win32-x64', 'rat.exe')
    );
  });
});
