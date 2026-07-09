'use strict';

const fs = require('fs');
const path = require('path');
const https = require('https');
const { pipeline } = require('stream/promises');
const { DownloadError } = require('./errors');

const MAX_REDIRECTS = 5;

// Deliberately using Node's built-in `https` module rather than global fetch:
// on Node 24 / Windows, calling process.exit() anywhere later in a process
// that has ever used fetch() crashes with a libuv assertion
// ("UV_HANDLE_CLOSING", src/win/async.c) — reproduced independently of this
// package's code. run.js must call process.exit() to forward the wrapped
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

module.exports = { downloadToFile };
