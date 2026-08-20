# Can I Build Websites with Morph?

**Part of:** [Questions & Answers](index.md) · [The Story of Morph](../story/index.md)

Morph builds **native desktop binaries**. It is not a web framework.

The web already has the best web tooling in existence — React, Next.js, Tailwind, browser DevTools, instant deployment. Morph isn't trying to compete with that, and it would be a bad idea to try.

Morph is for the places where a browser doesn't belong:

- **Desktop apps** — real windows, real menus, real filesystem access, system tray
- **Utilities & tools** — apps that live on the desktop and do one thing well
- **Offline / private** — data stays on the machine, no browser sandbox deciding what you can touch
- **Kiosks & embedded** — a lean native binary where shipping a browser is absurd
- **Performance-critical UIs** — when the app is the product and it has to feel instant

If your product is a website, use web tools. If your product is a *program that runs on someone's computer*, that's Morph's territory.