# App disk size: benchmarks

How much space a Morph app takes on disk — measured, not estimated.
Every Morph number below is a real build on Linux x86-64, file bytes
from `ls -la` (`--static --no-upx` for raw, then UPX `--lzma`).

## The headline

| App | Raw (static, self-contained) | UPX |
|---|---|---|
| hello world (single `h1`) | 397KB | **162KB** |
| components (state, buttons, lists, shared stores, events) | 879KB | 339KB |
| windows (multi-window, ownership, modals) | 891KB | 344KB |
| routes (file routing, mounts, page cache, links, native C++) | 846KB | 327KB |

A full graphical app written in JSX with reactive state ships as a
single zero-dependency file smaller than most web images. A 1080p photo
is typically 200–500KB — Morph's entire software pipeline fits inside
that. Your app is literally smaller than a screenshot *of* your app.
Take that screenshot, by the way: the PNG will probably be bigger than
the binary that rendered it. If your designer sends you a 300KB hero
image, congratulations — the art asset now outweighs the entire program.
Electron needs 95MB to say "hello". Morph says it in 162KB and still
has room for lunch.

## Real-world baseline comparison

Same class of measurement — fully self-contained GUI binaries:

| Framework / Ecosystem | Linkage mode | UPX size | × vs Morph |
|---|---|---|---|
| **Levizr Morph** | Statically compiled C++ | **162KB** | **1× (baseline)** |
| Tauri (v2.x / Rust) | Dynamic OS webview | ~1.5MB | ~9.5× |
| Go / Fyne | Fully self-contained | ~3.8MB | ~23× |
| C++ / Qt6 | Fully self-contained | ~4.2MB | ~26× |
| C# .NET (Native AOT) | Fully self-contained | ~8.0MB | ~49× |
| Flutter (desktop) | Fully self-contained | ~13.5MB | ~83× |
| Python / PyQt6 | Bundled container | ~30MB | ~185× |
| Electron | Bundled Chromium + Node | ~95MB | ~586× |

(Morph numbers measured in-tree; competitor figures are published
industry numbers for equivalent hello-world apps.)

## How size grows with features

The curve above is the answer to "what happens when I add real
features": hello (397KB) → a kitchen-sink app with routing, windows,
state, lists and native code (~850–890KB). Roughly **+470KB raw** from
"nothing" to "everything" — because the binary contains exactly what
the app uses:

- Unused subsystems never compile in (derived `MORPH_FEATURE_*` flags:
  reactivity, tasks, networking, ownership, page cache, HarfBuzz shaping
  — detected from your code, never toggled by hand).
- Unused functions GC out at link (`-ffunction-sections` + LTO).
- No symbols ship (`strip` on every release link).
- Static third-party deps are trimmed at source (gamepad DB, unwind
  tables) and per-requirement (HarfBuzz builds only for shaping content).

Details live in the [lean-binary design record](../future/lean-binary.md)
(the 371KB → 113KB dynamic journey is logged step by step there).

## Dynamic builds (for completeness)

Prefer system libraries over self-containment? Dynamic hello-world is
**113KB raw / 43KB UPX**, plus system GLFW/FreeType/HarfBuzz/X11
(~4MB on disk distribution-wide, shared by every app on the machine).
43KB. Your app now weighs less than this sentence's font file.
Same promise, different tradeoff — see [deployment](deployment.md).

## Reproduce it

```bash
./tests/runtime/check-size.sh   # dynamic hello ≤ 150KB gate
```

Static numbers above come from `morph build --no-upx --static`
followed by `upx --lzma`, measured 2026-09-23. Re-run any time —
if a number regresses, the gate catches the dynamic one and the table
here gets updated with the static ones.
