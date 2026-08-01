#!/usr/bin/env node
// Execute a wasm32-wasip1 Rust test binary under Node's WASI runtime.
//
// The repository root is preopened at "/" so compile-time absolute fixture
// paths (`CARGO_MANIFEST_DIR`) resolve exactly as they do natively. Test
// binaries only read committed fixtures.
'use strict';

const { WASI } = require('node:wasi');
const fs = require('fs');

const binary = process.argv[2];
if (!binary) {
  console.error('usage: wasm_test_runner.js <binary.wasm> [test args...]');
  process.exit(2);
}

const wasi = new WASI({
  version: 'preview1',
  args: [binary, ...process.argv.slice(3)],
  env: { ...process.env },
  preopens: { '/': '/' },
  // Node's preview1 `proc_exit` throws by default, which makes `start()`
  // swallow the Rust test harness exit code. `returnOnExit` makes `start()`
  // return that code so a failing WASM lane fails the feature-matrix gate.
  returnOnExit: true,
});

WebAssembly.instantiate(fs.readFileSync(binary), {
  wasi_snapshot_preview1: wasi.wasiImport,
})
  .then(({ instance }) => {
    try {
      const exit_code = wasi.start(instance);
      process.exit(typeof exit_code === 'number' ? exit_code : 0);
    } catch (error) {
      if (typeof error.code === 'number') {
        process.exit(error.code);
      }
      console.error(error && error.stack ? error.stack : String(error));
      process.exit(1);
    }
  })
  .catch((error) => {
    console.error(error && error.stack ? error.stack : String(error));
    process.exit(1);
  });
