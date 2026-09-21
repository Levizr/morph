# AGENTS.md — Instructions for AI Coding Agents

> **MANDATORY: follow the coding standards at [`./CODING_STANDARDS.md`](./CODING_STANDARDS.md) for every change in this repo. No exceptions. Run the formatters/linters below before every commit.**

## 1. Before You Write Code

1. Read [`./CODING_STANDARDS.md`](./CODING_STANDARDS.md) — Rust (§2) and C++ (§3) naming, formatting, error handling, and the review checklists (§6) are binding.
2. Match surrounding code. Consistency beats preference.
3. Rust: no `unwrap()`/`expect()` in production paths (`anyhow`/`thiserror`), grouped imports, `cargo fmt` clean.
4. C++ (`runtime/cpp`): Allman braces, `m_` member prefix, `#pragma once`, `inline constexpr` over `#define`, no raw `new`/`delete` in new code, feature code inside `#ifdef MORPH_FEATURE_*`.

## 2. Before You Commit

```bash
cargo fmt --all
cargo test --workspace
```

- Rust must also pass `cargo clippy --all-targets --all-features -- -D warnings` (allow-list in workspace `Cargo.toml`).
- C++ changes: `clang-format` clean per `.clang-format`; every fixture rebuild must compile (the C++ compiler is the linter — see §3).

## 3. Verification Is Mandatory

- Never claim a fix works without executing it: rebuild affected fixtures and run them.
- Headless runtime check (no display needed): `<binary> --morph-self-test` must report `0 failures`.
- Full fixture sweep: `./tests/runtime/run-selftests.sh` from the repo root.
- Fixture projects live in `tests/runtime/<name>`; example apps in `examples/`. Rebuild with `<repo>/target/debug/morph build --no-upx` from the project dir (delete a stale binary first — fingerprinting skips recompiles when only the compiler changed).
- A live X server is usually on `:0` — GUI apps can be screenshotted via `import -window <id>` for visual verification.

## 4. Docs Discipline

- User docs: `docs/` (registry: `docs/docs.registry.json`). Dev internals: `docs/dev/` (registry: `docs/dev.registry.json`). Future design: `docs/future/`.
- Keep registries in sync when adding/moving pages (slugs, keywords, descriptions).
- Design-record pages (`docs/future/`) are source of truth — update status tables and decision logs when implementation lands.

## 5. Git

- Inspect `git status` / `git diff` before committing; stage only intended files; never commit secrets or build output (`.morph/`, binaries).
- Concise commit messages matching repo style (`feat:`, `fix:`, `docs:`, `style:`, `test:`).
- Do not push unless asked.
