# Full Node.js Support

**Status:** future · **Priority:** medium · **Depends on:** [JS Coverage](js-coverage.md)

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

The idea in one line: **anything that runs on Node.js should run on Morph** — `import ... from "morph/fs"`, `npm install` anything, even entire servers — compiled to native C++, with no Node process shipped. The `node:*` spelling (`node:fs`, `node:path`, ...) keeps working as an alias, but in `.mx` files `morph/*` is the preferred style.

## Where this idea comes from

The everyday JS surface already compiles well — variables, functions, classes, arrays, strings, `fetch`, timers, promises, `async`/`await` all lower to native code today (see [JS Coverage](js-coverage.md) for the running catalog). That baseline working so well raised an obvious question: if UI logic compiles cleanly, why should server-style code be any different?

Reading through the Node.js docs, the answer started looking encouraging rather than crazy. Node's built-in surface (`node:fs`, `node:path`, `node:http`, ...) is a finite, documented API list — and each entry maps to a C++ equivalent the same way `fetch` and `setTimeout` already do. So this stopped looking impossible and started looking like a (large, honest) list of modules to implement, one at a time, behind the morpher we already have.

## Why Node.js, not Bun?

Fair question — Bun is fast and modern. Three reasons we target Node's surface, not Bun's:

- **Node is production grade.** Years of LTS discipline, a stable documented surface, behavior people bet businesses on. That's the foundation you want under a compat promise.
- **Almost every npm package is written for Node.** The ecosystem's assumptions — module resolution, builtin names, documented behaviors — are Node's assumptions. Targeting Node buys the whole ecosystem; targeting anything else buys a subset.
- **Bun supports Node code too.** Bun itself chases Node compatibility, which kind of proves the point: `node:*` is the standard. Implement Node's surface and Bun-oriented code largely comes along for free — the reverse wouldn't be true.

One clarification, because "not Bun" is easy to misread as "slower": it's the opposite. We're borrowing Node's *API surface*, not its engine. Bun is fast for a runtime that still runs JavaScript; Morph runs no JavaScript at all at runtime — no interpreter, no JIT, no VM. Your code is compiled ahead of time to optimized native C++ (with Rust where it wins), so what executes is machine code with the JS as surface only. The expectation is multiples of Bun's performance, not parity with it.

## What it would look like

### Morph imports first, Node imports honored

In `.mx` files you import from `morph/*` — the preferred Morph style:

```ts
import { readFile } from "morph/fs";
import path from "morph/path";

const text = await readFile(path.join("data", "notes.txt"), "utf8");
```

Same APIs as Node, no wrapper to learn — if you know Node, you already know this surface. And the Node spelling keeps working as an alias:

```ts
import { readFile } from "node:fs/promises"; // same module as morph/fs
```

`node:*` exists so Node code pastes in unchanged — and because npm packages import `node:*` internally, morpher has to resolve both spellings regardless. Prefer `morph/*` in code you write; `node:*` is the compat path.

### npm packages that work with Node work with Morph

```bash
npm install date-fns
```

```ts
import { format } from "date-fns";

format(new Date(), "'Today is a' eeee");
```

Packages resolve the normal way and get compiled at build time through morpher — the [Package Build Bridge](packages.md) mechanism, pointed at the npm registry instead of a Morph-only registry. Pure-JS packages just work; packages with native addons (node-gyp, prebuilt `.node` binaries) don't — the same boundary every non-Node runtime draws.

### Entire servers, compiled to native C++

```ts
import http from "morph/http"; // node:http works too

const server = http.createServer((req, res) => {
  res.writeHead(200, { "content-type": "text/plain" });
  res.end("hello from native code");
});

server.listen(3000);
```

`morph build` compiles it to a native binary — no V8, no Node process, no bundled runtime. Your server logic becomes machine code with the same coroutine scheduler your UI already runs on.

## How it could work

- **One C++ module per module, two spellings** — the same pattern as today: `fetch` lowers to coroutine HTTP, `setTimeout` lowers to the scheduler. `morph/fs` lowers to `std::filesystem`, `morph/path` to small pure functions, `morph/http` to a socket layer — and `node:fs`, `node:path`, `node:http` resolve to those same C++ modules as aliases. Morpher grows a module table (canonical `morph/*` entry plus generated `node:*` aliases); nothing about the architecture changes.
- **The event loop is already here.** Promises and timers already compile to the coroutine scheduler (`morph::Result<T>`, `co_await`). A server is that same scheduler with sockets attached and a loop that doesn't exit — new I/O surface, not a new execution model.
- **npm via the build bridge.** Resolve at build time, feed package JS through morpher, compile it in. A compat list tracks which packages are pure-JS and verified; anything else fails at build time with a clear error, not silently at runtime.
- **What stays out:** native addons, `node:worker_threads` semantics that assume V8 isolates, and anything that needs an actual JS engine at runtime. The line is "compilable to C++", and `morph check` already enforces lines like that (`mx-js-*` diagnostics).

## Documented behavior only — no Hyrum's Law

One rule, stated upfront: morpher implements **what the Node.js documentation says**, nothing more. [Hyrum's Law](https://www.hyrumslaw.com/) observes that with enough users, every observable behavior of a system gets depended on — error message strings, timing quirks, undocumented edge cases. We are explicitly not signing up for that.

The reason is architectural: we are not porting Node, we are **rewriting** each module natively for maximum performance. `morph/fs` is `std::filesystem` behind a Node-shaped API, not libuv with its exact scheduling quirks; `morph/http` is a socket layer, not a byte-for-byte port of Node's parser. Same documented inputs and outputs, different insides — so anything undocumented (exact error texts, timing, internal ordering) *will* differ, and that is by design, not a bug.

What this means in practice:

- If the Node docs promise it, it works — that's the compat contract, and the compat list tests exactly that.
- If it's observable but undocumented and your package depends on it, that package is outside the contract. File an issue and we may cover it deliberately — but "Node does X if you squint" is never a bug report that auto-wins.
- This is also why compat is verified per package version: documented behavior is stable, quirks aren't, so the list pins to what the docs guarantee.

## Current state

| Piece | State |
|---|---|
| Everyday JS → C++ (the foundation) | ✅ Shipped and expanding ([JS Coverage](js-coverage.md)) |
| Async core (`fetch`, timers, promises → coroutines) | ✅ Shipped |
| `morph/*` + `node:*` module surface | ❌ Not started |
| npm resolution at build time | ❌ Not started (the bridge itself is unbuilt — see [Packages](packages.md)) |
| Server-style loop semantics (sockets, listen/accept, long-lived loop) | ⚠️ Scheduler exists; server I/O unproven |

## Open questions

- **Which modules first?** `morph/path` (pure functions, no I/O, with its `node:path` alias) is the obvious beachhead — it proves the import path end to end. Then `morph/fs`, then `process`/`process.env`/argv (servers aren't real without env and args), then `morph/http` as the milestone that proves "entire servers".
- **Which Node version do we track?** The intent is to follow the latest Node release, so new APIs arrive as Node ships them — but that's not confirmed yet. Either way the target gets pinned explicitly (e.g. "Node 22 surface") because Node's API moves and the module list has to stay finite.
- **Native addons:** reject at install with a clear error, or a curated allowlist of re-implementable ones?
- **Does this change the "no Node runtime" story?** Yes, deliberately — Morph would become a Node-*compatible* compiler, not just a UI compiler. The "no V8, no interpreter" claim stays; "no Node APIs" goes.
- **UI-first or server-first?** Servers are the flashier milestone, but `node:path`/`node:fs` pay off inside UI apps (config files, local data) long before the first server ships.

## Build steps (when picked up)

1. `morph/path` (+ `node:path` alias) — pure functions, proves the import path end to end through morpher
2. `morph/fs` (sync + `fs/promises` shape) — file I/O over `std::filesystem`
3. `process` / `process.env` / argv — the minimum for real CLI and server programs
4. `morph/http` server — the "entire servers in native C++" milestone
5. npm resolution via the package bridge + a pure-JS compat list, with clear build-time errors for the rest
