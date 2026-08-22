# Why Morph Is Buggy

**Part of:** [The Story of Morph](index.md)

If you feel the framework is buggy — you're right. And I'm not going to pretend otherwise.

I went through my own source code, hunting, and came back embarrassed. The bugs are real, they are many, and some of them are ugly. Here they are, in public, because pretending otherwise would be worse than any bug.

## Layout is not perfect

- Every flex child sits at **double its left/top margin** — placement adds it, then layout adds it again, so rows drift sideways line by line
- With `flex-grow` on the children, **centering shifts the whole row** — the placement math reads a stale total and the row lands off-center
- Lines that should **wrap don't** — the wrap test forgets the gap, the line overflows by exactly one gap, and shrink silently squeezes your children instead
- `margin: auto` centers against a size that gets clamped *after* centering — a card with `max-width` sits visibly off-center
- Column-flex children with horizontal margins render **narrower than their content**, so text wraps early or clips

## Text breaks in fun ways

- Text **ignores its own padding** — first glyph inside the gutter, last word past the right edge
- Font inheritance reaches **one level deep**: a grandparent's `font-size: 32px` never reaches a grandchild through an unstyled div, and an explicit `font-size: 16px` is indistinguishable from unset
- Emoji measure with the text font but draw with the emoji font — so a centered label with an emoji sits slightly off-center, and wraps at the wrong spot

## Hot reload sometimes doesn't

- Save while a reload is already running and **your change is dropped** — nothing re-queues it. It applies on your next save. That "I saved and nothing happened" feeling? Real.
- Save several files within the same instant and some of them **never get compiled**
- Two CSS imports styling the same selector? Only the **later import's rules survive** — the earlier ones are silently replaced instead of merged

## The cascade has holes

- `h2 + p` matches like `h2 p` — sibling selectors style descendants they shouldn't
- Specificity ignores `:hover`, so hover rules can lose to rules they should beat
- `<img width="400">` **beats your CSS classes** — attributes override stylesheets here, the opposite of the browser

## Your JS hits sharp edges

- A string containing a quote character emits invalid C++ — the reload fails to compile
- Multi-line template literals embed raw newlines into string literals — compile error
- A closure returning from a function captures its local **by reference** — undefined behavior after return

## DevTools can get in your way

- On small windows the docked panel isn't fully clamped — it can **cover your app's content**, exactly what docking was supposed to prevent
- Inspect mode sees *through* the panel — elements hidden underneath still highlight
- Very short windows can produce NaN scrollbar math — the logs scrollbar jumps or vanishes

## And under the hood, worse

- In forge mode, damage rects ignore ancestor scroll offsets — scrolled lists leave **ghost bands**; the previous-frame cache survives hot reloads, so recycled nodes can render blank until they move
- Conditional rendering leaks both branches' nodes on every save — apps using `{cond ? a : b}` slowly eat memory
- Signals are read across threads without synchronization where `fetch()` resumes — rare torn reads, lost effect wakeups
- Nested scroll containers swallow wheel events even at their limit, so inner lists won't scroll

## Why

Because this is *too complex* for one pair of eyes. Hundreds of CSS properties interacting, an entire JS language surface compiling to C++, two renderers, a compiler pipeline. Thousands of places where a single wrong sign flips everything. I cannot find each one of them. I can just try.

## One person

Here's the thing: **I'm a kid.** Seventeen years old. I built every line of this alone — the parser, the compiler, the renderer, the layout engine you just watched fail. People call that impressive. Some days I believe them. Most days I look at a flexbox line drifting sideways because of a double-added margin and feel exactly like what I am: one person who can't do everything.

I fix bugs every day. But the ones that go unnoticed by me will stay unfixed forever — simply because I don't know they exist.

## So this is a request

If you are a developer reading this: **help me.** Fix a bug, or even just *find* one and tell me about it — a found bug is half-fixed.

- Email me at [bugs.morph@levizr.com](mailto:bugs.morph@levizr.com)
- Or create an issue on the GitHub repo: [Levizr/morph](https://github.com/Levizr/morph/issues)

Every report makes Morph better for everyone using it.

> A full technical version of this list — with file paths and line numbers — lives in [`help/bug-report-2026-08-23.md`](../../help/bug-report-2026-08-23.md). Heads up: that report was generated with AI help and may contain findings that aren't actually there. Verify before you fix — but if you confirm one, that's a real bug squashed.
