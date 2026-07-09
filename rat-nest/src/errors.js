'use strict';

/** Thrown when the current OS/CPU combination has no known release asset. */
class UnsupportedPlatformError extends Error {
  constructor(platform, arch) {
    super(
      `rat does not publish a binary for ${platform}/${arch}.\n` +
        'Supported platforms: linux (x64, arm64), darwin (x64, arm64), win32 (x64).\n' +
        'If you believe this platform should be supported, please open an issue at ' +
        'https://github.com/p-w-g/rat/issues.'
    );
    this.name = 'UnsupportedPlatformError';
  }
}

/** Thrown when fetching the release asset fails (network error or bad HTTP status). */
class DownloadError extends Error {
  constructor(url, detail) {
    super(
      `Failed to download the rat binary from:\n  ${url}\n\n${detail}\n\n` +
        'This usually means either the release for this version does not exist yet, ' +
        'or there is a network/proxy issue. You can verify the release manually at ' +
        'https://github.com/p-w-g/rat/releases'
    );
    this.name = 'DownloadError';
  }
}

module.exports = { UnsupportedPlatformError, DownloadError };
