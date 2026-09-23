# Record — Morph progress screenshots (unedited originals)

This folder is the raw, real journey. Nothing is renamed, re-encoded, or
retouched. Filenames are the original `Screenshot from YYYY-MM-DD HH-MM-SS.png`
so filesystem dates stay as evidence.

- Total: **28 PNGs** (25 in `records/`, 3 in `records/web-video-editor/`)
- `manifest.json` maps every file to its phase, title, and SHA-256.
- Verify anytime: `sha256sum -c` against `manifest.json`, or
  `stat "records/Screenshot from 2026-05-08 11-49-00.png"`.
- Bugs are kept on purpose next to fixes. That is the proof it is real.

## Why these screenshots were saved

Not to show off in the future. Each screenshot was captured to compare
Morph's native render against what the browser renders for the same
HTML/CSS. Side-by-side Chrome-vs-Morph shots (May 25) are the clearest
example: same page, two engines, pixel parity check.

Concrete example: the grey overlay in
`Screenshot from 2026-05-24 00-12-06.png` happened while implementing
CSS `border` + rounded corners (`border-radius`). That is where the CSS
`border-box` sizing implementation landed — the overflow made the bug
visible, the fix proved the box model.

## Why `web-video-editor/` exists

The 3 screenshots from `2026-04-28` are the `studio.levizr.com` web video
editor (`Timeline`, ruler, `Playhead.tsx` logs). Fighting frame accuracy in
the browser is what caused Morph — compile JSX+CSS to native instead.

## Phases (see `manifest.json` for per-file captions)

| Phase | Dates | What it proves |
|---|---|---|
| `00_origin_video_editor` | Apr 28 | Browser editor pain, origin of Morph |
| `01_html2gl_birth` | May 8 | First native window, Test Page, Levizr Engine UI |
| `02_hero_boilerplate` | May 21-22 | `Build beautiful apps seamlessly` bug-to-fix pairs |
| `03_layout_scroll` | May 23-24 | Nav, nesting, scroll viewport, modal/overlay bugs |
| `04_stress_browser_vs_morph` | May 25 | Chrome vs Morph side-by-side parity + one honest green-overlay failure |
| `05_mature_apps` | May 30-Aug 29 | Box debugger, Calculator, Google index, Login lifecycle |

## Sharing with developers

Point them here plus the public gallery at `https://morph.levizr.com/journey`.
Gallery images are optimized copies; originals live only here with hashes.
If anyone says "fake", ask them to check hashes, `stat` mtimes, and the
bug-to-fix pairs (e.g. overlapped buttons → centered fix, green overlay
failure) — fakes do not keep failures.

Extra proof: check the commit history. Commits around each screenshot's
date line up with the bug fixes and features the images show.

## Why screenshots stop in August

Screenshots stop after August for two reasons: documenting moved into
`docs/`, and the layout engine was mostly done. Screenshots existed to
compare Morph's render against the browser — once layout work wound down
and focus shifted to JS-to-C++ logic, states, and window management,
there was nothing left to compare visually, so no more screenshots.
