'use strict';

/** Thrown when the current OS/CPU combination has no known release asset. */
class UnsupportedPlatformError extends Error {
  constructor(platform, arch) {
    super(
      `rat does not publish a binary for ${platform}/${arch}.\n` +
        'Supported platforms: linux (x64, arm64), darwin (x64, arm64), win32 (x64).\n' +
        'If you believe this platform should be supported, please open an issue at ' +
        'https://github.com/p-w-g/rats/issues.'
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
        'https://github.com/p-w-g/rats/releases'
    );
    this.name = 'DownloadError';
  }
}

/** Thrown when a downloaded archive's SHA-256 hash doesn't match SHA256SUMS. */
class ChecksumMismatchError extends Error {
  constructor(assetName, expected, actual) {
    super(
      `Checksum verification failed for ${assetName}.\n` +
        `  expected: ${expected}\n` +
        `  actual:   ${actual}\n\n` +
        'The downloaded file does not match the published SHA256SUMS and has been ' +
        'discarded. This could mean a corrupted download or a compromised release - ' +
        'do not retry blindly. If this persists, please open an issue at ' +
        'https://github.com/p-w-g/rats/issues.'
    );
    this.name = 'ChecksumMismatchError';
  }
}

/** Thrown when extracting a downloaded archive fails (missing `tar`, corrupt archive). */
class ExtractionError extends Error {
  constructor(archivePath, detail) {
    super(
      `Failed to extract ${archivePath}:\n${detail}\n\n` +
        'rat-nest extracts release archives with the system `tar` command ' +
        '(present by default on macOS, Linux, and Windows 10 1803+). Make sure ' +
        'it is on your PATH.'
    );
    this.name = 'ExtractionError';
  }
}

module.exports = { UnsupportedPlatformError, DownloadError, ChecksumMismatchError, ExtractionError };
