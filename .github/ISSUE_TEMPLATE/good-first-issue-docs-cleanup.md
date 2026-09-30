---
name: "Good first issue: docs cleanup"
description: "Audit docs/future + help/ for outdated claims and sync status headers. No Rust/C++ needed."
title: "docs: remove outdated claims from docs/future and help/ (good first issue)"
labels: ["good first issue", "documentation"]
assignees: []
---

## Good first issue: docs audit — remove outdated `future` / `help` claims

**Difficulty:** Easy · **Area:** docs only, no code · **Time:** ~1–2 hours

Several docs pages still describe the repo as it looked months ago
(pre-Rust rewrite, pre-windows/routing, pre-Kill-Strings). Newcomers hit
contradictions on their first read. This issue is a scoped, mechanical
cleanup: delete or correct the stale bits, keep the design history that
still matters.

You do **not** need to know Rust, C++ or OpenGL for this.

### Concrete examples (start here, don't boil the ocean)

1. **`help/development.md` is stale after the Python → Rust rewrite**
   - File header already says "Historical document. Python was removed
     in September 2026", but the body still instructs:
     `pip install -e ".[dev]"`, `python -m pytest tests/ -v`, and documents
     a `morph/` Python toolchain tree (`cli/main.py`, `parser/`, `style/`, …)
     that no longer exists.
   - `CONTRIBUTING.md` is the current source of truth ("entire toolchain
     is Rust; only Python left is the translator fixture harness under
     `tests/translate/`").
   - **Task:** either delete `help/development.md` (preferred if nothing
     links to it) or replace the body with a 5-line redirect to
     `CONTRIBUTING.md` + `docs/dev/contributing/contributing.md`.
     Check for inbound links first: `rg "help/development" --glob '!target'`.

2. **`docs/future/windows/multi-window.md` says `Status: future`, but it shipped**
   - Page body already admits: "Window control from JSX is implemented
     and documented for users in [Windows & Routes](../guides/windows-and-routing.md)
     and [`Window` / `useWindow`](../api/windows.md)."
   - It also keeps a struck-through stale claim about
     `morph-open` / `morph-close` / `morph-navigate` attributes that
     "never existed … kept only so the history is honest".
   - **Task:** flip `Status:` to reflect reality (or add a
     `Shipped parts:` line like `dynamic-styles.md` does), delete the
     struck-through bullet, keep a one-line "Shipped → main docs" pointer,
     keep the runtime design + open questions.

3. **`docs/future/javascript/native-interop.md` says `Status: development`**
   - `README.md` (§ Current Status) and the API reference (`docs/api/*`)
     list `morphState`, `morphShared`, `morphEvent`, effects and native
     C++ interop as working.
   - **Task:** same treatment — add `Shipped parts:` + pointer to
     `docs/api/*` and `docs/guides/*`, leave only the genuinely
     unbuilt remainder as `future`.

4. **`docs/future/index.md` roadmap table**
   - Good pattern already exists: `window-ownership`, `compiler`,
     `kill-runtime-strings` rows are marked `✅ Shipped` with links to
     `shipped/`. Extend that pattern to the pages fixed above instead of
     inventing a new format.

### Scope (in-bounds / out-of-bounds)

In-bounds:

- `docs/future/*/*.md` status headers (`production` / `beta` / `development` / `future` — see `docs/future/index.md:36`)
- `docs/future/index.md` table rows
- `help/*.md` Python-era leftovers
- `docs/docs.registry.json` + `docs/dev.registry.json` **only if** you move/delete a page (keep slugs/keywords in sync per `AGENTS.md` §4)

Out-of-bounds (do **not** do in this PR):

- Rewriting design records, changing APIs, touching `runtime/` or `crates/`
- Large restructures of `docs/` — one focused PR per page group, small diffs welcome

### Acceptance criteria

- [ ] `help/development.md` no longer instructs `pip install` / Python toolchain (deleted or redirect stub, no inbound broken links)
- [ ] Every touched `docs/future/*.md` has exactly one `**Status:**` line using one of the four canonical values, plus either a `Shipped parts:` line or a `Shipped → main docs` pointer where applicable
- [ ] No struck-through (`~~…~~`) history-only paragraphs remain in touched pages (move genuine history to a 2–3 line `## History` note or delete)
- [ ] `docs/future/index.md` table matches the new statuses (`✅ Shipped` rows point at `shipped/` or main docs)
- [ ] Registries still validate (if you moved/deleted a page): slugs, `file` paths and keywords in sync
- [ ] `cargo test --workspace` untouched (docs-only PR should not break tests); mention docs preview link if available

### How to verify locally

```bash
# 1. find stale references
rg -n "pip install|python -m pytest|morph-open|morph-navigate|Status:" docs/future help --glob '*.md'

# 2. check nothing links to a page you delete
rg -n "help/development|future/multi-window|future/state-events" --glob '!target' --glob '!.git'

# 3. docs-only change: no build needed, but confirm workspace still passes
cargo test --workspace
```

### Tips for first-time contributors

- Read `AGENTS.md` §4 (docs discipline) and `CONTRIBUTING.md` before opening the PR.
- Keep PRs focused — one page group per PR is fine, e.g. "docs: mark multi-window shipped, drop stale morph-actions claim".
- If a page is genuinely half-shipped / half-future (like `dynamic-styles.md`), copy its format: `**Status:** future … **Shipped parts:** …` + `## 1. What Works Today (Shipped)` — that's the house style.
- Open a draft PR early if unsure whether something is "history worth keeping" vs "stale, delete it" — maintainers will guide.
