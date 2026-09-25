#!/usr/bin/env node
// Validate the playground's browser-side WAT encoder against wat2wasm.
//
// For every .wat in the given directory: if wat2wasm accepts it, encodeWat()
// must produce a byte-identical module. The default corpus (tests/wat-corpus/)
// is the wat2wasm-accepted subset of the parity corpus — the WATs wat2wasm
// itself rejects (the wasm backend's documented unsupported-feature classes:
// strings, lambdas, maps, undefined stdlib calls) are not committed; the
// playground reports those programs as unsupported at compile/run time.
//
// Usage: node scripts/validate_encoder.mjs [wat-dir] [wabt-bin-dir]
//        (defaults: tests/wat-corpus and wat2wasm from $PATH)
import { readFile, readdir, mkdir } from 'node:fs/promises';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { createRequire } from 'node:module';
import path from 'node:path';

const execFileP = promisify(execFile);
const require = createRequire(import.meta.url);

const watDir = process.argv[2]
  ?? new URL('../tests/wat-corpus', import.meta.url).pathname;
const wabtBin = process.argv[3] ?? '';
const wat2wasm = wabtBin ? path.join(wabtBin, 'wat2wasm') : 'wat2wasm';
const refDir = process.env.REF_DIR ?? '/tmp/watsref';
await mkdir(refDir, { recursive: true });
const playgroundJs = new URL('../registry/static/playground/playground.js', import.meta.url).pathname;

const { encodeWat } = require(playgroundJs);

const files = (await readdir(watDir)).filter((f) => f.endsWith('.wat'));
if (files.length === 0) {
  console.error(`no .wat files in ${watDir}`);
  process.exit(2);
}

let pass = 0;
let skipped = 0;
const failures = [];
for (const f of files) {
  const wat = await readFile(`${watDir}/${f}`, 'utf8');
  const refPath = path.join(refDir, `${f}.wasm`);
  let ref;
  try {
    await execFileP(wat2wasm, [`${watDir}/${f}`, '-o', refPath]);
    ref = new Uint8Array(await readFile(refPath));
  } catch (e) {
    // wat2wasm rejects it: an unsupported-feature class, not an encoder case.
    skipped++;
    continue;
  }
  let ours;
  try {
    ours = encodeWat(wat);
  } catch (e) {
    failures.push(`${f}: encodeWat threw: ${e.message}`);
    continue;
  }
  if (ours.length !== ref.length) {
    failures.push(`${f}: length mismatch ours=${ours.length} ref=${ref.length}`);
    continue;
  }
  let diff = -1;
  for (let i = 0; i < ref.length; i++) {
    if (ours[i] !== ref[i]) {
      diff = i;
      break;
    }
  }
  if (diff >= 0) {
    failures.push(
      `${f}: byte ${diff} differs ours=0x${ours[diff].toString(16)} ref=0x${ref[diff].toString(16)}`
    );
  } else {
    pass++;
  }
}

console.log(
  `encoder validation: ${pass} byte-identical to wat2wasm, ${skipped} skipped (unsupported-class WATs), ${failures.length} failures`
);
if (failures.length > 0) {
  console.error('FAILURES:');
  for (const f of failures) console.error('  ' + f);
  process.exit(1);
}
if (pass === 0) {
  console.error('no WAT validated — corpus generation is broken');
  process.exit(1);
}
