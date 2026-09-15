#!/usr/bin/env bash
# Binary smoke checks for the sandbox compiler — single source of truth.
#
# Consumed by:
#   - the "Binary Smoke (C + LLVM + Interpreter)" job in
#     .github/workflows/ci.yml (one thin step per check, for per-check red X
#     granularity in the Actions UI), and
#   - scripts/ci-local.sh (all checks in one gate).
#
# Usage:
#   bash scripts/ci-smoke.sh                # all checks
#   bash scripts/ci-smoke.sh hello array    # only the named checks
#
# Checks: hello | array | interp | llvm
set -euo pipefail

BIN="${BIN:-target/debug/sandbox}"
if [ ! -f "$BIN" ]; then
  echo "Sandbox binary not found at $BIN; building..." >&2
  cargo build --quiet
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# ── Fixtures (defined once; no backend-specific duplication) ────────────────

cat > "$TMP/hello.sbx" <<'EOF'
fn main() {
    print("Hello, Sandbox!")
}
EOF

cat > "$TMP/array.sbx" <<'EOF'
fn main() {
    for item in [10, 20, 30] {
        print(item)
    }
}
EOF

ARRAY_EXPECTED=$'10\n20\n30'

# ── Assertion helpers ────────────────────────────────────────────────────────

fail() { echo "FAIL: $1 smoke test" >&2; exit 1; }

expect_output() { # name expected actual
  if ! printf '%s\n' "$3" | diff -u - <(printf '%s\n' "$2"); then
    fail "$1"
  fi
}

# ── Checks ───────────────────────────────────────────────────────────────────

check_hello() {
  echo "===== hello (C backend) ====="
  cat "$TMP/hello.sbx"
  echo "----- run -----"
  # --quiet suppresses compiler progress lines: stdout is program output only.
  OUT="$("$BIN" run --quiet "$TMP/hello.sbx")"
  printf '%s\n' "$OUT"
  if ! printf '%s\n' "$OUT" | grep -qFx "Hello, Sandbox!"; then
    fail "hello"
  fi
  echo "Hello smoke test passed."
}

check_array() {
  echo "===== array (C backend) ====="
  cat "$TMP/array.sbx"
  echo "----- run -----"
  OUT="$("$BIN" run --quiet "$TMP/array.sbx")"
  printf '%s\n' "$OUT"
  expect_output "array" "$ARRAY_EXPECTED" "$OUT"
  echo "Array smoke test passed."
}

check_interp() {
  echo "===== interpreter ====="
  # The interpreter prints no banner; stdout is program output only.
  OUT="$("$BIN" interpret "$TMP/array.sbx")"
  printf '%s\n' "$OUT"
  expect_output "interpreter" "$ARRAY_EXPECTED" "$OUT"
  echo "Interpreter smoke test passed."
}

check_llvm() {
  echo "===== llvm backend ====="
  # sandbox -> .ll -> clang; clang's -Woverride-module warning goes to stderr.
  "$BIN" llvm-build "$TMP/array.sbx" -o "$TMP/llvm_prog" 2>/dev/null
  OUT="$("$TMP/llvm_prog")"
  printf '%s\n' "$OUT"
  expect_output "LLVM backend" "$ARRAY_EXPECTED" "$OUT"
  echo "LLVM backend smoke test passed."
}

# ── Dispatch ─────────────────────────────────────────────────────────────────

ALL_CHECKS=(hello array interp llvm)

if [ "$#" -eq 0 ]; then
  set -- "${ALL_CHECKS[@]}"
fi

for name in "$@"; do
  case "$name" in
    hello)  check_hello ;;
    array)  check_array ;;
    interp) check_interp ;;
    llvm)   check_llvm ;;
    *)
      echo "Unknown smoke check: $name" >&2
      echo "Valid checks: ${ALL_CHECKS[*]}" >&2
      exit 2
      ;;
  esac
done

echo "===== smoke: all requested checks passed ====="
