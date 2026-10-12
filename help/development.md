# Development Guide

> **Redirect** — this page described the pre-rewrite Python toolchain and is now obsolete.
See the current docs instead:

- [`CONTRIBUTING.md`](../CONTRIBUTING.md) — setup, conventions, and the PR workflow
- [`docs/dev/contributing/contributing.md`](../docs/dev/contributing/contributing.md) — where help matters most, and the conventions with their reasoning
- [`docs/dev/architecture/`](../docs/dev/architecture/) — the current pipeline and build-system internals

## History

Morph started as a Python toolchain and was rewritten in Rust in September 2026. The
pip-based install steps, the pytest run commands, and the `morph/` Python layout that
this page used to document no longer apply. The only Python left in the repo is the
translator's fixture-test harness under `tests/translate/`, which drives the built
`morph` binary.
