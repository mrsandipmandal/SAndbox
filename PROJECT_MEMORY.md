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
  gap was in that body-`let` form and is now closed). Parity harness hardened:
  every subprocess spawn goes through `run_with_timeout` (with a post-EOF
  exit wait), so a runaway program can no longer hang CI.
