// Sandbox playground — compiles .sbx to WebAssembly entirely in the browser.
//
// Three pieces live here:
//
//  1. encodeWat(): a WAT → wasm-binary encoder covering exactly the subset
//     the wasmgen backend emits (all-i64 programs, labeled block/loop/br,
//     folded const/instruction forms, a string data segment). The compiler
//     runs in-page (compiler.wasm) and emits .wat text; encoding to binary
//     here keeps the artifact pipeline dependency-free — no wasm-bindgen
//     glue, no wat2wasm server-side.
//  2. The compiler loader + package fetcher: compiler.wasm exposes a C ABI
//     (sbx_alloc/sbx_free/sbx_compile); packages referenced by `use` are
//     fetched from this registry's own API and injected into the compile.
//  3. The runner: instantiates the encoded module with a console.log stub
//     (identical contract to scripts/runwasm.mjs) and prints each logged
//     i64 as signed decimal.

// ── 1. WAT → wasm binary encoder ────────────────────────────────────────────
//
// wasmgen emits a closed grammar (verified across the whole parity corpus):
//
//   module  := "(module" import memory data* func* export ")"   (any order)
//   import  := "(import \"console\" \"log\" (func $log (param i64)))"
//   memory  := "(memory (export \"memory\") N)"
//   data    := "(data (i32.const 0)" ("...")* ")"
//   func    := "(func $name" (param $p i64)* ("(result i64)")?
//              (local $l i64)* instr* ")" | "(unreachable)"
//   export  := "(export \"main\" (func $main))"
//   instr   := folded: "(op ...)" with leaf forms "(i64.const N)",
//              "(local.get $x)", "(call $f a)", "(br $l)", "(if ...)",
//              "(block $l ...)", "(loop $l ...)"
//   i64 ops : add sub mul div_s rem_s and or xor shl shr_s shr_u
//             lt_s le_s gt_s ge_s eq ne eqz  (plus i32.const/i32.eqz,
//             i64.extend_i32_u, f64.const, i64.trunc_f64_s,
//             i64.reinterpret_f64)

const OPCODES = {
  'i32.eqz': [0x45],
  'i32.const': [0x41],
  'i64.eqz': [0x50],
  'i64.add': [0x7c],
  'i64.sub': [0x7d],
  'i64.mul': [0x7e],
  'i64.div_s': [0x7f],
  'i64.rem_s': [0x81],
  'i64.and': [0x83],
  'i64.or': [0x84],
  'i64.xor': [0x85],
  'i64.shl': [0x86],
  'i64.shr_s': [0x87],
  'i64.shr_u': [0x88],
  'i64.lt_s': [0x53],
  'i64.gt_s': [0x55],
  'i64.le_s': [0x57],
  'i64.ge_s': [0x59],
  'i64.eq': [0x51],
  'i64.ne': [0x52],
  'i64.const': [0x42],
  'i64.extend_i32_u': [0xad],
  'i64.trunc_f64_s': [0xb0],
  'i64.reinterpret_f64': [0xbf],
  'f64.const': [0x44],
  'local.get': [0x20],
  'local.set': [0x21],
  'call': [0x10],
  'block': [0x02],
  'loop': [0x03],
  'if': [0x04],
  'else': [0x05],
  'end': [0x0b],
  'br': [0x0c],
  'br_if': [0x0d],
  'return': [0x0f],
  'unreachable': [0x00],
};

const VALTYPE_I64 = 0x7e;

class WatError extends Error {
  constructor(message, line) {
    super(`WAT encode error (line ${line}): ${message}`);
    this.line = line;
  }
}

function encodeWat(watText) {
  const out = new ByteWriter();
  const ctx = {
    funcs: new Map(), // name -> index (imports first)
    labels: [],       // active label stack, innermost last
    line: 0,
  };

  // Tokenize top-level structure: an S-expression scan where the head atom
  // decides the node kind. Comments (";;" lines) are dropped by the lexer.
  const ast = parseWatModule(watText, ctx);

  // ── Binary scaffolding ──────────────────────────────────────────────
  // Type section: wasmgen emits only () -> i64 (result fns) and () -> ().
  // Calls to $log are (i64) -> (), folded through a type too if referenced.
  const typeKeys = new Map(); // "sig" -> type index
  const typeList = [];
  function typeIndex(params, results) {
    const key = params.join(',') + '->' + results.join(',');
    if (!typeKeys.has(key)) {
      typeKeys.set(key, typeList.length);
      typeList.push({ params, results });
    }
    return typeKeys.get(key);
  }

  const funcMetas = []; // { typeIndex, locals: [i64 extra], body: [] }
  const exportsSec = []; // { name, funcIndex }
  const dataSegments = [];
  let memoryMin = 1;

  // Pass 1: register function names (imports first, then defined fns in
  // source order) so `call $f` resolves to the right index.
  let nextFuncIndex = 0;
  let hasLogImport = false;
  let logTypeIndex = -1;
  for (const node of ast.children) {
    if (node.head === 'import') {
      // Only the console.log import is ever emitted; register its
      // (i64)->() type NOW — every type must exist before the type
      // section is serialized, and assembly happens after pass 2.
      ctx.funcs.set('$log', nextFuncIndex++);
      hasLogImport = true;
      logTypeIndex = typeIndex(['i64'], []);
    } else if (node.head === 'func') {
      ctx.funcs.set(node.name, nextFuncIndex++);
    }
  }

  // Pass 2: encode each defined function.
  for (const node of ast.children) {
    switch (node.head) {
      case 'memory': {
        memoryMin = node.memoryPages;
        break;
      }
      case 'data': {
        // Strings concatenate into one segment at offset 0 (wasmgen lays
        // them out back-to-back starting at (i32.const 0)).
        const bytes = [];
        for (const s of node.strings) bytes.push(...s);
        dataSegments.push({ offset: 0, bytes });
        break;
      }
      case 'func': {
        const params = [];
        for (const p of node.params) params.push(VALTYPE_I64);
        const results = node.hasResult ? ['i64'] : [];
        const ti = typeIndex(params, results);
        // Locals beyond params are all i64.
        const localGroups = [];
        const extra = node.locals.filter((l) => !node.params.includes(l.name));
        if (extra.length > 0) localGroups.push({ count: extra.length, type: VALTYPE_I64 });
        const body = new ByteWriter();
        const fellThrough = encodeFuncBody(node, body, ctx);
        // Mirror wasmgen's "pin the end with (unreachable)" rule: a result
        // function whose instruction stream can fall off the end gets the
        // pin — but only when wasmgen did NOT emit one itself (that shows
        // up as an explicit (unreachable) we already encoded).
        if (node.hasResult && fellThrough) body.pushOp('unreachable');
        body.pushOp('end');
        funcMetas.push({ typeIndex: ti, localGroups, body });
        break;
      }
      case 'export': {
        const idx = ctx.funcs.get(node.funcRef);
        if (idx === undefined) throw new WatError(`export references unknown ${node.funcRef}`, node.line);
        exportsSec.push({ name: node.exportName, funcIndex: idx });
        break;
      }
      case 'import':
        break; // handled in pass 1
      default:
        throw new WatError(`unsupported top-level (${node.head})`, node.line);
    }
  }

  // ── Assemble the module ─────────────────────────────────────────────
  out.pushBytes([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]); // magic + version

  const sections = [];
  // Type section (1)
  {
    const s = new ByteWriter();
    s.pushUleb(typeList.length);
    for (const t of typeList) {
      s.pushBytes([0x60]);
      s.pushUleb(t.params.length);
      for (const p of t.params) s.pushBytes([p === 'i64' ? VALTYPE_I64 : 0x7e]);
      s.pushUleb(t.results.length);
      for (const r of t.results) s.pushBytes([r === 'i64' ? VALTYPE_I64 : 0x7e]);
    }
    sections.push([1, s]);
  }
  // Import section (2) — console.log with its (i64)->() type
  if (hasLogImport) {
    const s = new ByteWriter();
    s.pushUleb(1);
    s.pushName('console');
    s.pushName('log');
    s.pushBytes([0x00]); // func import
    s.pushUleb(logTypeIndex);
    sections.push([2, s]);
  }
  // Function section (3)
  {
    const s = new ByteWriter();
    s.pushUleb(funcMetas.length);
    for (const f of funcMetas) s.pushUleb(f.typeIndex);
    sections.push([3, s]);
  }
  // Memory section (5)
  {
    const s = new ByteWriter();
    s.pushUleb(1);
    s.pushBytes([0x00]); // limits: min only
    s.pushUleb(memoryMin);
    sections.push([5, s]);
  }
  // Export section (7). wasmgen writes the memory export INLINE on the
  // memory node, which encodes as an ordinary export entry; wat2wasm
  // orders exports by their position in the module, and the memory node
  // precedes the (export) nodes — so "memory" goes first.
  {
    const s = new ByteWriter();
    s.pushUleb(exportsSec.length + 1);
    s.pushName('memory');
    s.pushBytes([0x02]); // memory export kind
    s.pushUleb(0);
    for (const e of exportsSec) {
      s.pushName(e.name);
      s.pushBytes([0x00]);
      s.pushUleb(e.funcIndex);
    }
    sections.push([7, s]);
  }
  // Code section (10)
  {
    const s = new ByteWriter();
    s.pushUleb(funcMetas.length);
    for (const f of funcMetas) {
      const fb = new ByteWriter();
      fb.pushUleb(f.localGroups.length);
      for (const g of f.localGroups) {
        fb.pushUleb(g.count);
        fb.pushBytes([g.type]);
      }
      fb.pushBytes(f.body.bytes);
      s.pushUleb(fb.bytes.length);
      s.pushBytes(fb.bytes);
    }
    sections.push([10, s]);
  }
  // Data section (11)
  if (dataSegments.length > 0) {
    const s = new ByteWriter();
    const segs = dataSegments; // all at offset 0; emit as separate segments
    s.pushUleb(segs.length);
    for (const seg of segs) {
      s.pushBytes([0x00]); // active, memory 0
      s.pushBytes([0x41, 0x00, 0x0b]); // i32.const 0 end
      s.pushUleb(seg.bytes.length);
      s.pushBytes(seg.bytes);
    }
    sections.push([11, s]);
  }

  let prev = 0;
  for (const [id, s] of sections) {
    void prev;
    out.pushBytes([id]);
    out.pushUleb(s.bytes.length);
    out.pushBytes(s.bytes);
  }
  return out.bytes;
}

// ── Byte writer helpers ──────────────────────────────────────────────────────

class ByteWriter {
  constructor() {
    this.bytes = [];
  }
  pushBytes(bs) {
    for (const b of bs) this.bytes.push(b & 0xff);
  }
  pushUleb(n) {
    n = BigInt(n);
    do {
      let byte = Number(n & 0x7fn);
      n >>= 7n;
      if (n !== 0n) byte |= 0x80;
      this.bytes.push(byte);
    } while (n !== 0n);
  }
  pushSleb(n) {
    let v = BigInt(n);
    for (;;) {
      const byte = Number(v & 0x7fn);
      v >>= 7n;
      const signBit = (byte & 0x40) !== 0;
      if ((v === 0n && !signBit) || (v === -1n && signBit)) {
        this.bytes.push(byte);
        break;
      }
      this.bytes.push(byte | 0x80);
    }
  }
  pushName(str) {
    const utf8 = Array.from(new TextEncoder().encode(str));
    this.pushUleb(utf8.length);
    this.pushBytes(utf8);
  }
  pushOp(name) {
    const op = OPCODES[name];
    if (!op) throw new WatError(`unsupported opcode ${name}`, 0);
    this.pushBytes(op);
  }
}

// ── WAT lexer/parser ─────────────────────────────────────────────────────────

function parseWatModule(watText, ctx) {
  const lines = watText.split('\n');
  // Flatten into tokens with line info; strings keep as single tokens.
  const tokens = [];
  let i = 0;
  while (i < lines.length) {
    let line = lines[i];
    const lineNo = i + 1;
    const semi = line.indexOf(';;');
    if (semi >= 0) line = line.slice(0, semi);
    let j = 0;
    while (j < line.length) {
      const c = line[j];
      if (c === ' ' || c === '\t' || c === '\r') {
        j++;
        continue;
      }
      if (c === '(' || c === ')') {
        tokens.push({ t: c, line: lineNo });
        j++;
        continue;
      }
      if (c === '"') {
        let k = j + 1;
        const chars = [];
        while (k < line.length && line[k] !== '"') {
          if (line[k] === '\\') {
            const hex = line.slice(k + 1, k + 3);
            chars.push(parseInt(hex, 16));
            k += 3;
          } else {
            chars.push(line.charCodeAt(k));
            k++;
          }
        }
        tokens.push({ t: 'str', value: chars, line: lineNo });
        j = k + 1;
        continue;
      }
      let k = j;
      while (k < line.length && !' \t\r()"'.includes(line[k])) k++;
      tokens.push({ t: 'atom', value: line.slice(j, k), line: lineNo });
      j = k;
    }
    i++;
  }

  let pos = 0;
  function peek() {
    return tokens[pos];
  }
  function next() {
    return tokens[pos++];
  }

  // Parse a ( ... ) node: head is first atom; children recursively.
  function parseNode() {
    const open = next();
    if (open.t !== '(') throw new WatError('expected (', open.line);
    ctx.line = open.line;
    const headTok = peek();
    let head = null;
    if (headTok && headTok.t === 'atom') {
      head = next().value;
    }
    const node = { t: '(', head, children: [], line: open.line };
    for (;;) {
      const tok = peek();
      if (!tok) throw new WatError('unexpected EOF', open.line);
      if (tok.t === ')') {
        next();
        return node;
      }
      if (tok.t === '(') {
        node.children.push(parseNode());
        continue;
      }
      node.children.push(next());
    }
  }

  const mod = parseNode();
  if (mod.head !== 'module') throw new WatError('expected (module ...)', 1);

  for (const node of mod.children) {
    switch (node.head) {
      case 'memory': {
        // (memory (export "memory") N)
        const pages = node.children.filter((c) => c.t === 'atom' && /^\d+$/.test(c.value));
        node.memoryPages = pages.length ? parseInt(pages[pages.length - 1].value, 10) : 1;
        break;
      }
      case 'data': {
        // (data (i32.const 0) ("...") ("...") )
        node.strings = node.children.filter((c) => c.t === 'str').map((c) => c.value);
        break;
      }
      case 'func': {
        const nameTok = node.children.find((c) => c.t === 'atom' && c.value.startsWith('$'));
        node.name = nameTok ? nameTok.value : '$anonymous';
        node.params = [];
        node.locals = [];
        node.hasResult = false;
        node.hasExplicitUnreachable = false;
        node.instrs = []; // instruction nodes/tokens in body order
        for (const c of node.children) {
          if (c.t !== '(') continue;
          if (c.head === 'param') {
            for (const p of c.children.filter((x) => x.t === 'atom' && x.value.startsWith('$'))) {
              node.params.push(p.value);
            }
          } else if (c.head === 'local') {
            for (const l of c.children.filter((x) => x.t === 'atom' && x.value.startsWith('$'))) {
              node.locals.push({ name: l.value });
            }
          } else if (c.head === 'result') {
            node.hasResult = true;
          } else {
            node.instrs.push(c);
          }
        }
        // (falls-through detection happens during encoding; nothing to
        // annotate here)
        const last = node.instrs[node.instrs.length - 1];
        node.hasExplicitUnreachable = !!last && last.head === 'unreachable';
        break;
      }
      case 'export': {
        // (export "name" (func $f))
        const nameTok = node.children.find((c) => c.t === 'str');
        const inner = node.children.find((c) => c.t === '(' && c.head === 'func');
        node.exportName = nameTok ? String.fromCharCode(...nameTok.value) : '';
        node.funcRef = inner ? inner.children.find((c) => c.t === 'atom').value : null;
        break;
      }
      default:
        break;
    }
  }
  return mod;
}

// Encode one function's body instructions (already parsed nodes).
function encodeFuncBody(funcNode, out, ctx) {
  // Map local names to indices for THIS function only.
  const localIndex = new Map();
  funcNode.params.forEach((p, i) => localIndex.set(p, i));
  funcNode.locals.forEach((l, i) => localIndex.set(l.name, funcNode.params.length + i));

  // wabt dead-code-eliminates everything after an unconditional terminator
  // (return/unreachable/br) inside a block and drops a terminator that sits
  // directly before `end`, then heals the block: `(return) (unreachable)
  // (end)` serializes as bare `unreachable`. Match that so our binaries are
  // byte-identical to wat2wasm's for the same text. `br` keeps DCE but is
  // never last (wasmgen always closes loops with `(br $continue)`).
  function isUnconditionalTerminator(node) {
    return node && node.t === '(' &&
      (node.head === 'return' || node.head === 'unreachable' || node.head === 'br');
  }
  function emitSeq(nodes) {
    for (let i = 0; i < nodes.length; i++) {
      const node = nodes[i];
      if (node.t !== '(') continue;
      if (isUnconditionalTerminator(node) && node.head === 'br') {
        // br: DCE the rest, but keep the br itself.
        encodeInstr(node);
        return;
      }
      encodeInstr(node);
    }
  }

  function encodeInstr(node) {
    const line = node.line || 0;
    if (node.t === '(') {
      switch (node.head) {
        case 'local.set': {
          const name = node.children.find((c) => c.t === 'atom').value;
          const idx = localIndex.get(name);
          if (idx === undefined) throw new WatError(`unknown local ${name}`, line);
          // Folded form: the value expression executes BEFORE local.set.
          for (const c of node.children) {
            if (c.t === '(') encodeInstr(c);
          }
          out.pushOp('local.set');
          out.pushUleb(idx);
          return;
        }
        case 'local.get': {
          const name = node.children.find((c) => c.t === 'atom').value;
          const idx = localIndex.get(name);
          if (idx === undefined) throw new WatError(`unknown local ${name}`, line);
          out.pushOp('local.get');
          out.pushUleb(idx);
          return;
        }
        case 'call': {
          const name = node.children.find((c) => c.t === 'atom' && c.value.startsWith('$')).value;
          const idx = ctx.funcs.get(name);
          if (idx === undefined) throw new WatError(`call to unknown ${name}`, line);
          // Folded form: arguments execute before the call instruction.
          for (const c of node.children) {
            if (c.t === '(') encodeInstr(c);
          }
          out.pushOp('call');
          out.pushUleb(idx);
          return;
        }
        case 'i64.const': {
          const raw = node.children.find((c) => c.t === 'atom').value;
          out.pushOp('i64.const');
          out.pushSleb(parseWatInt(raw));
          return;
        }
        case 'i32.const': {
          const raw = node.children.find((c) => c.t === 'atom').value;
          out.pushOp('i32.const');
          out.pushSleb(parseWatInt(raw));
          return;
        }
        case 'f64.const': {
          const raw = node.children.find((c) => c.t === 'atom').value;
          out.pushOp('f64.const');
          out.pushF64(parseFloat(raw));
          return;
        }
        case 'block':
        case 'loop': {
          const label = node.children.find((c) => c.t === 'atom' && c.value.startsWith('$'));
          if (label) ctx.labels.push(label.value);
          out.pushOp(node.head);
          out.pushBytes([0x40]); // void blocktype
          emitSeq(node.children.filter((c) => c.t === '('));
          out.pushOp('end');
          if (label) ctx.labels.pop();
          return;
        }
        case 'if': {
          // wasmgen folds if as: (if cond-expr (then ...) (else ...)) —
          // the condition evaluates BEFORE the if instruction.
          const then = node.children.find((c) => c.t === '(' && c.head === 'then');
          const els = node.children.find((c) => c.t === '(' && c.head === 'else');
          for (const c of node.children) {
            if (c.t === '(' && c !== then && c !== els) encodeInstr(c); // condition
          }
          out.pushOp('if');
          out.pushBytes([0x40]);
          // An `if` is itself a branch target (depth 0 from inside its
          // arms), but the text form gives it no name — push an anonymous
          // frame so named lookups ($continue etc.) resolve one level up,
          // exactly as wat2wasm resolves them.
          ctx.labels.push(null);
          if (!then) throw new WatError('if without then', line);
          emitSeq(then.children.filter((c) => c.t === '('));
          if (els) {
            out.pushOp('else');
            emitSeq(els.children.filter((c) => c.t === '('));
          }
          ctx.labels.pop();
          out.pushOp('end');
          return;
        }
        case 'br':
        case 'br_if': {
          const label = node.children.find((c) => c.t === 'atom').value;
          const depth = ctx.labels.lastIndexOf(label);
          if (depth < 0) throw new WatError(`br to unknown label ${label}`, line);
          // Folded br_if carries its condition as a child: it evaluates
          // before the branch instruction (plain br has no children).
          for (const c of node.children) {
            if (c.t === '(') encodeInstr(c);
          }
          out.pushOp(node.head);
          out.pushUleb(ctx.labels.length - 1 - depth);
          return;
        }
        case 'return': {
          // Folded return value evaluates before the return instruction.
          for (const c of node.children) {
            if (c.t === '(') encodeInstr(c);
          }
          out.pushOp('return');
          return;
        }
        case 'unreachable': {
          out.pushOp('unreachable');
          return;
        }
        default: {
          // Plain numeric instruction, possibly with folded operands.
          if (!OPCODES[node.head]) {
            throw new WatError(`unsupported instruction ${node.head}`, line);
          }
          for (const c of node.children) {
            if (c.t === '(') encodeInstr(c);
          }
          out.pushOp(node.head);
          return;
        }
      }
    }
    throw new WatError('expected instruction node', line);
  }

  let fellThrough = true;
  for (const instr of funcNode.instrs) {
    encodeInstr(instr);
    if (isUnconditionalTerminator(instr)) {
      // Everything after the terminator is dead; wabt also drops a
      // trailing terminator before `end`, so stop here unconditionally.
      fellThrough = false;
      break;
    }
  }
  return fellThrough;
}

function parseWatInt(raw) {
  // wasmgen prints i64 consts as signed decimal; i32 consts may be hex-ish
  // for data offsets (plain 0 here). Handle underscores per WAT spec.
  const clean = raw.replace(/_/g, '');
  return BigInt(clean);
}

// f64 bits writer on ByteWriter
ByteWriter.prototype.pushF64 = function (v) {
  const buf = new ArrayBuffer(8);
  new DataView(buf).setFloat64(0, v, true);
  for (const b of new Uint8Array(buf)) this.bytes.push(b);
};

// ── 2. Compiler module loader + registry packages ───────────────────────────

let compilerPromise = null;

function loadCompiler() {
  if (compilerPromise) return compilerPromise;
  compilerPromise = (async () => {
    const resp = await fetch('/playground/compiler.wasm');
    if (!resp.ok) throw new Error(`compiler.wasm fetch failed: HTTP ${resp.status}`);
    const bytes = await resp.arrayBuffer();
    const { instance } = await WebAssembly.instantiate(bytes, {});
    return instance.exports;
  })();
  return compilerPromise;
}

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

// Collect `use` package names from source (first path segment, unique).
function collectPackageNames(source) {
  const names = new Set();
  for (const m of source.matchAll(/(^|\n)\s*use\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*::/g)) {
    names.add(m[2]);
  }
  return [...names];
}

// Fetch package sources from the registry API. Returns null if any package
// is unavailable (the caller reports a helpful message).
async function fetchPackages(names, setStatus) {
  const entries = [];
  for (const name of names) {
    setStatus(`Fetching package ${name}…`);
    let info;
    try {
      const res = await fetch(`/api/v1/packages/${encodeURIComponent(name)}`);
      if (!res.ok) return { error: `Package \`${name}\` not found on this registry (HTTP ${res.status}).` };
      info = await res.json();
    } catch (e) {
      return { error: `Failed to reach the registry for \`${name}\`: ${e.message}` };
    }
    const version = info.latest_version;
    if (!version) return { error: `Package \`${name}\` has no published versions.` };
    setStatus(`Downloading ${name}@${version}…`);
    let res;
    try {
      res = await fetch(`/api/v1/packages/${encodeURIComponent(name)}/${encodeURIComponent(version)}/download`);
    } catch (e) {
      return { error: `Download failed for \`${name}\`: ${e.message}` };
    }
    if (!res.ok) return { error: `Download failed for \`${name}\`: HTTP ${res.status}` };
    const source = await res.text();
    entries.push(`${name}=${source}`);
  }
  return { blob: entries.join('\0') };
}

async function compileSource(source, setStatus) {
  const exports = await loadCompiler();
  setStatus('Loading compiler…');

  const pkgNames = collectPackageNames(source);
  let blob = null;
  if (pkgNames.length > 0) {
    const fetched = await fetchPackages(pkgNames, setStatus);
    if (fetched.error) return { error: fetched.error };
    blob = fetched.blob;
  }

  setStatus('Compiling…');
  const [pPtr, pLen] = writeGuest(exports, source);
  let bPtr = 0;
  let bLen = 0;
  if (blob !== null) {
    [bPtr, bLen] = writeGuest(exports, blob);
  }
  const errSlot = exports.sbx_alloc(4);
  new DataView(exports.memory.buffer).setUint32(errSlot, 0, true);

  const outPtr = exports.sbx_compile(pPtr, pLen, bPtr, bLen, errSlot);
  if (outPtr === 0) {
    const errAddr = new DataView(exports.memory.buffer).getUint32(errSlot, true);
    return { error: readEnvelope(exports, errAddr) };
  }
  const wat = readEnvelope(exports, outPtr);
  setStatus('Encoding to wasm binary…');
  let binary;
  try {
    binary = encodeWat(wat);
  } catch (e) {
    return { error: `Internal: ${e.message}`, wat };
  }
  return { wat, binary };
}

// ── 3. Runner: console.log(i64) contract, like scripts/runwasm.mjs ──────────

async function runWasm(binary, lineSink) {
  const logs = [];
  const importObject = {
    console: {
      log: (v) => {
        const signed = BigInt.asIntN(64, v);
        logs.push(signed.toString());
        lineSink(signed.toString());
      },
    },
  };
  const { instance } = await WebAssembly.instantiate(new Uint8Array(binary).buffer, importObject);
  const entry = instance.exports.main;
  if (!entry) throw new Error('module exports no main()');
  const t0 = performance.now();
  entry();
  return { logs, ms: performance.now() - t0 };
}// ── UI wiring ────────────────────────────────────────────────────────────────

const EXAMPLES = {
  'Hello, Sandbox': `fn main() {\n    print(42)\n    print(6 * 7)\n}`,
  'Block scoping': `fn main() {\n    let y = 5\n    if true {\n        let y = 7\n        print(y)\n    }\n    print(y)\n}`,
  'Loop + function': `fn squared(n: i64) -> i64 {\n    return n * n\n}\n\nfn main() {\n    let total = 0\n    for i in 0..5 {\n        total = total + squared(i)\n    }\n    print(total)\n    print(squared(12))\n}`,
  'Registry package': `use sandbox_math_ext::factorial\n\nfn main() {\n    print(factorial(10))\n}`,
};

const $ = (id) => document.getElementById(id);

async function onRun() {
  const source = $('editor').value;
  $('run').disabled = true;
  $('status').textContent = 'Booting…';
  $('output').textContent = '';
  $('error').textContent = '';
  $('wat').textContent = '';
  try {
    const t0 = performance.now();
    const result = await compileSource(source, (s) => ($('status').textContent = s));
    if (result.error) {
      $('error').textContent = result.error;
      $('status').textContent = 'Failed';
      return;
    }
    $('wat').textContent = result.wat;
    $('status').textContent = 'Running…';
    await runWasm(result.binary, (line) => {
      $('output').textContent += line + '\n';
    });
    const ms = (performance.now() - t0).toFixed(0);
    $('status').textContent = `Done in ${ms}ms`;
  } catch (e) {
    $('error').textContent = `Runtime error: ${e.message}`;
    $('status').textContent = 'Failed';
  } finally {
    $('run').disabled = false;
  }
}

function init() {
  const params = new URLSearchParams(location.search);
  const example = params.get('example');
  $('editor').value = EXAMPLES[example] ?? EXAMPLES['Hello, Sandbox'];
  for (const name of Object.keys(EXAMPLES)) {
    const b = document.createElement('button');
    b.className = 'example-btn';
    b.textContent = name;
    b.addEventListener('click', () => {
      $('editor').value = EXAMPLES[name];
      $('output').textContent = '';
      $('error').textContent = '';
      $('wat').textContent = '';
      $('status').textContent = 'Ready';
      history.replaceState(null, '', `?example=${encodeURIComponent(name)}`);
    });
    $('examples').appendChild(b);
  }
  $('run').addEventListener('click', onRun);
  // Tab inserts two spaces instead of leaving the textarea.
  $('editor').addEventListener('keydown', (e) => {
    if (e.key === 'Tab') {
      e.preventDefault();
      const el = e.target;
      const start = el.selectionStart;
      el.setRangeText('  ', start, el.selectionEnd, 'end');
    }
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
      e.preventDefault();
      onRun();
    }
  });
  $('status').textContent = 'Ready';
}

if (typeof document !== 'undefined') {
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
}

// Node validation hook: scripts/validate_encoder.mjs evaluates this file in
// a sandbox with a document stub and drives encodeWat() over the parity
// corpus, comparing each module byte-for-byte against wat2wasm.
if (typeof module !== 'undefined' && module.exports) {
  module.exports = { encodeWat, OPCODES, parseWatModule };
}
