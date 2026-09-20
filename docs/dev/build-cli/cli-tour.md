# Meet `morph`: Every Command, No Small Talk

**Part of:** [Dev Docs](../architecture/overview.md)

The `morph` binary is the only program in the toolchain a user ever touches. One binary, a dozen verbs, zero Python. It is implemented by the `morphc` crate (`crates/morphc/src/` — yes, the crate is called `morphc`, the binary is called `morph`, and this naming situation is load-bearing history, so just memorize it).

## Layout: one file per verb

The codebase enforces a simple rule: **one command per file** under `crates/morphc/src/commands/`. Finding a command's implementation is therefore a game even a newcomer wins on the first try.

| File | Command | What it does |
|---|---|---|
| `commands/init.rs` | `morph new` | Scaffolds a project: `src/App.mx`, `morph.config.json`, component/CSS folders, assets |
| `commands/install.rs` | `morph install` | Downloads the C++ runtime into the project (via `morph-cache` linkage) |
| `commands/dev.rs` | `morph dev` | Live window + watch + hot reload (see [Dev Mode](../architecture/dev-mode.md)) |
| `commands/build.rs` | `morph build` | Ahead-of-time compile to a standalone binary (see [The Build Machine](../architecture/build-system.md)) |
| `commands/run.rs` | `morph run` | Build-plus-run; `--static` for the single-file variant |
| `commands/check.rs` | `morph check` | Parse + lint without emitting: the fastest way to ask "is my project sane?" |
| `commands/doctor.rs` | `morph doctor` | Verifies the toolchain: Rust, C++ compiler, OpenGL, GLFW, FreeType/HarfBuzz |
| `commands/cache.rs` | `morph cache` | Inspects and prunes the global runtime cache |
| `commands/update.rs` | `morph update` | Reads `versions/morphc/version.json` and upgrades the toolchain |
| `commands/translate.rs` | `morph <file> --to cpp\|rust` | Direct file morphing through the `morpher` crate — no project required |
| `commands/mod.rs` | — | Wiring: subcommand registration and dispatch |
| `main.rs` | — | Dispatch + CLI parsing only. No business logic — the file would file a complaint if you tried |
| `logger.rs` | — | Shared logging and terminal formatting |
| `cache.rs`, `versions.rs` | — | CLI-side cache helpers and version handling |

## Two hats, one binary

`morph` serves two completely different users, and keeping them straight explains half the CLI's shape:

1. **Project mode** (`.mx` apps): `new` → `install` → `dev` → `build`/`run`. The full pipeline from [The Compiler Pipeline](../architecture/compiler-pipeline.md), driven by `morph-config` for project settings.
2. **File mode** (`morph foo.ts --to cpp`): a single file in, C++ (or experimental Rust) out, via `morpher`. No project, no config, no window. Translators, tinkerers, and test harnesses live here.

## Error philosophy

Commands return `Result`-shaped errors and **never panic on user input**. A typo in a filename is not an occasion for a stack trace — it is an occasion for a sentence explaining what was expected and what was received. Library errors use `thiserror`, application-level errors use `anyhow` (see `CODING_STANDARDS.md`), and `unwrap()` in a command path is a bug, not a shortcut.

## Worked example: `morph check`, the unsung hero

`check` runs parse + lint and stops before emission. It is the cheapest possible verification — no C++ compile, no window — and it is what CI and the fixture-skeptical should reach for first. If `check` passes and `build` fails, the problem is in emission or compilation, not in your source. That bisection alone saves hours.

## Where to cut

| "I want to…" | Touch |
|---|---|
| Add a new subcommand | New file in `commands/` + registration in `commands/mod.rs`. Follow the one-file-per-verb rule or face the review comments |
| Change project scaffolding | `commands/init.rs` (and keep `my-app/`-style templates consistent) |
| Change toolchain verification | `commands/doctor.rs` + `morph-build`'s `platform.rs` |
| Change direct-file output | `commands/translate.rs` + the `morpher` crate |
| Change log formatting | `logger.rs` — one place, every command benefits |

## Verify by

```bash
cargo test --workspace
target/debug/morph doctor
target/debug/morph check   # inside any fixture project
```

New commands must not panic on garbage input — feed yours some garbage and confirm it responds with prose, not a panic.
