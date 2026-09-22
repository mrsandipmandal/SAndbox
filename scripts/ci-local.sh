#!/usr/bin/env bash
# Run every .github/workflows/ci.yml gate locally in one command, in CI order,
# so you can verify a push before making it.
#
# Usage:
#   bash scripts/ci-local.sh               # all gates
#   bash scripts/ci-local.sh --fail-fast   # stop at the first failing gate
#   bash scripts/ci-local.sh --skip-registry
#
# Exits 0 only if every enabled gate passes.
set -uo pipefail

FAIL_FAST=0
SKIP_REGISTRY=0
for arg in "$@"; do
  case "$arg" in
    --fail-fast)    FAIL_FAST=1 ;;
    --skip-registry) SKIP_REGISTRY=1 ;;
    -h|--help)
      sed -n '2,10p' "$0"; exit 0 ;;
    *)
      echo "Unknown option: $arg (see --help)" >&2; exit 2 ;;
  esac
done

# Colors (disabled when not a terminal)
if [ -t 1 ]; then
  GREEN=$'\033[32m'; RED=$'\033[31m'; YELLOW=$'\033[33m'; BOLD=$'\033[1m'; RESET=$'\033[0m'
else
  GREEN=""; RED=""; YELLOW=""; BOLD=""; RESET=""
fi

RESULTS=()
TOTAL_START=$(date +%s)
ANY_FAILED=0

# run_gate NAME [--dir DIR] -- CMD [args...]
# Runs CMD (optionally after cd DIR, in this same shell so status and
# bookkeeping propagate) and records PASS/FAIL with duration.
GATE_DIR="."
run_gate() {
  local name="$1"; shift
  if [ "$1" = "--dir" ]; then GATE_DIR="$2"; shift 2; else GATE_DIR="."; fi
  printf '%s\n' "${BOLD}━━━ gate: ${name} ━━━${RESET}"
  local start status saved
  saved=$PWD
  if [ "$GATE_DIR" != "." ]; then
    cd "$GATE_DIR" || { echo "cannot cd $GATE_DIR" >&2; exit 2; }
  fi
  start=$(date +%s)
  "$@" > /tmp/ci-local-last.log 2>&1
  status=$?
  cd "$saved"
  local dur=$(( $(date +%s) - start ))

  if [ "$status" -eq 0 ]; then
    RESULTS+=("PASS  ${name}  (${dur}s)")
    printf '%s\n' "${GREEN}✓ ${name} passed${RESET} (${dur}s)"
  else
    RESULTS+=("FAIL  ${name}  (${dur}s)")
    ANY_FAILED=1
    printf '%s\n' "${RED}✗ ${name} FAILED${RESET} (${dur}s) — last 40 lines:"
    tail -n 40 /tmp/ci-local-last.log
    if [ "$FAIL_FAST" -eq 1 ]; then
      printf '%s\n' "${YELLOW}--fail-fast: stopping.${RESET} Full log: /tmp/ci-local-last.log"
      exit 1
    fi
  fi
  printf '%s\n' ""
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 2
  fi
}

require_cmd cargo
require_cmd gcc
require_cmd python3

# Optional but recommended: wabt enables the wasm backend in the parity gate.
# Without it the wasm cases are skipped with a NOTE (harness handles this).
if ! command -v wat2wasm >/dev/null 2>&1; then
  echo "note: wat2wasm not found — wasm parity cases will be skipped (apt install wabt)"
fi

# ── Gate 1: check (fmt + clippy + build + tests) ────────────────────────────
run_gate "cargo fmt"        cargo fmt --all -- --check
run_gate "cargo clippy"     cargo clippy --all-targets -- -D warnings
run_gate "cargo build"      cargo build --quiet
run_gate "cargo test"       cargo test --quiet

# ── Gate 2: binary smoke (C + LLVM + interpreter) ───────────────────────────
run_gate "binary smoke"     bash scripts/ci-smoke.sh

# ── Gate 3: parity ───────────────────────────────────────────────────────────
run_gate "parity"           cargo test --quiet --test parity

# ── Gate 4: registry (separate crate) ────────────────────────────────────────
if [ "$SKIP_REGISTRY" -eq 1 ]; then
  RESULTS+=("SKIP  registry gates  (--skip-registry)")
  printf '%s\n' "${YELLOW}→ registry gates skipped (--skip-registry)${RESET}"
  printf '%s\n' ""
else
  if [ -f registry/Cargo.toml ]; then
    run_gate "registry fmt"    --dir registry cargo fmt --all -- --check
    run_gate "registry clippy" --dir registry cargo clippy --all-targets -- -D warnings
    run_gate "registry build"  --dir registry cargo build --quiet
    run_gate "registry test"   --dir registry cargo test --quiet
  else
    RESULTS+=("SKIP  registry gates  (registry/ not present)")
    printf '%s\n' "${YELLOW}→ registry gates skipped (registry/ not present)${RESET}"
    printf '%s\n' ""
  fi
fi

# ── Summary ──────────────────────────────────────────────────────────────────
TOTAL_DUR=$(( $(date +%s) - TOTAL_START ))
printf '%s\n' "${BOLD}━━━━━━━━━━━━━━━━ summary ━━━━━━━━━━━━━━━━${RESET}"
printf '%s\n' "${RESULTS[@]}" \
  | sed "s/^PASS/${GREEN}PASS${RESET}/; s/^FAIL/${RED}FAIL${RESET}/; s/^SKIP/${YELLOW}SKIP${RESET}/"
printf '%s\n' ""
if [ "$ANY_FAILED" -eq 0 ]; then
  printf '%s\n' "${GREEN}${BOLD}All gates passed in ${TOTAL_DUR}s — safe to push.${RESET}"
else
  printf '%s\n' "${RED}${BOLD}Some gates FAILED (total ${TOTAL_DUR}s) — fix before pushing.${RESET}"
  printf '%s\n' "Full log of the last failing gate: /tmp/ci-local-last.log"
fi
exit "$ANY_FAILED"
