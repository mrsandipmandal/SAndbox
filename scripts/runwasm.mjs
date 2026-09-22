#!/usr/bin/env node
// Runner for sandbox-generated .wasm modules (wasm backend parity testing).
//
// Usage: node scripts/runwasm.mjs <module.wasm>
//
// The sandbox wasm backend imports exactly one function, console.log(i64),
// and exports `main` plus linear `memory`. This shim stubs the import,
// runs main, and prints each logged i64 as a signed decimal — matching the
// native C backend's `print` output byte for byte.
import { readFile } from 'node:fs/promises';

const file = process.argv[2];
if (!file) {
  console.error('usage: node runwasm.mjs <module.wasm>');
  process.exit(2);
}

const bytes = await readFile(file);
const logs = [];
const importObject = {
  console: { log: (v) => logs.push(BigInt.asIntN(64, v)) },
};

const mod = await WebAssembly.instantiate(bytes, importObject);
const exp = mod.instance.exports;
const entry = exp.main ?? exp.$main;
if (!entry) {
  console.error('no exported main found in', file);
  process.exit(1);
}
entry();
process.stdout.write(logs.map(String).join('\n') + (logs.length ? '\n' : ''));
