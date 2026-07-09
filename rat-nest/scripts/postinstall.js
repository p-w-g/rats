'use strict';

// Best-effort predownload so the first `rat` invocation doesn't pay the
// download latency. This is intentionally allowed to fail without failing
// `npm install`: environments that run installs offline or with restricted
// network access (sandboxed CI, corporate proxies) would otherwise break
// entirely. bin/rat.js calls the same ensureBinary() lazily on first run, so
// nothing is lost if this step can't complete — it's just slower once.
const { ensureBinary } = require('../src/installer');

ensureBinary()
  .then(() => {
    process.exit(0);
  })
  .catch((err) => {
    console.warn('[@p-w-g/ratnest] Could not prefetch the rat binary during install.');
    console.warn(`[@p-w-g/ratnest] ${err.message}`);
    console.warn('[@p-w-g/ratnest] It will be downloaded automatically the first time you run `rat`.');
    process.exit(0);
  });
