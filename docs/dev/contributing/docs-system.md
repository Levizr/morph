# Docs System

**Part of:** [Dev Docs](../architecture/overview.md)

How documentation flows from the morph repo to `morph.levizr.com`, and how to add or fix a page on either track. Yes, the docs system is documented in the docs — someone has to maintain it, and that someone is whoever touches it last. Possibly you. Welcome.

## The pipeline

```
morph repo                          site repo (morph.levizr.com)       browser
──────────                          ────────────────────────────       ───────
docs/**/*.md ──┐
               ├─► raw.githubusercontent.com ──► /docs/[...slug] ──► /docs/…
docs/docs.registry.json ─┘   (nav: sections, titles, SEO, dates)
docs/dev/<category>/*.md ──┐
               ├─► raw.githubusercontent.com ──► /dev/docs/[...slug] ──► /dev/docs/…
docs/dev.registry.json ─┘
```

The site fetches from GitHub `main` — there is no build-time copy. Pushing to `main` updates the site (a purge workflow clears the fetch cache per push). Dev pages live one directory per category (`architecture/`, `crates/`, `morpher/`, `state/`, `runtime/`, `build-cli/`, `testing/`, `contributing/`, `bugs/`) mirroring the registry categories. Both tracks share the registry schema: `{ title, slug, file, status, author, description, keywords, lastUpdated, publishedAt, priority, changefreq }`.

## The two tracks

| | User docs (`/docs`) | Dev docs (`/dev/docs`) |
|---|---|---|
| Source | `docs/` | `docs/dev/<category>/` |
| Registry | `docs/docs.registry.json` | `docs/dev.registry.json` |
| Audience | People building apps | Contributors + the curious |
| Promises | Yes — changes need migration notes | No — internals can change freely |
| URL | `/docs/<slug>` | `/dev/docs/<slug>` |

## Adding a page

1. Write the `.md` in `docs/` (user) or in the matching `docs/dev/<category>/` directory (internals — every dev page lives in its category dir, never flat in `docs/dev/`).
2. Register it in the matching registry file: `slug` is the URL path, `file` is the repo path — they don't have to match, but keep them mirrored (`slug: architecture/foo` ↔ `file: dev/architecture/foo`) unless you enjoy confusing the next editor. New category? Add the directory *and* the registry category together — one without the other is a page the site can't find or a nav entry pointing at air. Bump `lastUpdated` on any page you touch.
3. Link rules: `page.md` → same category; `../<category>/<page>.md` → another dev category (e.g. `../runtime/networking.md`); `../../guides/x.md` from a dev page → `/docs/guides/x` automatically; `../../../CONTRIBUTING.md` (escaping the docs root) → GitHub blob link automatically. Anchors (`#section`) survive all of these — use them.
4. Validate before pushing:
   ```bash
   python3 -c "import json; json.load(open('docs/dev.registry.json'))"
   ```
   Registry must stay valid JSON, every `file` must resolve to a real `.md`, and every relative link must land somewhere. Then push — the site picks it up from `main`.

## Moving or renaming a page

Moving a file means touching four things, and forgetting any one of them breaks something silently:

1. The file itself (`git mv` keeps history — use it for tracked files).
2. Its `file` field in the registry.
3. Every link pointing at it (grep the whole `docs/` tree — `future/` and `guides/` link into dev pages too).
4. Its `slug`, if you want the URL to follow (remember: slug changes break published URLs; `file` changes don't).

## Styles

- User docs: second person, task-oriented, honest about gaps, email CTA for open ideas (`suggestions.morph@levizr.com`).
- Dev docs: file references required, decision flows over prose, concepts before mechanics, one worked example minimum, a "where to cut" table, and "verify by" steps for anything load-bearing.
- Neither track uses emojis in prose. Code blocks carry the examples, not adjectives. Funny is welcome; unclear is not — if the joke obscures the mechanism, cut the joke.
