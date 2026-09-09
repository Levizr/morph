# Docs System

**Part of:** [Dev Docs](overview.md)

How documentation flows from the morph repo to `morph.levizr.com`, and how to add or fix a page on either track. Yes, the docs system is documented in the docs — someone has to maintain it.

## The pipeline

```
morph repo                          site repo (morph.levizr.com)       browser
──────────                          ────────────────────────────       ───────
docs/**/*.md ──┐
               ├─► raw.githubusercontent.com ──► /docs/[...slug] ──► /docs/…
docs/docs.registry.json ─┘   (nav: sections, titles, SEO, dates)
docs/dev/**/*.md ──┐
                   ├─► raw.githubusercontent.com ──► /dev/docs/[...slug] ──► /dev/docs/…
docs/dev.registry.json ─┘
```

The site fetches from GitHub `main` — there is no build-time copy. Pushing to `main` updates the site (a purge workflow clears the fetch cache per push). Both tracks share the registry schema: `{ title, slug, file, status, author, description, keywords, lastUpdated, publishedAt, priority, changefreq }`.

## The two tracks

| | User docs (`/docs`) | Dev docs (`/dev/docs`) |
|---|---|---|
| Source | `docs/` | `docs/dev/` |
| Registry | `docs/docs.registry.json` | `docs/dev.registry.json` |
| Audience | People building apps | Contributors + the curious |
| Promises | Yes — changes need migration notes | No — internals can change freely |
| URL | `/docs/<slug>` | `/dev/docs/<slug>` |

## Adding a page

1. Write the `.md` in `docs/` (user) or `docs/dev/` (internals).
2. Register it in the matching registry file: `slug` is the URL path, `file` is the repo path (for dev docs: slug `foo`, file `dev/foo`). Bump `lastUpdated` on any page you touch.
3. Link rules: `other-page.md` → same track; `../guides/x.md` from a dev page → `/docs/guides/x` automatically; `../../CONTRIBUTING.md` (escaping the docs root) → GitHub blob link automatically.
4. Validate the registry is still JSON (`python3 -c "import json; json.load(open('docs/dev.registry.json'))"`), then push — the site picks it up.

## Styles

- User docs: second person, task-oriented, honest about gaps, email CTA for open ideas (`suggestions.morph@levizr.com`).
- Dev docs: file:line references required, decision flows over prose, "verify by" steps for anything load-bearing (fixtures, harnesses, screenshot diffs).
- Neither track uses emojis in prose. Code blocks carry the examples, not adjectives.
