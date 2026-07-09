'use strict';

const { spawn } = require('child_process');

/**
 * Runs the rat binary, forwarding argv, stdio, and the child's exit
 * behavior (exit code or terminating signal) back to this process.
 *
 * Resolves the current process's exit code by calling process.exit itself,
 * so callers don't need to handle that — this is meant to be the last thing
 * bin/rat.js does.
 *
 * @param {string} binaryPath
 * @param {string[]} args
 */
function runBinary(binaryPath, args) {
  const child = spawn(binaryPath, args, { stdio: 'inherit' });

  // If someone Ctrl-Cs (or kills) the wrapper process, pass the same signal
  // through to the actual CLI instead of leaving it running detached.
  const forwardedSignals = ['SIGINT', 'SIGTERM', 'SIGHUP'];
  const forwardSignal = (signal) => child.kill(signal);
  for (const signal of forwardedSignals) {
    process.on(signal, forwardSignal);
  }

  child.on('error', (err) => {
    if (err.code === 'EACCES') {
      console.error(`rat: could not execute ${binaryPath} (permission denied).`);
    } else if (err.code === 'ENOENT') {
      console.error(`rat: binary not found at ${binaryPath}. Try reinstalling @p-w-g/ratnest.`);
    } else {
      console.error(`rat: failed to launch: ${err.message}`);
    }
    process.exit(1);
  });

  child.on('exit', (code, signal) => {
    for (const s of forwardedSignals) {
      process.removeListener(s, forwardSignal);
    }
    if (signal) {
      // Re-raise the same signal on ourselves so our own exit status matches
      // what a shell would report for a signal-terminated process.
      process.kill(process.pid, signal);
      return;
    }
    process.exit(code === null ? 1 : code);
  });
}

module.exports = { runBinary };
