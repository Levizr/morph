# The Story of Morph

This is my space — the story of why Morph exists, the choices I made, and the answers to the questions people actually ask. Not a spec, not a roadmap. Just the story.

> I'm [PIYUSH](https://github.com/Piyushthelagend) — the original author. Everything here is written in my own words, the way I remember it. Corrections and questions welcome at [suggestions.morph@levizr.com](mailto:suggestions.morph@levizr.com).

## How it started

### The real reason: a video editor

Morph didn't start as a framework project. It started as **a video editor** — an app that would let users create 2D animations, with basic and advanced tools. And honestly, I wanted it for myself too: Linux doesn't have a good video editor that matches what I expect. So I thought — why not build my own?

A video editor needs a GUI. That one requirement sent me down a rabbit hole I never came back from.

### The search: why is there nothing?

I looked at the GUI frameworks everyone uses, and found there was **nothing** that fit:

- **Electron** — it puts a whole browser inside your app. It eats RAM and adds enormous size. On a basic PC like mine, I can't afford that — and even my internet connection is slow and limited, so downloading hundreds of megabytes for every app is not an option. On top of that, communicating with native things is painful — separate binaries, processes, bridges. It's a browser wearing an app's clothes.
- **Qt** — powerful, but the learning curve is steep, even a little change means recompiling, and I could never fully understand the commercial licensing situation. I don't want to build on something I can't legally reason about.
- **Others** — imgui and the simpler toolkits are nice, but they can't fulfill what I wanted: a modern, component-based, fast UI with the DX of the web.

Nothing. Zero. Every option was a compromise that broke one of the three things I needed: **small, fast, and easy to build with**.

### "Then I'll learn graphics APIs — like Adobe did"

So I thought: fine, no framework can do this — I'll learn graphics APIs and create it from scratch, the way Adobe builds its own tooling. No compromise, everything designed for exactly what I need.

I started learning **OpenGL**. And somewhere in that learning, the idea clicked:

> If it's not there — then why not create my own *framework*?

Not just the video editor's renderer — the whole framework. The thing I was searching for, built by me. That's the exact moment Morph was born: not when I started writing code, but when I stopped looking for a framework and started planning one.

### "Why not build my own?"

And from here — why not build my own? A clean slate. I know how this sounds. A solo dev, one person, building a whole GUI framework — people look at that like it's crazy. *"A solo dev, a huge project, a GUI framework… wow."*

I thought: let's do it. I'm not afraid of learning new things, and I'd rather put my hands on something people call hard and give up on. And there's a feeling in that — you know the one. I can't explain it in words. You could call it a dopamine spike. It's why I'm here.

### The journey starts

I researched available frameworks properly — *why is there no single framework that fulfills a developer's wishes?* — and I made a plan, built for my own benefit first.

Why open source? Because this is a huge project. Maybe someone can contribute. (Still no contributors other than me — but the door is open.)

I contacted people about it. They said: *"It's huge."*

Yes. That's the point.

### Why this page exists

I'm writing this story and the future docs for a very practical reason: **when I tell people about Morph, their suggestions are almost always things already in my plan or my decisions.** I hear the same ideas over and over — things I've already thought about, already decided, or already documented.

So I wrote it all down. Everything I've decided, everything I'm planning, everything I've already considered. So that when you give me a suggestion, it can be **beyond this** — something I haven't thought of. The roadmap is my defense against the same suggestion twice; this story is the context that makes it useful.

If you read this and think *"but what about…"* — and it's not in the [roadmap](../future/index.md) — then you're exactly the person I want to hear from.

## The big decisions

The story is really a series of choices. Each has its own page:

| Decision | One line | Full story |
|---|---|---|
| The name | A real word that explains what it does — from a Minecraft mod | [Why Morph](why-morph.md) |
| The company | Levizr is mine — a name nobody was using, registered 1 Jan 2025 | [Why Levizr](why-levizr.md) |
| Toolchain language | Python was the fastest way to prove the idea — and it never ships | [Why Python first](why-python.md) |
| Runtime language | C++ is the smallest, most natural target for compiled JS — and keeps the 1 MB promise | [Why C++ first](why-cpp.md) |
| Rendering | OpenGL 3.3 is easy, cross-platform, and runs on the oldest hardware | [Why OpenGL](why-opengl.md) |
| File format | `.mx` signals "this is Morph, not React" — and TS compiles cleanly to C++ | [Why .mx](why-mx.md) |
| UI syntax | JSX is the best DX I've used — and you already know web syntax | [Why JSX](why-jsx.md) |
| The bugs | Yes, Morph is buggy — here's why, and how you can help | [Why Morph Is Buggy](why-buggy.md) |

## Currently building

| What | Status | Full plan |
|------|--------|-----------|
| morphc — the Rust rewrite | In progress | [Working on morphc](working-on-morphc.md) |

The Rust rewrite replaces the Python toolchain with a single native binary. Updated as work progresses.

## Questions people ask

The most common questions live in the [FAQ](../faq/index.md) — each answered in detail on its own page:

| Question | Page |
|---|---|
| Is Morph just Electron with extra steps? | [Q: Electron?](../faq/q-electron.md) |
| Do I need to know C++? | [Q: Do I need C++?](../faq/q-cpp.md) |
| Is it React? Can I use React? | [Q: Is it React?](../faq/q-react.md) |
| Does Morph use a virtual DOM like React? | [Q: Virtual DOM?](../faq/q-virtual-dom.md) |
| Is it for games or for apps? | [Q: Games or apps?](../faq/q-games.md) |
| What about the web — can I build websites? | [Q: What about the web?](../faq/q-web.md) |
| What happens to my JavaScript at runtime? | [Q: JS at runtime](../faq/q-js-runtime.md) |
| Can I use npm packages? | [Q: npm packages](../faq/q-npm.md) |
| How does hot reload work if everything is compiled? | [Q: Hot reload](../faq/q-hot-reload.md) |
| Is it production-ready? | [Q: Production-ready](../faq/q-production.md) |

## Ask me anything

This section is meant to grow. If you have a question that isn't answered here — about the architecture, a decision, or "why didn't you just use X" — ask it at [suggestions.morph@levizr.com](mailto:suggestions.morph@levizr.com) or in a repo issue. The best questions get their own page in the [FAQ](../faq/index.md), answered in detail.