# Release Process

How releases are cut, and how version numbers work — including the
patch-999 carry rule.

## Versioning convention

- Format: `MAJOR.MINOR.PATCH`, tagged `v<version>` (e.g. `v0.0.5`).
- The release line follows the **GitHub Releases sequence**
  (v0.0.1 → v0.0.4 → v0.0.5 → …). Tags from the older, abandoned scheme
  (v0.2.0 … v1.1.0) remain in history but are no longer the reference.

### The 999-carry rule

A component never exceeds 999. When incrementing would push a component
past 999, it resets to 0 and the next-higher component increments —
no matter how many times it has been incremented before:

```
1.0.999  --patch-->  1.1.0
1.1.999  --patch-->  1.2.0
0.0.999  --patch-->  0.1.0
1.999.x  --minor-->  2.0.x
```

What each bump means for this project:

| bump  | when to use it                                          |
|-------|---------------------------------------------------------|
| patch | bug fixes, compiler/CI/tooling fixes, doc changes       |
| minor | new language or backend features (e.g. total division), |
|       | new tooling surfaces (registry, playground)             |
| major | breaking language or ABI changes                        |

The `registry/` crate is versioned independently (it is a server, not
the language) and does not participate in the release flow.

## Where the version lives (kept in sync by release.sh)

1. `Cargo.toml` → `version` (the language/CLI crate)
2. `playground-compiler/Cargo.toml` → `version`
3. `src/main.rs` → uses `env!("CARGO_PKG_VERSION")`, so `sandbox
   --version` follows Cargo automatically — never hardcode it there.
4. `CHANGELOG.md` → a `## [<version>] - <date>` section

## Release runbook

### One-command flow (preferred)

```bash
bash scripts/release.sh patch   # or: minor | major
```

The script: verifies the tree is clean and HEAD is pushed → computes
the next version with the 999-carry rule → bumps all version locations
→ inserts a dated `## [<version>]` stub into CHANGELOG.md → runs the
fast gates (build + smoke) → commits `release: v<version>` and tags
`v<version>`. Pass `--push` to also push `master` and the tag; without
it, push yourself:

```bash
git push origin master && git push origin v0.0.5
```

### What happens after the tag lands on GitHub

`.github/workflows/release.yml` runs automatically:

1. Builds all five platforms (x86_64/aarch64 Linux, x86_64/aarch64
   macOS, x86_64 Windows).
2. Per-target **binary verification**: the binary must exist, be a
   sane size, and `file(1)` must report the expected architecture —
   a wrong-arch cross-compile fails that matrix leg.
3. **Artifact-completeness gate**: all five platform archives must be
   present before checksums are generated; a partial release cannot
   publish.
4. Publishes the GitHub Release with the archives + `SHA256SUMS.txt`
   (self-checked with `sha256sum --check` before upload).

### Post-release checklist

- [ ] GitHub Release exists with 5 archives + `SHA256SUMS.txt`
- [ ] `sandbox --version` in the released binary prints the new version
- [ ] CI (`workflow-lint`, parity, playground) is green on the release commit
- [ ] If anything failed: fix, `release.sh patch` again (a failed tag
      may be deleted locally and on the remote **only if no release was
      published** — never rewrite a published release)
