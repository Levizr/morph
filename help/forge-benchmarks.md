# FORGE — Benchmarks (Phase 9)

> Status: Measured 2026-10-03 (`tests/runtime/bench-forge.sh`, `g++-14 -std=c++23`)
> Scope: Damage-model scenarios + renderer TU sizes + TU-exclusion (`nm`) checks
> Related: `renderer-flash-forge.md` (§10 verification matrix), `hybrid-renderer.md` (estimates)

## Method

Headless and deterministic — no display needed:

- The benchmark drives the **real** `DamageSet` / `TilePool` / `scroll-shift`
  code over synthetic scenes at 1920×1080 and compares against flash, which
  repaints the full frame by construction. Raster cost is proportional to
  repainted area; present cost is the backbuffer blit (`w*h*4`, skipped on
  idle under forge).
- Run: `./tests/runtime/bench-forge.sh` (builds
  `renderers/forge/tests/forge_bench.cpp` + the three pure-CPU forge TUs,
  then the TU-size / `nm` checks).
- Binary sizes below are linked `examples/flash` vs `examples/forge`
  (`morph build --no-upx`, stripped and `MORPH_NO_STRIP=1`).

## Damage model — 1920×1080 (2,073,600 px)

| Workload | flash raster | forge damage | Raster saved | flash present | forge present |
|---|---|---|---|---|---|
| static (idle) | 2,073,600 px | 0 px | — (skip) | 8,294,400 B | **0 B (skipped)** |
| scrub (200×60 widget) | 2,073,600 px | 13,056 px (0.6%) | **158.8×** | 8,294,400 B | 8,294,400 B |
| scroll (800×1000 list, Δ120) | 2,073,600 px | 103,401 px (5.0%) | **20.1×** | 8,294,400 B | 8,294,400 B |
| full-screen anim | 2,073,600 px | 2,073,600 px | 1.0× (parity) | 8,294,400 B | 8,294,400 B |

Identical across 100 / 5,000 / 20,000-node scenes: damage is area-based,
so forge raster work stays flat while flash repaints every node every frame.
Scroll damage = exposed strip (800×120) + old/new scrollbar thumbs; the
scrollbar used to be damaged as a full column until the bounding-union was
found merging it into the whole container (fixed via thumb rects).
Full-screen animation degrades gracefully to flash parity by design
(`>32`-anim guard + geometry-anim fullscreen).

Tile residency across frames: a tile in a region the precise boxes never
touch survives a frame whose merged damage rect covers it (`residency=kept`,
2 tiles / 680,000 B of the 16 MB budget) — invalidation follows precise
changes only, not coarse merged rects.

## Binary size — `examples/flash` vs `examples/forge` (measured)

Both examples build and link (`morph build --no-upx`):

| Binary | Stripped | Unstripped (`MORPH_NO_STRIP=1`) |
|---|---|---|
| `examples/flash` | 145,608 B | 189,400 B |
| `examples/forge` | 288,968 B | 331,224 B |
| Delta | **+143,360 B (+98.5%)** | +141,824 B |

The forge binary carries the full retained backend (damage + tile pool +
scroll-shift + mover layers) at `-O2` while flash builds at `-Oz`; the
delta is backend code, not app code. (Binaries measured before the
`renderer` config wiring shipped contained no forge code at all — see
the production-gap row in `docs/future/rendering/forge-tile-pool.md` —
so older size claims for the forge example are void.)

## TU-exclusion (`nm`) checks

`tests/runtime/bench-forge.sh` verifies the Phase 8 mechanism at object
level (production TU exclusion in `crates/morph-build/src/lib.rs`):

- Flash `window.o` references **no** `forge::` symbols.
- Forge `window.o` references **no** `flash::` symbols.
- `forge.o` defines `forgeCommit` (present in forge builds only).

On linked binaries (`MORPH_NO_STRIP=1`): the flash binary contains zero
forge refs and the forge binary zero `flash::` refs — LTO + `--gc-sections`
fold the chosen backend's small methods inline, so thoroughly that even the
selected backend leaves no named symbols behind. (`damage.o` ships in both
by design: `DamageSet` serves the shared geometry-diff path.)

## RAM model @1080p

| Piece | Cost |
|---|---|
| Retained FBO (RGBA8 + depth/stencil) | ~8.3 MB + ~8 MB |
| Tile pool (hard cap, residency-only) | ≤ 16 MB |
| Layer pool (hard cap) | ≤ 4 MB |
| Hello-world floor | ≈ 30 MB vs ~22 MB flash |

Live RSS is shown in the DevTools Rendering tab (FRAME card, peak RSS via
`getrusage`); tile/layer residency in the FORGE card.
