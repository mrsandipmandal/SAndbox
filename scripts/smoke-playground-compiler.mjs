#!/usr/bin/env node
// Smoke-test the playground compiler artifact (registry/static/playground/
// compiler.wasm) through its real C ABI, in-process under node:
//
//   1. sbx_version returns the crate version.
//   2. The "Block scoping" example compiles to WAT, encodes to a binary via
//      the page's own encodeWat(), instantiates, and runs (7 then 5 — the
//      inner shadowed binding first, then the outer one).
//   3. A `use` package injected through the packages blob resolves: the
//      "Registry package" example prints factorial(10) = 3628800. Package
//      source is read from registry-data/ — no running registry needed.
//   4. A type error surfaces on the error path: out = 0 and the message
//      names the undefined variable.
//
// Usage: node scripts/smoke-playground-compiler.mjs [compiler.wasm]
import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const wasmPath = process.argv[2]
  ?? path.resolve('registry/static/playground/compiler.wasm');

const { encodeWat } = require(new URL(
  '../registry/static/playground/playground.js',
  import.meta.url,
).pathname);

// ── ABI helpers (mirror playground.js) ──────────────────────────────────────

function readEnvelope(exports, ptr) {
  const mem = new Uint8Array(exports.memory.buffer);
  const len = mem[ptr] | (mem[ptr + 1] << 8) | (mem[ptr + 2] << 16) | (mem[ptr + 3] << 24);
  return new TextDecoder().decode(mem.slice(ptr + 4, ptr + 4 + len));
}

function writeGuest(exports, str) {
  const bytes = new TextEncoder().encode(str);
  const ptr = exports.sbx_alloc(bytes.length);
  new Uint8Array(exports.memory.buffer, ptr, bytes.length).set(bytes);
  return [ptr, bytes.length];
}

function runBinary(binary) {
  const logs = [];
  const mod = new WebAssembly.Module(new Uint8Array(binary).buffer);
  const inst = new WebAssembly.Instance(mod, {
    console: { log: (v) => logs.push(BigInt.asIntN(64, v).toString()) },
  });
  inst.exports.main();
  return logs;
}

function compile(exports, source, blob = null) {
  const [pPtr, pLen] = writeGuest(exports, source);
  let [bPtr, bLen] = [0, 0];
  if (blob !== null) [bPtr, bLen] = writeGuest(exports, blob);
  const errSlot = exports.sbx_alloc(4);
  new DataView(exports.memory.buffer).setUint32(errSlot, 0, true);
  const outPtr = exports.sbx_compile(pPtr, pLen, bPtr, bLen, errSlot);
  const errAddr = new DataView(exports.memory.buffer).getUint32(errSlot, true);
  return { outPtr, errAddr };
}

// ── Tests ───────────────────────────────────────────────────────────────────

let failures = 0;
function check(name, ok, detail = '') {
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${detail ? ` — ${detail}` : ''}`);
  if (!ok) failures++;
}

const bytes = new Uint8Array(await readFile(wasmPath));
check('artifact loads', bytes[0] === 0 && bytes[1] === 0x61 && bytes[2] === 0x73 && bytes[3] === 0x6d,
  `${bytes.length} bytes`);

const instance = new WebAssembly.Instance(new WebAssembly.Module(bytes.buffer), {});
const exports = instance.exports;

const mem0 = new Uint8Array(exports.memory.buffer);
const vStart = exports.sbx_version();
let vEnd = vStart;
while (mem0[vEnd] !== 0) vEnd++;
const version = new TextDecoder().decode(mem0.subarray(vStart, vEnd));
check('sbx_version', /^\d+\.\d+\.\d+$/.test(version), version);

// 2. Block scoping (the freshly-landed semantics, through the wasm pipeline).
const blockScoping = `fn main() {
    let y = 5
    if true {
        let y = 7
        print(y)
    }
    print(y)
}`;
const r1 = compile(exports, blockScoping);
check('block scoping compiles', r1.outPtr !== 0, r1.outPtr === 0 ? readEnvelope(exports, r1.errAddr) : '');
if (r1.outPtr !== 0) {
  const wat = readEnvelope(exports, r1.outPtr);
  const binary = encodeWat(wat);
  const logs = runBinary(binary);
  check('block scoping runs 7,5', JSON.stringify(logs) === JSON.stringify(['7', '5']),
    JSON.stringify(logs));
}

// 3. Package injection via the packages blob.
const pkgDir = 'registry-data/packages/sandbox_math_ext';
const versions = (await readdir(pkgDir)).filter((f) => /^\d+\.\d+\.\d+\.sb$/.test(f))
  .sort((a, b) => a.localeCompare(b, undefined, { numeric: true }));
check('math_ext package present in registry-data', versions.length > 0);
if (versions.length > 0) {
  const pkgSource = await readFile(path.join(pkgDir, versions.at(-1)), 'utf8');
  const example = 'use sandbox_math_ext::factorial\n\nfn main() {\n    print(factorial(10))\n}';
  const blob = `sandbox_math_ext=${pkgSource}\0`;
  const r2 = compile(exports, example, blob);
  check('package use compiles', r2.outPtr !== 0, r2.outPtr === 0 ? readEnvelope(exports, r2.errAddr) : '');
  if (r2.outPtr !== 0) {
    const logs = runBinary(encodeWat(readEnvelope(exports, r2.outPtr)));
    check('factorial(10) = 3628800', JSON.stringify(logs) === JSON.stringify(['3628800']),
      JSON.stringify(logs));
  }
}

// 4. Error path.
const r3 = compile(exports, 'fn main() {\n    print(undefined_var)\n}');
const errOk = r3.outPtr === 0 && readEnvelope(exports, r3.errAddr).includes("Undefined variable 'undefined_var'");
check('error path reports type error', errOk,
  r3.outPtr === 0 ? readEnvelope(exports, r3.errAddr) : 'unexpectedly succeeded');

if (failures > 0) {
  console.error(`\n${failures} smoke check(s) failed`);
  process.exit(1);
}
console.log('\nall playground-compiler smoke checks passed');
