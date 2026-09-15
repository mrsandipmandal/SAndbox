#!/usr/bin/env bash
# Quick local mirror of the GitHub Actions "Binary Smoke" job.
# Usage: bash scripts/ci-smoke.sh
set -euo pipefail

BIN="${BIN:-target/debug/sandbox}"
if [ ! -f "$BIN" ]; then
  echo "Sandbox binary not found at $BIN; building..." >&2
  cargo build --quiet
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "===== hello ====="
printf 'fn main() {\n    print("Hello, Sandbox!")\n}\n' > "$TMP/hello.sbx"
cat "$TMP/hello.sbx"
echo "----- run -----"
# --quiet suppresses compiler progress lines: stdout is program output only.
OUT="$("$BIN" run --quiet "$TMP/hello.sbx")"
printf '%s\n' "$OUT"
if ! printf '%s\n' "$OUT" | grep -qFx "Hello, Sandbox!"; then
  echo "FAIL: hello smoke test" >&2
  exit 1
fi
echo "Hello smoke test passed."

echo "===== array ====="
printf 'fn main() {\n    for item in [10, 20, 30] {\n        print(item)\n    }\n}\n' > "$TMP/array.sbx"
cat "$TMP/array.sbx"
echo "----- run -----"
OUT="$("$BIN" run --quiet "$TMP/array.sbx")"
printf '%s\n' "$OUT"
if ! printf '%s\n' "$OUT" | diff -u - <(printf '10\n20\n30\n'); then
  echo "FAIL: array smoke test" >&2
  exit 1
fi
echo "Array smoke test passed."

echo "===== interpreter ====="
OUT="$($BIN interpret "$TMP/array.sbx")"
printf '%s\n' "$OUT"
if ! printf '%s\n' "$OUT" | diff -u - <(printf '10\n20\n30\n'); then
  echo "FAIL: interpreter smoke test" >&2
  exit 1
fi
echo "Interpreter smoke test passed."

echo "===== llvm backend ====="
# clang's -Woverride-module warning goes to stderr; silence it.
"$BIN" llvm-build "$TMP/array.sbx" -o "$TMP/llvm_prog" 2>/dev/null
OUT="$($TMP/llvm_prog)"
printf '%s\n' "$OUT"
if ! printf '%s\n' "$OUT" | diff -u - <(printf '10\n20\n30\n'); then
  echo "FAIL: LLVM backend smoke test" >&2
  exit 1
fi
echo "LLVM backend smoke test passed."

echo "===== smoke: all checks passed ====="
