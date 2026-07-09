#!/usr/bin/env node
'use strict';

const { ensureBinary } = require('../src/installer');
const { runBinary } = require('../src/run');

async function main() {
  let binaryPath;
  try {
    binaryPath = await ensureBinary();
  } catch (err) {
    // ensureBinary throws the friendly, already-formatted errors from
    // src/errors.js — print the message only, no stack trace noise.
    console.error(err.message);
    process.exit(1);
  }

  runBinary(binaryPath, process.argv.slice(2));
}

main();
