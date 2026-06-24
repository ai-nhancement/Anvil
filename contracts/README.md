# Local-model coder contracts — a per-capability library

A coder contract is **scaffolding**, and the right amount of it scales **inversely with
model capability**. A weak model falls without structure; a strong model trips over it.
This was measured, reproduced, and mechanism-explained on the dialect bench (`anvil bench`,
gemma4 family, n=10 per cell), not assumed.

## The crossover (why there must be more than one contract)

Same two contracts, two models — the winners are **opposite**:

| contract | gemma4:e2b (2B) | gemma4:e4b (4B) |
|---|---|---|
| **full** (base + clauses) | **~68/70** ✅ | 63/70 |
| **base** (minimal) | 43/70 | **69/70** ✅ |

- The 2B *needs* the clauses: without ACT it emits tool calls as text (`add-file` 0→10);
  without VERIFY it ships unverified (`fix-failing-test` 2→10).
- The 4B is *hurt* by them: the same clauses drop it 69→63 (it over-deliberates / performs
  the process instead of doing it). Even a single, well-aimed extra line cost it −2.

## The tiers

- **`coder_local_base_v4.md` — THE BASE (≥4B / capable tier).** Role + the balanced
  edit_file/write_file line, zero clauses. Optimal for capable local models. This is also
  the **foundation we build everything else from.**
- **`coder_local_base.md` — full (~2B tier).** The base + ACT/VERIFY/TRUTH/PERSISTENCE
  clauses, which a small model needs to act and verify reliably.

## How to grow a tier (the method)

Start from the base. Add **one** clause/line at a time and **bench it** — keep it only if
the number goes up *for that model*. Over-bounding is the failure mode: every line is a
constraint, so each must pay for itself. Don't reason about what "should" help; measure it.

(`coder_local_base_v2.md`, `_v3.md`, `_v5.md` are retained iteration artifacts — see the
session log for what each tested. v5 = base + a precision line that measurably cost −2 on e4b.)
