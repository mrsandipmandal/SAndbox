#!/usr/bin/env bash
# Cut a release: bump the version everywhere, stub the changelog, run the
# fast gates, commit and tag. See RELEASE.md for the full runbook.
#
# Usage:
#   bash scripts/release.sh [patch|minor|major] [--push]
#
#   patch|minor|major   which component to bump (default: patch)
#   --push              also push master and the tag after tagging
#
# Version rule: components never exceed 999 — incrementing past 999
# carries (1.0.999 --patch--> 1.1.0). See RELEASE.md.
set -euo pipefail
cd "$(dirname "$0")/.."

BUMP="patch"
PUSH=0
for arg in "$@"; do
  case "$arg" in
    patch|minor|major) BUMP="$arg" ;;
    --push)            PUSH=1 ;;
    -h|--help) sed -n '2,12p' "$0"; exit 0 ;;
    *) echo "unknown argument: $arg (see --help)" >&2; exit 2 ;;
  esac
done

# ── Preconditions ────────────────────────────────────────────────────────────
if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
  echo "FAIL: working tree has uncommitted changes — commit or stash first." >&2
  exit 1
fi
BRANCH="$(git rev-parse --abbrev-ref HEAD)"
if [ "$BRANCH" != "master" ] && [ "$BRANCH" != "main" ]; then
  echo "FAIL: releases are cut from master (on: $BRANCH)." >&2
  exit 1
fi
git fetch origin --quiet
TIP="$(git rev-parse HEAD)"
REMOTE_TIP="$(git rev-parse "origin/${BRANCH}")"
if [ "$TIP" != "$REMOTE_TIP" ]; then
  echo "FAIL: HEAD ($TIP) is not the tip of origin/${BRANCH} ($REMOTE_TIP)." >&2
  echo "      Push or pull first so the release commit is on the remote line." >&2
  exit 1
fi

# ── Read + bump the version (999-carry rule) ────────────────────────────────
CUR="$(grep -m1 '^version' Cargo.toml | sed 's/version = "\(.*\)"/\1/')"
MAJOR="${CUR%%.*}"; REST="${CUR#*.}"
MINOR="${REST%%.*}"; PATCH="${REST##*.}"

case "$BUMP" in
  major)
    if [ "$MAJOR" -ge 999 ]; then
      echo "FAIL: major $MAJOR cannot carry past 999." >&2; exit 1
    fi
    MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
  minor)
    if [ "$MINOR" -ge 999 ]; then
      if [ "$MAJOR" -ge 999 ]; then
        echo "FAIL: minor $MINOR cannot carry past 999 at major $MAJOR." >&2; exit 1
      fi
      MAJOR=$((MAJOR + 1)); MINOR=0
    else
      MINOR=$((MINOR + 1))
    fi
    PATCH=0 ;;
  patch)
    if [ "$PATCH" -ge 999 ]; then
      if [ "$MINOR" -ge 999 ]; then
        if [ "$MAJOR" -ge 999 ]; then
          echo "FAIL: version $CUR is at the top of the representable range." >&2; exit 1
        fi
        MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0
      else
        MINOR=$((MINOR + 1)); PATCH=0
      fi
    else
      PATCH=$((PATCH + 1))
    fi ;;
esac

NEXT="${MAJOR}.${MINOR}.${PATCH}"
TAG="v${NEXT}"
echo "release: $CUR -> $NEXT ($BUMP)"

if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
  echo "FAIL: tag $TAG already exists." >&2
  exit 1
fi

# ── Apply the bump ───────────────────────────────────────────────────────────
for toml in Cargo.toml playground-compiler/Cargo.toml; do
  sed -i "0,/^version = \".*\"/s//version = \"${NEXT}\"/" "$toml"
done

DATE="$(date +%F)"
STUB="## [${NEXT}] - ${DATE}

### Added

- (describe user-visible additions here)

### Fixed

- (describe fixes here)

"
awk -v stub="$STUB" '
  { print }
  /^## \[/ && !done {
    printf "%s", stub
    done = 1
  }
' CHANGELOG.md > CHANGELOG.md.tmp && mv CHANGELOG.md.tmp CHANGELOG.md

# ── Fast gates ───────────────────────────────────────────────────────────────
echo "── gate: cargo build ──"
cargo build --quiet
echo "── gate: playground build ──"
cargo build --quiet --manifest-path playground-compiler/Cargo.toml
echo "── rebuild playground compiler.wasm ──"
# The committed artifact carries the version string (sbx_version); rebuild
# it here so the playground About line never trails the release version.
bash scripts/build-playground.sh
echo "── gate: version consistency ──"
CLI_VERSION="$(cargo run --quiet -- --version | awk '{print $2}')"
if [ "$CLI_VERSION" != "$NEXT" ]; then
  echo "FAIL: sandbox --version reports $CLI_VERSION, expected $NEXT." >&2
  exit 1
fi
echo "── gate: unit tests ──"
cargo test --quiet --manifest-path playground-compiler/Cargo.toml
echo "── gate: compiler.wasm version consistency ──"
node scripts/smoke-playground-compiler.mjs \
  registry/static/playground/compiler.wasm "$NEXT"

# ── Commit + tag ─────────────────────────────────────────────────────────────
git add Cargo.toml playground-compiler/Cargo.toml CHANGELOG.md src/main.rs \
  registry/static/playground/compiler.wasm
# src/main.rs only changes on the first release after the env!() fix; adding
# an unchanged file is a no-op, so this is safe on every run.
git commit -m "release: v${NEXT}"
git tag "${TAG}"
echo "tagged ${TAG} on $(git rev-parse --short HEAD)"

if [ "$PUSH" -eq 1 ]; then
  git push origin "${BRANCH}"
  git push origin "${TAG}"
  echo "pushed ${BRANCH} + ${TAG} — release.yml will build and publish."
else
  echo "next: git push origin ${BRANCH} && git push origin ${TAG}"
fi
