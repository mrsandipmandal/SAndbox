# PROJECT_MEMORY.md

Single source of truth for the Sandbox language project. Read this file first
every session; keep it updated as work completes.

## Project snapshot

**Sandbox** is a programming language written in Rust (repo root `Cargo.toml`)
with three execution backends that must stay behaviorally identical:

| Backend | Entry | Notes |
|---|---|---|
| C | `sandbox run file.sbx` (default) | Emits C, compiles with gcc. Runtime: `stdlib::c_preamble()`. |
| LLVM | `sandbox llvm-build f.sbx -o out` | Emits `.ll`, compiles with clang. `@sandbox_main` + `i32 @main` shim. |
| Interpreter | `sandbox interpret file.sbx` | In-tree, instant feedback. |

Other crates: `registry/` (package registry server, own gates).

### Gates (run before any commit)
- `bash scripts/ci-local.sh` — every CI gate locally, in CI order
- `bash scripts/ci-smoke.sh hello|array|interp|llvm` — smoke checks (single source of truth, used by CI too)
- `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings`
- Parity: `cargo test --test parity` — corpus in `tests/parity.rs`; every new language
  feature must add a case requiring ALL backends to agree

### CI/CD (all green on GitHub)
- `ci.yml`: check, smoke, parity, registry, Docker Build (pushes image to
  `ghcr.io/mrsandipmandal/sandbox-registry`, sha + latest tags, smoke-checked),
  Deploy Registry (SSH + compose, health gate, auto-rollback; skips loudly until
  `DEPLOY_SSH_HOST`/`DEPLOY_SSH_PRIVATE_KEY` secrets are set on the `production`
  environment; **you** are the required reviewer there).
- `release.yml`: cross-platform builds + `SHA256SUMS.txt` attached to releases.

## Working protocol (every session)

1. Read this file; pick the next unchecked roadmap item (one small step at a time).
2. Plan the step; get explicit user approval before implementing.
3. Implement; every language feature ships with a parity corpus case (ALL backends).
4. `bash scripts/ci-local.sh` fully green → conventional commits → push → CI green
   on GitHub → tick the box here with a one-line note.

## Roadmap

Direction approved by user: **web development first, then machine-level
(compiler-level only: bitwise ops, sized/unsigned ints, casts — no FFI,
no raw pointers).** Small steps, one approved item at a time. Language for
docs/explanations: **English**.

### Track A — Web development

- [x] **A1. Maps/dicts** — `map<string, int>`: `{"a": 1}` literals, `m["key"]`,
  `len(m)`, methods `insert/get/has/remove/keys`, ordered iteration. C runtime
  `sbx_map_*` shared by C + LLVM backends; interpreter uses ordered Rust map.
  Parity case `map_basics` (ALL backends). `values()` shipped in C1.
- [x] **A2. JSON** — real JSON on maps + arrays: `json::parse_map(s)` (recursive-descent,
  C runtime shared by C+LLVM; Rust mirror in the interpreter), `json::stringify_map(m)`,
  `json::get_str(m, k, def)`, `json::get_int`, `json::stringify_array`, `json::array_get_int`;
  legacy scalar `json::*` builtins now work on the interpreter too. Parity cases
  `json_parse_basics`, `json_string_fields`, `json_array_roundtrip` (ALL backends).
  Fixed en route: LLVM builtin return-type mapping (i8*/double overrides + bool zext —
  string builtins used to emit invalid IR); legacy `map::new/contains` module-call rows
  repointed at the A1 `sbx_map*` runtime. *(Interpreter floats stay i64 → float-only
  demo lines differ from compiled backends; documented deferred gap.)*
- [x] **A3. Real HTTP server** — implicit per-request accessors `http::method()` /
  `query()` / `body()` / `req_header(name)` (handler signature unchanged: path in,
  body string out), response helpers `http::set_status(code)` + `http::set_header(name,
  value)` (content-type special-cased), pure helpers `http::query_param(query, name)` /
  `form_get(body, name)` / `form_param(name)` / `url_decode(s)`, static files
  `http::serve_static(dir)` (auto-served before the handler, mime-by-extension,
  any `..` in the path → 403 before the handler even with no static dir). C runtime
  `__sbx_http_*` shared by C + LLVM (first-ever LLVM `http::serve` support incl.
  handler fn-pointer bitcast); real TcpListener mirror in the interpreter. Legacy
  `http::get/post/.../status_code/headers` now work on the interpreter too. Fixed en
  route: LLVM stored string-concat results in i64 allocas (BinaryOp type inference);
  LLVM void-builtin call path; `query` as keyword after `::`; interpreter `let x = y`
  (string var) dropped the value; interpreter string-returning fns never surfaced
  values (last_returned_str). Parity cases `http_url_decode`, `http_query_params`,
  `http_form_parse` (ALL backends); integration test `test_http_a3_features_end_to_end`
  (query/headers/status/form/custom-header/traversal-403) on the extended
  `examples/http_server_demo.sbx`. *(Request-body size capped at one 8 KiB recv;
  no chunked encoding; concurrency = one thread per accept, sequential handling.)*
- [x] **A4. Web polish** — `html::escape`, cookies, `%{key}` templates, example app
  + integration test that boots the server and hits endpoints.
  *(Done 2026-09-18.)* `html::escape`/`html::unescape` (& < > " ' entities +
  numeric &#NN;/&#xHH; decode, unknown entities pass through); cookies via
  `http::set_cookie(name, value)` (Set-Cookie; Path=/; CR/LF/; rejected),
  `http::get_cookie(name)` (reads request Cookie header) and pure
  `http::cookie_get(header, name)`; `tmpl::render(tpl, k1, v1, ...)` replaces
  `%{key}` (unknown keys stay; variadic — typechecker enforces odd arg count
  ≥3 all-strings, C ABI is (count, ...) variadic, LLVM declare (i64, ...)).
  Rust mirrors in the interpreter incl. `resolve_arg_str` (arg resolution by
  AST shape first so stale `__auto_` strings can't poison later calls).
  `examples/web_app.sbx` (templates + cookies + escaping + text/html + 404)
  and `test_web_app_a4_end_to_end` (status/content-type/Set-Cookie/round-trip/
  404). En-route fix: C runtime now ignores SIGPIPE in serve/serve_once — a
  client disconnecting mid-response used to kill the whole server. Parity
  cases `a4_html_escape`, `a4_tmpl_render`, `a4_cookie_get` (ALL backends).

### Track B — Machine-level (compiler-level only)

- [x] **B1. Bitwise operators** — `& | ^ << >> ~` (lexer must distinguish `&` from `&&`).
  *(Done 2026-09-18.)* New tokens Amp/Caret/Shl/Shr/Tilde (`&&` still And);
  single `|` stays the lambda Pipe token — the parser treats an INFIX Pipe
  as BitOr (a lambda can only start a primary, so no ambiguity). Precedence
  is C's: shift > comparison > & > ^ > | > && > || (parse chain or → and →
  bitor → bitxor → bitand → comparison → shift → additive). Semantics:
  i64-only (typechecker rejects bool/mixed operands — stricter than C);
  `>>` is arithmetic (LLVM ashr); `<<` is wrapping — the C backend emits
  `(long)((unsigned long long)l << r)` because signed-shift-into-sign-bit
  is UB in C and gcc constant-folded 1<<63 to 0 while the runtime shift
  produced i64::MIN; `~x` is a real UnaryOp::BitNot (LLVM: xor with -1).
  wasmgen gained the new arms (kept the pre-existing and/or-for-&&-||
  conflation untouched). Parity: `bitwise_ops`, `shift_ops` (ALL backends);
  integration: `test_bitwise_ops_all_backends`,
  `test_bitwise_interpreter_matches_c`.
- [x] **B2. Sized/unsigned ints + casts** — `u8..u64`, `i8..i32`, `usize`, `as` casts.
  *(Done 2026-09-22.)* DESIGN: **i64-at-rest** — narrow ints live as i64 in
  every backend; arithmetic/comparisons run at i64; width+sign matter only at
  *typed stores* (let, assign, param binding, fn return), where the value
  wraps to the declared type. `usize` = 64-bit unsigned (same repr as i64).
  Implemented as a post-typecheck AST pass (`src/b2.rs`):
  `desugar_typed_stores` inserts `Expr::Cast` at every typed store, so all
  three backends implement exactly one wrapping primitive. The pass threads
  the variable→type map sequentially through statements (a typed `let` must
  be visible to later `Assign`s — the map-based first version silently missed
  re-assignment wraps). New tokens TypeI8..TypeU64/TypeUsize + `As`; ast.rs
  `Type::Int(IntTy{bits,signed})` + `Expr::Cast`; `as` precedence sits BETWEEN
  prefix unaries and binaries (new `parse_cast` level feeding
  parse_multiplication): `-1 as u8` → 255, `x as u32 + 1` → (x as u32) + 1.
  LLVM: `wrap_i64_to` (signed = trunc+sext, unsigned = shl+**lshr** — ashr
  sign-extends, wrong for unsigned); Cast = fptosi-to-i64-then-wrap for float
  sources; Neg emits `fneg double` for float operands (sub 0, x was invalid
  IR) and infer_llvm_type gained UnaryOp + ArrayLiteral (i64*) arms, fixing a
  pre-existing untyped `let a = [1,2,3]` LLVM crash. C: uint8_t..uint64_t
  names, wrap via C casts. PRE-EXISTING BUG FIXED: unit-suffix literals
  (`5 kg`, `3 s`) grabbed the unit IDENT across newlines, so
  `let x: i64 = 5\n g = g + 1` parsed as `5 g` and swallowed `g` (vars g/h/m/s
  + others failed to re-assign); unit suffixes now bind same-line only.
  Parity: `b2_casts`, `b2_narrow_wrap` (ALL backends); integration:
  `test_b2_int_types_all_backends`.
- [x] **B3. Wasm backend parity (integer programs)**
  *(Done 2026-09-22.)* The wasm backend went from compile-only prototype to a
  fourth executable backend: real locals (collect_locals declares every
  let/assign-bound name; params keep their names), named local.set/get,
  nested folded-WAT narrow-wrap on stores, `f64.trunc_f64_s` float-source
  casts (Cast-to-i64 emits the source identity — emitting nothing left the
  stack empty), range `for` (`..`/`..=`) with break/continue via labeled
  block/loop + BREAK/CONTINUE sentinels, i32 comparisons in condition
  position (`gen_wasm_cond`; truthiness = `i64.ne 0`) vs i64-extended
  comparisons in value position (`!x` = `extend_i32_u(i64.eqz)`).
  Validator subtlety: a result fn whose body ends in a value-if (branches
  return) still reaches the end in reachable state → trailing `(unreachable)`
  (`ends_with_return` matches literal trailing Return only).
  **SHARED FRONT-END PASS** (`b2::implicit_returns`, wired into every
  pipeline incl. interpreter): fns WITH a declared return type get trailing
  ExprStmt → explicit `return`, and a trailing value-yielding `if` (both
  branches) gets branch-level explicit returns — the language's recursion
  idiom now returns real values on all backends (fact/fib were 0 before).
  Ungated in unannotated fns it broke real programs (`return void_call()`
  doesn't compile in C) — has_ret gate is load-bearing.
  INTERPRETER FIXES: `for` over a Range iterated `0..count`, dropping the
  start (`1..=3` printed 0,1,2) — loop destructures the Range bounds now.
  Runner: `scripts/runwasm.mjs` (node; stubs console.log(i64), calls main);
  needs wabt's wat2wasm. Parity harness gains `Backend::Wasm` (skip with
  NOTE when wat2wasm/node missing; ci.yml parity job installs wabt).
  WASM GAPS (allowlist, documented in tests/parity.rs): strings (invalid WAT
  on print(str)), arrays/maps/structs, match arms never execute, bool
  literals print 1/0, lambdas. Parity: 11 integer cases ALL_INT incl. new
  `recursion` (fact+fib).

### Known deferred issues (found during work; not scheduled)
- ~~String arrays (`["a","b"]`) broken differently on all 3 backends (blocks `values()`)~~
  **Fixed (C1, 2026-09-23)**: heap-handle design. `sbx_strarr` for string arrays,
  `sbx_i64arr` for i64 arrays (incl. `map.values()`); both carry runtime length so
  len/index/for/print agree across C/LLVM/interpreter. `keys()`/`values()` now return
  real arrays. Parity: `str_array_basics`, `map_keys_values` (C+LLVM+interp).
- `break`/`continue` inside range loops mis-scope in edge cases; C corrupts its
  induction slot when the body redeclares a range-loop variable (LLVM/interp shadow).
  **Partially addressed 2026-09-24**: block-scoping renamer gives For bodies a
  fresh scope, so body `let`s no longer collide with the C induction slot; only
  `break`/`continue` edge cases remain (would need structured control flow).

## Decisions log
- 2026-09-15: Roadmap direction set (web first; machine-level = compiler-only).
  Maps = `map<string, int>` only (arrays are i64-only today; string arrays are
  broken — see deferred issues). Insertion-ordered maps (deterministic parity).
- 2026-09-23: C1 string/i64 arrays use opaque heap handles (`sbx_strarr`/
  `sbx_i64arr`, i8* at the LLVM ABI) that carry their own length — arrays are
  reference values (aliasing on `let b = a`), and bare-`long*`-style arrays
  without runtime length are never exposed to Sandbox code.
- 2026-09-15: Deploy pipeline = GHCR push + SSH compose deploy, rollback on
  failed health check; production environment requires user approval.
- 2026-09-24: **Block scoping is Rust-style, enforced in the typechecker.**
  `let` binds in the innermost block; inner blocks may shadow (a shadowing
  `let` is a NEW binding, not reassignment); using a name outside its block
  is an `Undefined variable` check error. Implementation: a front-end desugar
  pass `b2::resolve_block_scoping` (runs in all 4 compile pipelines, NOT in
  `Compiler::check`) renames shadowing `let`s to fresh `name__sN` and rewrites
  reads/assigns via a scope map, so no backend needs shadowing logic; If/While
  now push typechecker scopes like For/IfLet/Match already did. The
  interpreter additionally snapshots/restores its var maps around If/While
  arms (per-iteration for loops bodies) so runtime state can't leak across
  iterations — but it stays lax on unbound idents (evaluates 0): the
  typechecker is the enforcement point, matching B3's documented interp gaps.
  Parity: `block_scope_shadow`, `block_scope_fresh` (ALL_INT); integration:
  `block_scope_use_after_branch_is_rejected`,
  `block_scope_shadowing_is_fresh_binding`. Corpus modernized: `while_loop`
  and `break_continue` used `let i = i + 1` counters, which under the new
  rule shadow the loop variable and diverge by design (like Rust) — rewritten
  to `i = i + 1` and widened to ALL_INT (LLVM's old while-loop stale-value
  gap was in that body-`let` form and is now closed).  Parity harness hardened:
  every subprocess spawn goes through `run_with_timeout` (with a post-EOF
  exit wait), so a runaway program can no longer hang CI.
- 2026-09-25: **Browser playground** at `/playground` (served by the
  registry crate, all artifacts under `registry/static/playground/` so the
  registry-only Docker context stays intact). Compiles .sbx → wasm entirely
  in-page: `playground-compiler/` is a standalone cdylib crate that includes
  the compiler's leaf modules (lexer/parser/typechecker/b2/wasmgen + shared
  ast/token/diagnostic/stdlib) by path — the main crate is a binary and its
  `compiler.rs` needs `std::process::Command`, so it is NOT reusable on
  wasm32. C ABI (no wasm-bindgen): `sbx_version`, `sbx_alloc`, `sbx_free`,
  `sbx_compile(in,pkgs,err)` returning a guest ptr to `[len:u32 LE][payload]`
  (payload = .wat text; error message written to `err_out` envelope on
  failure, panic=abort). Packages for `use` are fetched by the page JS from
  the registry's own API (latest_version → download) and passed as a
  `name=source\0` blob, injected as ModuleDefs — the same shape the CLI uses
  for vendored packages. The page encodes the emitted .wat to a binary with
  its own encoder (`encodeWat`), byte-identical to wat2wasm on the
  supported subset (validated: `scripts/validate_encoder.mjs`, 25 of the 47
  parity-corpus WATs byte-identical; the other 22 are wat2wasm-rejected
  unsupported classes — strings, lambdas, undefined stdlib calls). The
  encoder mirrors wabt's DCE (dead code after an unconditional terminator
  is dropped; a result-function body that falls through gets a closing
  `unreachable` pin). **Drift risk**: the 9 module copies under
  playground-compiler/ are regenerated by hand — re-copy + re-run
  `scripts/build-playground.sh` (rebuilds the wasm artifact and installs it
  at registry/static/playground/compiler.wasm, which is committed because
  handlers use `include_bytes!`) whenever src leaf modules change, then
  `node scripts/smoke-playground-compiler.mjs` (ABI smoke: block scoping,
  package injection, error path) and the encoder validator.
- 2026-09-25: **Range-loop bodies can no longer corrupt the induction slot.**
  A body assignment to the loop variable (`i = ...`) used to rewrite C's
  `for (long i = ...; i < n; i++)` counter: the loop wedged (`i = 99`),
  stepped wrong (`i = i + 2` printed 2 5 8 …), or break/continue landed on
  the corrupted slot. Semantics settled on: **the loop variable is a fresh
  binding each iteration** (like a `let` in the body) — a body assignment
  hits the per-iteration copy, never the counter. Implementation is a b2
  desugar (in `resolve_block_scoping`, all 4 pipelines): range loops get a
  counter-fresh hidden slot (`<var>_it__sN`, so a user var named like the
  old plain-`__it` shape can't collide — the interpreter's variable map is
  flat) plus a leading `let i = <slot>` seed; a body `let i` still shadows
  via the recursive pass. **Scope: range loops only.** Element iteration
  (arrays, strings, string arrays, `.values()`) is intentionally NOT
  desugared — every backend re-initializes the binding from the iterable at
  the top of each iteration, so there is nothing to corrupt, and the
  interpreter binds string-array elements outside its i64 map where a seed
  would not survive. Parity: `for_range_body_assign_break`,
  `for_range_body_assign_continue`, `for_range_shadow_let_break`,
  `for_range_nested_shadow_break` (ALL_INT); integration:
  `for_range_body_assign_cannot_corrupt_counter`. Known-unfixed adjacent
  edge (pre-existing, all backends agree): a while-loop condition reading a
  variable that the body only shadow-lets never sees updates — infinite by
  the same Rust-style-shadowing semantics, not a backend divergence.
- 2026-09-25: **B2 completion: typed array indices + declared-type `let`
  bindings.** Roadmap item B2 (sized/unsigned ints, `as` casts) was already
  implemented; probing found two typechecker gaps and fixed both in
  `src/typechecker.rs`:
  (1) `Expr::Index` rejected narrow indices ("Array index must be i64, got
  'u64'") — now any `is_int_ty` index is accepted, since values are i64 at
  rest and need no conversion (`a[2 as usize]`, `a[j]` with `j: u8`).
  (2) `Stmt::Let` stored the **value's** type in scope, ignoring the
  annotation — so `let idx: usize = 1` passed as an index by accident while
  the equivalent cast was rejected. Now the declared type is bound (after a
  `types_compatible` check); assignments to declared-narrow vars already go
  through the implicit wrap store, so behavior is unchanged on all hosts.
  NOT changed (pre-existing, out of scope): LLVM `len(array_ident)` returns
  0 (arrays have no runtime header in the LLVM backend — the reason parity
  cases like `for_over_array` run C+interp only); wasm has no array/`len`
  lowering (documented unsupported classes). Parity:
  `b2_typed_indices`, `b2_declared_type_bindings` (C_LLVM_INTERP);
  integration: `b2_typed_indices_and_declared_bindings`.
- 2026-09-26: **Wasm backend: heap-allocated arrays** (indexing, `len()`,
  `for x in arr`) — the last of B3's allowlisted gaps closed. DESIGN: a
  bump-allocator heap after the data segment — `$heap` global starts at
  `(data_end + 7) & !7` (past string data); memory sized
  `pages = (heap_start + 4096).div_ceil(65536).max(1)`. Five runtime helpers
  from `emit_array_runtime` with **i64 boundaries** (array addresses are i64
  in user locals; `i32.wrap_i64` happens inside the helpers so user code
  never wraps): `$sbx_alloc`, `$sbx_arr_new` (allocs 8 + 8·count, stores the
  length header, returns the address zero-extended to i64), `$sbx_len`,
  `$sbx_get`, `$sbx_store`. Array repr = address of an 8-byte length header
  + packed i64 elements. Every value-returning helper ends in an explicit
  `(return ...)` — the JS encoder appends a fall-through `unreachable` pin to
  result bodies, which would otherwise fire at runtime. LOWERING: wasmgen
  tracks `array_vars` per fn (let/assign of an ArrayLiteral, `let b = a`
  aliasing, array-typed params) and discovers hidden temps `$sbxtmpN` during
  body gen, then splices their `(local ...)` declarations before the
  instructions afterwards (`output.split_off(body_start)`) — WAT requires
  locals declared first. Covers: literals (let/assign/anon-expr → temp
  address), `a[i]` reads with arbitrary index exprs, `len(arr)` (Ident in
  array_vars only), and `for x in arr` / `for x in [..]` (fresh idx temp,
  `br_if $break` on `!(idx < len)`, binding re-seeded via `$sbx_get` each
  iteration so a body reassign can't corrupt iteration). ENCODER:
  playground.js gained i32 add/sub/mul + `i32.wrap_i64`, i64.load/store with
  memarg immediates (align=3, offset=0), a Global section (id 6, between
  Memory and Export), typed params/locals/results, `global.get/set` with
  index immediates (global.set must fold its value child first — the first
  version didn't and failed validation); `valtypeByte` accepts numeric bytes
  (the type list stores raw bytes; it was double-mapping). Corpus:
  `tests/wat-corpus/wasm_arrays.wat` — 26 of 47 parity WATs now
  byte-identical to wat2wasm. PARITY: wasm matches C/interp everywhere they
  work and **beats the host backends on known gaps** (LLVM `len(array)` = 0
  and can't lower for-in over local arrays; interpreter binds array-typed
  params to 0 and len-through-alias reads 0; C decays param arrays and
  miscounts). New tier `C_INTERP_WASM` for `wasm_arrays_forin`;
  `wasm_arrays`, `wasm_arrays_exprs` are ALL_INT. Playground: Arrays example
  in the EXAMPLES list, footer notes arrays supported (strings/maps/structs
  still aren't); `compiler.wasm` rebuilt and committed (include_bytes!).
- 2026-09-26: **LLVM `len(array_ident)` returns the literal length instead
  of 0** (the documented B2-completion gap). llvmgen keeps an
  `array_lens: HashMap<String, usize>` static-fact map mirroring C's
  `array_lengths`: `let a = [..]` registers a's length, `let b = a`
  (an alias of a tracked i64*) inherits it, and any other initializer
  drops the entry. Soundness discipline: writes kill facts *everywhere*
  (Assign/non-literal Let inside loops and branches, IfLet pattern
  bindings); at If/IfLet joins and every loop exit the fact state is
  **intersected** with the entry state (a fact survives only if all paths
  agree it is unchanged), so a surviving entry is provably a constant —
  len() is either right or falls back to 0, never wrong. The dead-end
  first version gated registration on the sticky `left_entry` latch
  (arrays declared after a loop got no fact) and blanket-cleared at loop
  boundaries (facts died across benign literal for-ins, leaving
  `a[len(a)-1]` reading garbage); kill-on-write + intersect fixes both.
  Soundness quirk kept in check: the for-in-over-unsupported-iterable
  fall-through emits its body once inline in the caller's context, and
  facts now flow through kill-on-write — a body literal-let can establish
  a fact after that one-pass loop, sound only because body lets get fresh
  allocas and the "loop" never re-executes. Verified against C with a
  23-print probe across every construct; LLVM matches C except where C is
  itself wrong: aliases (C sizeof-decays a `long*` to 1; LLVM emits the
  true length) and `a[len(a)-1]` after a benign loop (C's earlier fact
  survives its non-intersected tracking). Known boundaries (documented in
  the parity case): branch/loop-body len reads are 0 on LLVM where C still
  answers; growing a literal reassign overflows C's fixed-size buffer
  (LLVM is fine); for-in over a local array *variable* still runs the body
  once inline (C_INTERP_WASM stays, comment corrected). Parity:
  `array_len_basics` (ALL_INT — all four backends; 57 programs, 135
  executions). playground-compiler bundles no llvmgen copy, so the browser
  artifact is unaffected.
