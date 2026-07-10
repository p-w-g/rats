'use strict';

const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const crypto = require('node:crypto');
const { verifyChecksum } = require('../src/download');
const { ChecksumMismatchError } = require('../src/errors');

function tempFileWith(content) {
  const filePath = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'ratnest-test-')), 'rat-linux-x64.tar.gz');
  fs.writeFileSync(filePath, content);
  return filePath;
}

function sha256(content) {
  return crypto.createHash('sha256').update(content).digest('hex');
}

test('verifyChecksum passes and keeps the file when the hash matches', async () => {
  const content = 'fake archive bytes';
  const filePath = tempFileWith(content);
  const checksumsText = `${sha256(content)}  rat-linux-x64.tar.gz\n${sha256('other')}  rat-darwin-x64.tar.gz\n`;

  await verifyChecksum(filePath, 'rat-linux-x64.tar.gz', checksumsText);

  assert.ok(fs.existsSync(filePath));
});

test('verifyChecksum throws and deletes the file when the hash does not match', async () => {
  const filePath = tempFileWith('fake archive bytes');
  const checksumsText = `${sha256('not the same bytes')}  rat-linux-x64.tar.gz\n`;

  await assert.rejects(() => verifyChecksum(filePath, 'rat-linux-x64.tar.gz', checksumsText), ChecksumMismatchError);
  assert.ok(!fs.existsSync(filePath));
});

test('verifyChecksum throws when SHA256SUMS has no entry for the asset', async () => {
  const filePath = tempFileWith('fake archive bytes');
  const checksumsText = `${sha256('unrelated')}  rat-darwin-x64.tar.gz\n`;

  await assert.rejects(() => verifyChecksum(filePath, 'rat-linux-x64.tar.gz', checksumsText), ChecksumMismatchError);
  assert.ok(!fs.existsSync(filePath));
});
