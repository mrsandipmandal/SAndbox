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
  Parity case `map_basics` (ALL backends). *(values() deferred: needs dynamic
  string arrays.)*
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
- [ ] **B2. Sized/unsigned ints + casts** — `u8..u64`, `i8..i32`, `usize`, `as` casts.

### Known deferred issues (found during work; not scheduled)
- String arrays (`["a","b"]`) broken differently on all 3 backends (blocks `values()`).
- `break`/`continue` inside range loops mis-scope in edge cases; C corrupts its
  induction slot when the body redeclares a range-loop variable (LLVM/interp shadow).
- Brand-new name declared in a branch and used after it: C rejects, interp accepts,
  LLVM emitted invalid IR (needs a scoping/typechecker decision first).

## Decisions log
- 2026-09-15: Roadmap direction set (web first; machine-level = compiler-only).
  Maps = `map<string, int>` only (arrays are i64-only today; string arrays are
  broken — see deferred issues). Insertion-ordered maps (deterministic parity).
- 2026-09-15: Deploy pipeline = GHCR push + SSH compose deploy, rollback on
  failed health check; production environment requires user approval.
