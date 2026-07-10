'use strict';

const fs = require('fs');
const path = require('path');
const https = require('https');
const crypto = require('crypto');
const { pipeline } = require('stream/promises');
const { DownloadError, ChecksumMismatchError } = require('./errors');

const MAX_REDIRECTS = 5;

// Deliberately using Node's built-in `https` module rather than global fetch:
// on Node 24 / Windows, calling process.exit() anywhere later in a process
// that has ever used fetch() crashes with a libuv assertion
// ("UV_HANDLE_CLOSING", src/win/async.c) — reproduced independently of this
// package's code. exec.js must call process.exit() to forward the wrapped
// CLI's exit code, so fetch is not safe to use here. https.get has no such
// issue and needs no extra dependency.
function get(url, redirectsLeft) {
  return new Promise((resolve, reject) => {
    https
      .get(url, (res) => {
        const { statusCode, headers } = res;
        if (statusCode >= 300 && statusCode < 400 && headers.location) {
          res.resume(); // discard this response body
          if (redirectsLeft <= 0) {
            reject(new Error('Too many redirects'));
            return;
          }
          resolve(get(new URL(headers.location, url).toString(), redirectsLeft - 1));
          return;
        }
        resolve({ statusCode, response: res });
      })
      .on('error', reject);
  });
}

async function fetchOk(url) {
  let statusCode, response;
  try {
    ({ statusCode, response } = await get(url, MAX_REDIRECTS));
  } catch (cause) {
    throw new DownloadError(url, `Network error: ${cause.message}`);
  }

  if (statusCode !== 200) {
    response.resume();
    throw new DownloadError(url, `Server responded with HTTP ${statusCode}`);
  }

  return response;
}

/**
 * Downloads `url` to `destPath`, writing to a `.part` sibling file first and
 * renaming it into place only once the transfer completes successfully. This
 * keeps a failed/interrupted download from being mistaken for a valid cached
 * binary on the next run.
 *
 * @param {string} url
 * @param {string} destPath
 */
async function downloadToFile(url, destPath) {
  const response = await fetchOk(url);
  const partPath = `${destPath}.part`;
  fs.mkdirSync(path.dirname(destPath), { recursive: true });

  try {
    await pipeline(response, fs.createWriteStream(partPath));
  } catch (cause) {
    fs.rmSync(partPath, { force: true });
    throw new DownloadError(url, `Write error: ${cause.message}`);
  }

  fs.renameSync(partPath, destPath);
}

/**
 * Downloads `url` and returns its body as a UTF-8 string. For small text
 * assets only (e.g. SHA256SUMS) - not the atomic-rename path, since there's
 * nothing on disk to leave in a half-written state.
 *
 * @param {string} url
 * @returns {Promise<string>}
 */
async function downloadText(url) {
  const response = await fetchOk(url);
  const chunks = [];
  for await (const chunk of response) {
    chunks.push(chunk);
  }
  return Buffer.concat(chunks).toString('utf8');
}

function sha256File(filePath) {
  return new Promise((resolve, reject) => {
    const hash = crypto.createHash('sha256');
    fs.createReadStream(filePath)
      .on('data', (chunk) => hash.update(chunk))
      .on('error', reject)
      .on('end', () => resolve(hash.digest('hex')));
  });
}

/**
 * Verifies `filePath` against its entry in a SHA256SUMS file's contents
 * (standard `sha256sum` format: "<hash>  <filename>" per line). Deletes
 * `filePath` and throws on any mismatch or missing entry - a downloaded
 * binary is never trusted without a matching checksum.
 *
 * @param {string} filePath
 * @param {string} assetName - the filename as it appears in SHA256SUMS
 * @param {string} checksumsText
 */
async function verifyChecksum(filePath, assetName, checksumsText) {
  const line = checksumsText.split('\n').find((l) => l.trim().endsWith(assetName));
  const expected = line ? line.trim().split(/\s+/)[0] : null;

  if (!expected) {
    fs.rmSync(filePath, { force: true });
    throw new ChecksumMismatchError(assetName, '(no entry in SHA256SUMS)', '(unknown)');
  }

  const actual = await sha256File(filePath);
  if (actual !== expected) {
    fs.rmSync(filePath, { force: true });
    throw new ChecksumMismatchError(assetName, expected, actual);
  }
}

module.exports = { downloadToFile, downloadText, verifyChecksum };
