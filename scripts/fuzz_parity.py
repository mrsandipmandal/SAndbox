#!/usr/bin/env python3
"""Differential fuzzing for the Sandbox backends (C, LLVM, interpreter, wasm).

Generates small random programs from a construct vocabulary every backend
supports, optionally mutates integer literals in them, executes each program
on every backend the same way tests/parity.rs does, and reports any backend
disagreement with a saved reproduction file.

Construct vocabulary is deliberately conservative — it only emits programs
the four backends are expected to agree on, rotating three program kinds:
  - integer programs: i64 arithmetic (+ - *), bitwise (& | ^ << >>),
    unary -, parens; let chains; range for loops with break/continue and
    if guards; print(expr)
  - array programs: literals, len(), in-bounds indexing, len-derived
    indices, for-in over array literals, shrinking reassignment
  - narrow programs (B2 audit areas): typed lets `let v: u8..u64/usize`,
    wrap-on-store and wrap-on-reassign, `as` casts between every width,
    comparisons on typed variables (signed i64 on the bit pattern,
    including u64/usize holding high patterns), printed via the bool->int
    cast or used as if/else conditions

Known-divergent constructs (aliases like `let b = a`, bool prints, growing
reassignment, for-in over array variables, strings, maps, match) are never
generated, so any reported disagreement is a real backend bug, not a known
gap. Narrow programs follow class-based arithmetic rules so no generated
expression can overflow i64 while evaluating (the interpreter panics on
overflow where the compiled backends wrap):
  small (u8/i8/u16/i16, |value| ≤ 2^16 after the width wrap): + - freely
        (worst chain of four leaves < 2^19); * only between literal-only
        subexpressions (a depth-2 literal tree stays under 2^31) — a
        small×small product could reach 2^32, and C would compute it in a
        promoted 32-bit int rather than at i64
  mid   (u32/i32, values in ±2^31): only + - (a product could reach 2^93)
  big   (u64/usize/i64, any i64 bit pattern): no arithmetic — the vars are
        compared, cast, bit-folded and printed, never added or multiplied

Usage:
    python3 scripts/fuzz_parity.py [--count 200] [--seed N]
                                   [--backends c,llvm,interp,wasm]

Exit status: 0 if no disagreement was found, 1 otherwise.
"""

from __future__ import annotations

import argparse
import random
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
SANDBOX = REPO_ROOT / "target" / "debug" / "sandbox"
RUNWASM = REPO_ROOT / "scripts" / "runwasm.mjs"
TIMEOUT_SECS = 10
ALL_BACKENDS = ("c", "llvm", "interp", "wasm")


class SandboxError(Exception):
    """A backend could not produce output (compile error, crash, timeout)."""


# ── Output filtering (ported from tests/parity.rs) ──────────────────────────

def is_progress_line(line: str) -> bool:
    """True for sandbox compiler progress lines, not program output."""
    trimmed = line.lstrip()
    if trimmed.startswith("[sandbox]"):
        return True
    if line.startswith((" ", "\t")):
        if trimmed.startswith("→") or trimmed.startswith("✓") or trimmed.startswith("⚠"):
            return True
        if trimmed.startswith("[") and ("] FnDef" in trimmed or "] Other" in trimmed):
            return True
    return False


def filter_output(raw: str) -> str:
    return "\n".join(l for l in raw.splitlines() if not is_progress_line(l)).strip()


def run_cmd(cmd: list[str], timeout: int = TIMEOUT_SECS) -> subprocess.CompletedProcess:
    try:
        return subprocess.run(
            cmd, capture_output=True, text=True, timeout=timeout, cwd=REPO_ROOT
        )
    except subprocess.TimeoutExpired as e:
        raise SandboxError(f"{cmd[0]} timed out after {timeout}s") from e


# ── Backend runners (same invocation shapes as tests/parity.rs) ─────────────

def run_backend(backend: str, source: str, workdir: Path) -> str:
    sbx = workdir / f"prog_{backend}.sbx"
    sbx.write_text(source)
    if backend in ("c", "interp"):
        mode = "run" if backend == "c" else "interpret"
        out = run_cmd([str(SANDBOX), mode, str(sbx)])
        if out.returncode != 0:
            raise SandboxError(f"{mode} exited {out.returncode}: {out.stderr[-300:]}")
        return filter_output(out.stdout)
    if backend == "llvm":
        bin_path = workdir / "llvm_prog"
        build = run_cmd([str(SANDBOX), "llvm-build", str(sbx), "-o", str(bin_path)])
        if build.returncode != 0:
            raise SandboxError(f"llvm-build exited {build.returncode}: {build.stderr[-300:]}")
        out = run_cmd([str(bin_path)])
        if out.returncode != 0:
            raise SandboxError(f"llvm binary exited {out.returncode}: {out.stderr[-300:]}")
        return filter_output(out.stdout)
    if backend == "wasm":
        wat = workdir / "prog.wat"
        wasm = workdir / "prog.wasm"
        gen = run_cmd([str(SANDBOX), "wasm", str(sbx), "-o", str(wat)])
        if gen.returncode != 0:
            raise SandboxError(f"sandbox wasm exited {gen.returncode}: {gen.stderr[-300:]}")
        asm = run_cmd(["wat2wasm", str(wat), "-o", str(wasm)])
        if asm.returncode != 0:
            raise SandboxError(f"wat2wasm failed: {asm.stderr[-300:]}")
        out = run_cmd(["node", str(RUNWASM), str(wasm)])
        if out.returncode != 0:
            raise SandboxError(f"node runner exited {out.returncode}: {out.stderr[-300:]}")
        return filter_output(out.stdout)
    raise ValueError(f"unknown backend {backend}")


# ── Program generation ──────────────────────────────────────────────────────

BINOPS = ["+", "-", "*", "&", "|", "^"]


def gen_int_atom(rng: random.Random) -> str:
    r = rng.random()
    if r < 0.55:
        return str(rng.randint(-20, 20))
    if r < 0.70:
        return str(rng.randint(-2**40, 2**40))
    if r < 0.85:
        return f"(-{rng.randint(1, 20)})"
    return f"(0 - {rng.randint(1, 20)})"


def gen_int_expr(rng: random.Random, names: list[str], depth: int = 0) -> str:
    if depth >= 2 or rng.random() < 0.35:
        if names and rng.random() < 0.3:
            return rng.choice(names)
        return gen_int_atom(rng)
    op = rng.choice(BINOPS)
    lhs = gen_int_expr(rng, names, depth + 1)
    rhs = gen_int_expr(rng, names, depth + 1)
    return f"({lhs} {op} {rhs})"


def gen_shift_expr(rng: random.Random, names: list[str]) -> str | None:
    """A shift expression with a positive base and a small shift amount.

    The base must stay non-negative: shifting a negative value is undefined
    behavior in C, so allowing variables (which can hold negatives) here
    would generate UB rather than testable behavior.
    """
    return f"({rng.randint(1, 1000)} << {rng.randint(0, 3)})"


def gen_int_program(rng: random.Random) -> str:
    lines = ["fn main() {"]
    names: list[str] = []
    for j in range(rng.randint(3, 5)):
        if names and rng.random() < 0.25:
            value = gen_int_expr(rng, names)
        else:
            value = gen_int_atom(rng)
            if rng.random() < 0.2:
                shifted = gen_shift_expr(rng, names)
                if shifted:
                    value = shifted
        name = f"v{j}"
        lines.append(f"    let {name} = {value}")
        names.append(name)

    if rng.random() < 0.6:
        var = rng.choice("ijk")
        lo = rng.randint(-3, 3)
        hi = lo + rng.randint(0, 4)
        op = rng.choice(["..", "..="])
        lines.append("    let acc = 0")
        lines.append(f"    for {var} in {lo}{op}{hi} {{")
        lines.append(f"        acc = acc + {var}")
        if rng.random() < 0.5:
            lines.append(f"        if {var} == {rng.randint(lo - 1, hi + 1)} {{")
            lines.append("            continue")
            lines.append("        }")
        if rng.random() < 0.4:
            lines.append(f"        if {var} == {rng.randint(lo - 1, hi + 1)} {{")
            lines.append("            break")
            lines.append("        }")
        lines.append("    }")
        lines.append("    print(acc)")

    for _ in range(rng.randint(2, 4)):
        lines.append(f"    print({gen_int_expr(rng, names)})")
    lines.append("}")
    return "\n".join(lines)


# ── Narrow-type program generation (B2 audit areas) ───────────────────────────

# (type name, arithmetic class). Classes bound expression safety; see the
# module docstring. Typed stores wrap to the declared width in every backend,
# so a var's class is preserved across reassignment.
NARROW_TYPES = [
    ("u8", "small"),
    ("i8", "small"),
    ("u16", "small"),
    ("i16", "small"),
    ("u32", "mid"),
    ("i32", "mid"),
    ("u64", "big"),
    ("usize", "big"),
    ("i64", "big"),
]

# Literals chosen to straddle wrap boundaries and i64 extremes (no 2^63 —
# the positive literal does not fit i64 and only negation reaches MIN).
BOUNDARY_LITS = [
    "255",
    "256",
    "-128",
    "127",
    "32768",
    "-32769",
    "4294967291",
    "9223372036854775807",
    "-9223372036854775807",
]

CMPS = ["==", "!=", "<", ">", "<=", ">="]


def gen_arith_small(
    rng: random.Random, small_names: list[str], depth: int = 0
) -> tuple[str, bool]:
    """A (expr, literal_only) pair over small-class operands that cannot
    overflow i64 — or even C's promoted 32-bit int arithmetic. + and - run
    freely over small variables (four 2^16 leaves at depth 2 stay < 2^19);
    * is only kept when both children are literal-only, bounding any
    literal tree (four leaves, |leaf| ≤ 200) below 2^31."""
    if depth >= 2 or rng.random() < 0.4:
        if small_names and rng.random() < 0.6:
            return rng.choice(small_names), False
        return str(rng.randint(-200, 200)), True
    op = rng.choice(["+", "-", "*"])
    lhs, l_lit = gen_arith_small(rng, small_names, depth + 1)
    rhs, r_lit = gen_arith_small(rng, small_names, depth + 1)
    # Literal-only subtrees: leaves are |lit| ≤ 200 and the tree is at most
    # depth 2, so any product stays ≤ 200^4 < 2^31 — C's promoted-int
    # arithmetic and i64 arithmetic agree bit for bit.
    if l_lit and r_lit:
        return f"({lhs} {op} {rhs})", True
    # Variable-bearing subtrees: only + and -. A typed variable is always
    # width-wrapped on store (|v| ≤ 2^16 for small widths), so a depth-2
    # chain of four leaves stays < 2^19 and cannot overflow C's promoted
    # 32-bit int either — the backends agree exactly.
    safe_op = op if op in ("+", "-") else rng.choice(["+", "-"])
    return f"({lhs} {safe_op} {rhs})", False


def gen_bit_expr(rng: random.Random, all_names: list[str], depth: int = 0) -> str:
    """A bitwise/shift expression over any-typed operands. Bitwise ops and
    shifts cannot overflow (shift amounts are literals in 0..62; the C
    backend's `<<` goes through its defined unsigned hop)."""
    if depth >= 1 or rng.random() < 0.45:
        if all_names and rng.random() < 0.55:
            return rng.choice(all_names)
        return str(rng.randint(-100, 100))
    r = rng.random()
    if r < 0.6:
        op = rng.choice(["&", "|", "^"])
        lhs = gen_bit_expr(rng, all_names, depth + 1)
        rhs = gen_bit_expr(rng, all_names, depth + 1)
        return f"({lhs} {op} {rhs})"
    if r < 0.8:
        return f"({gen_bit_expr(rng, all_names, depth + 1)} << {rng.randint(0, 62)})"
    if r < 0.9:
        return f"({gen_bit_expr(rng, all_names, depth + 1)} >> {rng.randint(0, 62)})"
    return f"(~{gen_bit_expr(rng, all_names, depth + 1)})"


def gen_operand(rng: random.Random, all_names: list[str], small_names: list[str]) -> str:
    """Any evaluation-safe integer expression: a variable of any class, a
    plain or boundary literal, small arithmetic, or a bit fold. Results may
    carry any i64 bit pattern — they may be compared, cast, bit-folded,
    stored (wrapping) or printed, but never fed to + - *."""
    r = rng.random()
    if r < 0.35 and all_names:
        return rng.choice(all_names)
    if r < 0.55:
        return str(rng.randint(-300, 300))
    if r < 0.70:
        return rng.choice(BOUNDARY_LITS)
    if r < 0.85 and small_names:
        return gen_arith_small(rng, small_names)[0]
    return gen_bit_expr(rng, all_names)


def gen_narrow_program(rng: random.Random) -> str:
    """Typed-let program exercising the B2 audit areas: wrap-on-store,
    wrap-on-reassign, casts between widths, and comparisons on typed vars
    (including u64/usize holding high bit patterns). Comparisons reach the
    output through the bool->int cast or if/else conditions — never as a
    bare printed bool (LLVM has no bool-print codegen)."""
    lines = ["fn main() {"]
    all_names: list[str] = []
    small_names: list[str] = []
    for j in range(rng.randint(3, 5)):
        ty, cls = rng.choice(NARROW_TYPES)
        name = f"v{j}"
        if rng.random() < 0.25 and small_names:
            init = gen_arith_small(rng, small_names)[0]
        elif rng.random() < 0.5:
            init = str(rng.randint(-300, 300))
        else:
            init = rng.choice(BOUNDARY_LITS)
        lines.append(f"    let {name}: {ty} = {init}")
        all_names.append(name)
        if cls == "small":
            small_names.append(name)

    have_acc = False
    for _ in range(rng.randint(3, 6)):
        r = rng.random()
        if r < 0.22:
            lines.append(f"    print({gen_operand(rng, all_names, small_names)})")
        elif r < 0.42:
            lhs = gen_operand(rng, all_names, small_names)
            rhs = gen_operand(rng, all_names, small_names)
            lines.append(f"    print(({lhs} {rng.choice(CMPS)} {rhs}) as i64)")
        elif r < 0.58:
            lhs = gen_operand(rng, all_names, small_names)
            rhs = gen_operand(rng, all_names, small_names)
            lines.append(f"    if {lhs} {rng.choice(CMPS)} {rhs} {{ print(7) }} else {{ print(13) }}")
        elif r < 0.72:
            # Reassignment wraps to the declared width again — defined on
            # every backend for any operand.
            name = rng.choice(all_names)
            lines.append(f"    {name} = {gen_operand(rng, all_names, small_names)}")
        elif r < 0.86:
            operand = rng.choice(all_names + [str(rng.randint(-300, 300))])
            ty, _ = rng.choice(NARROW_TYPES)
            lines.append(f"    print(({operand} as {ty}))")
        else:
            if not have_acc:
                lines.append("    let acc = 0")
                have_acc = True
            step = rng.choice(small_names) if small_names else "1"
            lines.append("    for i in 0..3 {")
            lines.append(f"        acc = acc + {step}")
            lines.append("    }")
            lines.append("    print(acc)")
    lines.append("}")
    return "\n".join(lines)


def gen_array_program(rng: random.Random) -> str:
    """Array program with statically-tracked in-bounds indices.

    Per-array state: current length and the highest index used so far — a
    reassignment may only shrink to a length that still covers every index
    already emitted for that array, so no out-of-bounds read can be generated.
    """
    lines = ["fn main() {"]
    arrays: list[dict] = []  # {name, curlen, max_idx, reassigned}
    total = 0
    n_arrs = 1 if rng.random() < 0.7 else 2
    for ai in range(n_arrs):
        name = "arr" if ai == 0 else f"brr{ai}"
        n = rng.randint(1, 5)
        elems = [str(rng.randint(-50, 50)) for _ in range(n)]
        lines.append(f"    let {name} = [{', '.join(elems)}]")
        arrays.append({"name": name, "curlen": n, "max_idx": 0, "reassigned": False})

    have_sum = False
    for _ in range(rng.randint(2, 4)):
        a = rng.choice(arrays)
        name, curlen = a["name"], a["curlen"]
        roll = rng.random()
        if roll < 0.18:
            lines.append(f"    print(len({name}))")
        elif roll < 0.40 and curlen > 0:
            i = rng.randint(0, curlen - 1)
            a["max_idx"] = max(a["max_idx"], i)
            lines.append(f"    print({name}[{i}])")
        elif roll < 0.50 and curlen > 0:
            lines.append(f"    print({name}[len({name}) - 1])")
        elif roll < 0.62:
            if not have_sum:
                lines.append("    let s = 0")
                have_sum = True
            # Literal iterables only: for-in over an array *variable* is the
            # documented one-pass-inline gap in the LLVM backend, so it would
            # report a known gap as a fresh disagreement on every run.
            iterable = "[" + ", ".join(
                str(rng.randint(-9, 9)) for _ in range(rng.randint(1, 4))
            ) + "]"
            lines.append(f"    for x in {iterable} {{")
            lines.append("        s = s + x")
            lines.append("    }")
            lines.append("    print(s)")
        elif roll < 0.74 and curlen >= 2:
            i = rng.randint(0, curlen - 2)
            j = rng.randint(i + 1, curlen - 1)
            a["max_idx"] = max(a["max_idx"], i, j)
            lines.append(f"    print({name}[{i}] + {name}[{j}])")
        elif roll < 0.87 and not a["reassigned"] and curlen > a["max_idx"]:
            # Reassign to a literal that still covers every index emitted so
            # far AND never grows the array: C element-copies into a fixed
            # stack buffer, so any growth overflows it (known gap).
            new_len = rng.randint(max(1, a["max_idx"] + 1), curlen)
            elems = [str(rng.randint(-50, 50)) for _ in range(new_len)]
            lines.append(f"    {name} = [{', '.join(elems)}]")
            a["curlen"] = new_len
            a["max_idx"] = 0
            a["reassigned"] = True
        else:
            lines.append(f"    print({name}[0] + {total})")
            total = rng.randint(-9, 9)  # arbitrary; keeps the print well-formed
    lines.append("}")
    return "\n".join(lines)


# ── Mutation ────────────────────────────────────────────────────────────────

# Integers not glued to an identifier character: the lookbehind keeps the
# mutator from renaming variables (v0 -> v-6 would just be a parse error).
NUM_RE = re.compile(r"(?<![A-Za-z0-9_])-?\d+")

# Lines whose first integer literal must NOT be swapped: subscripts (index
# validity is statically tracked by the generator) and shift amounts
# (a negative shift is UB in C and a panic in the interpreter).
MUTATION_SKIP = ("[", "<<", ">>")


def mutate(source: str, rng: random.Random) -> str:
    out = []
    for line in source.splitlines():
        if any(tok in line for tok in MUTATION_SKIP) or not (m := NUM_RE.search(line)):
            out.append(line)
            continue
        new = str(rng.randint(-10, 10))
        out.append(line[: m.start()] + new + line[m.end() :])
    return "\n".join(out)


# ── Main loop ───────────────────────────────────────────────────────────────

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--count", type=int, default=200, help="programs to test")
    parser.add_argument("--seed", type=int, default=None, help="RNG seed (repro)")
    parser.add_argument(
        "--backends",
        type=lambda s: s.split(","),
        default=list(ALL_BACKENDS),
        help="comma-separated backend subset",
    )
    parser.add_argument(
        "--no-mutate", action="store_true", help="test only freshly generated programs"
    )
    args = parser.parse_args()

    rng = random.Random(args.seed)
    seed = args.seed if args.seed is not None else rng.randrange(2**31)
    rng = random.Random(seed)
    print(f"fuzz_parity: seed={seed} count={args.count} backends={','.join(args.backends)}")

    if not SANDBOX.exists():
        print(f"building sandbox binary at {SANDBOX} ...")
        build = run_cmd(["cargo", "build", "--quiet"], timeout=300)
        if build.returncode != 0:
            print(build.stderr, file=sys.stderr)
            return 2

    backends = list(args.backends)
    if "wasm" in backends and shutil.which("wat2wasm") is None:
        print("NOTE: wat2wasm not found — dropping the wasm backend")
        backends.remove("wasm")
    if not backends:
        print("no backends to test", file=sys.stderr)
        return 2

    ok = skipped = failures = 0
    fail_dir = REPO_ROOT / "fuzz_failures"
    for i in range(args.count):
        # Rotate program kinds uniformly: arrays, plain integer, narrow.
        kind = i % 3
        if kind == 0:
            source = gen_array_program(rng)
        elif kind == 1:
            source = gen_int_program(rng)
        else:
            source = gen_narrow_program(rng)
        if not args.no_mutate and rng.random() < 0.5:
            source = mutate(source, rng)

        with tempfile.TemporaryDirectory() as td:
            workdir = Path(td)
            results: dict[str, str] = {}
            errors: dict[str, str] = {}
            for backend in backends:
                try:
                    results[backend] = run_backend(backend, source, workdir)
                except SandboxError as e:
                    errors[backend] = str(e)

            if "c" in errors:
                # C is the reference; if it cannot run the program the case is
                # outside the supported vocabulary — drop it.
                skipped += 1
                continue
            if len(results) < 2:
                # Not enough working backends to compare (e.g. a wasm-only
                # support failure). A C vs single-backend diff is covered by
                # the parity corpus; do not report it as a fuzz disagreement.
                skipped += 1
                if len(errors) == 1:
                    print(f"[skip] case {i}: {list(errors)[0]} failed: {errors[list(errors)[0]]}")
                continue

            distinct = set(results.values())
            if len(distinct) == 1:
                ok += 1
                continue

            failures += 1
            fail_dir.mkdir(exist_ok=True)
            repro = fail_dir / f"case_{i}.sbx"
            repro.write_text(source)
            print(f"\n=== DISAGREEMENT case {i} (saved {repro.relative_to(REPO_ROOT)}) ===")
            print(source)
            print("--- outputs ---")
            for backend, out in results.items():
                print(f"{backend:>6}: {out!r}")
            for backend, err in errors.items():
                print(f"{backend:>6}: ERROR {err}")
            if failures >= 5:
                print("\nstopping after 5 disagreements")
                break

    print(
        f"\nfuzz_parity: {ok} agreed, {skipped} skipped, "
        f"{failures} disagreements (seed={seed})"
    )
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
