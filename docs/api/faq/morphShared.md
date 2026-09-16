# morphShared FAQ

## Why morphShared instead of React-style context providers?

React context has three costs that `morphShared` refuses to pay — and the design is inspired by how SolidJS and Svelte already solved them with signals and external stores.

**1. Provider hell.** In React, sharing state means wrapping the tree:

```tsx
// React: every shared value needs a provider mounted above its users
<ThemeProvider>
  <AuthProvider>
    <CartProvider>
      <App />   {/* buried three levels deep before it renders anything */}
    </CartProvider>
  </AuthProvider>
</ThemeProvider>
```

Each provider is a component, a file, a context object, and a hook (`useContext(ThemeContext)`). Add a fourth shared value, touch the tree again. In Morph there are no providers at all — a component imports the store directly, like any other module import. The component tree stays a tree of UI, not a tree of state plumbing.

**2. Blunt re-renders.** When a React context value changes, *every* consumer of that provider re-renders — even ones using an unrelated slice of the value. And in practice it gets worse: creating a proper provider per value is tedious, so lazy codebases converge on one giant `AppContext` holding theme, user, cart, settings — everything. Now a header that only reads the theme re-renders when the cart changes, because the provider force-feeds *all* of its data to *every* consumer on *any* change:

```tsx
// React: one lazy provider for unrelated values
<AppContext.Provider value={{ theme, user, cart, settings }}>
```

```tsx
// This header only wants theme — but cart updates re-render it anyway,
// because it subscribed to the whole bucket, not the slice it reads.
const { theme } = useContext(AppContext)
```

That is a performance problem that grows with the app: the more values stuffed into shared providers, the more components re-render for changes they never use. The disciplined fix — one provider per value — just recreates provider hell from point 1, so teams rarely do it. The undisciplined reality is one junk-drawer context and mystery re-renders.

`morphShared` makes the disciplined shape the *easy* shape: each store is already its own one-line "provider", so splitting costs nothing and nobody reaches for the junk drawer. And subscription is signal-grained — only components that actually read the getter re-render, because reading subscribes that component and nothing else. There is no "all consumers update" step to optimize away, and no shared bucket to get force-fed from.

In complexity terms: React context updates are **O(N)** — N being every consumer under the provider, so each new component attached to the tree makes every future update more expensive, whether that component cares about the value or not. Morph shared updates are **O(1) in the size of the app** — the cost depends only on the readers of that one store and never grows as you add components elsewhere. A thousand new components that don't read the store add exactly zero update cost. That is what fine-grained reactivity buys you: updates scale with interest, not with tree size.

**3. Indirection.** Context separates three things across three places: the context object, the provider that sets the value, and the hook call that reads it. To answer "where does this value come from?" you chase all three. A shared binding is one line in one file, imported by name:

```tsx
// themeStore.ts — the whole "context": declaration, value, and API
export const [theme, setTheme] = morphShared<'light' | 'dark'>('light')
```

```tsx
// any component — the whole "consumer": one import
import { theme, setTheme } from './themeStore'
```

**The inspiration.** This is not a new invention — it follows SolidJS and Svelte, which both moved state out of the component tree: Solid with signals and stores that live outside components and update with fine-grained precision (no VDOM, no provider re-render cascades), Svelte with `writable` stores and module-level state you import and subscribe to directly. `morphShared` takes that same lesson — state as an importable reactive value, not as tree plumbing — and pushes it one step further: instead of a store *contract* (`writable()`, `$subscription`), the TypeScript module system itself is the store contract. Export a binding, import it anywhere. No provider components, no subscription syntax, no context objects.

If you come from React: wherever you would create a context + provider + `useContext` trio, write one `export const [x, setX] = morphShared(...)` line instead. Everything else — reading, writing, re-rendering — behaves like state, because it *is* state, just with app-wide identity.

## Why a separate `morphShared` name? Why not just `morphState` at the top of the file?

Because the name states your intent — to yourself, to your team, and to the compiler. `morphState` means "mine, per instance". `morphShared` means "everyone's, one value". If top-level `morphState` were allowed, every module-scope declaration would be ambiguous: shared on purpose, or a local that someone forgot to put inside a component?

With two names there is no guessing, and the compiler teaches the model when you slip:

```tsx
// ✗ Build error (mx-state-scope): morphState needs a component instance.
// The error is the lesson — this value would escape to every importer,
// so say so explicitly:
const [count, setCount] = morphState(0)

// ✓ Intent is explicit: one value, shared by everyone who imports it
export const [count, setCount] = morphShared<number>(0)
```

Rule of thumb: if you can answer "who owns this?" with a component instance, it is `morphState`. If the answer is "the whole app", it is `morphShared`. The different names keep you from confusing the two six months later.

## Why path-based imports instead of ID-based access like `setShared('cart', 0)`?

String IDs are a global namespace with no owner, and global namespaces don't scale to teams. Two developers pick `'cart'` for two different things, both writes land in one bucket, and the bug shows up far from either call site — with no compiler error pointing at it, because strings are invisible to the compiler.

Imports fix all three problems at once:

**1. No coordination needed.** Any file can declare `count` — the file path disambiguates, so teams never negotiate names:

```tsx
// team-a/cart.ts
export const [count, setCount] = morphShared<number>(0)

// team-b/notifications.ts — same binding name, zero conflict
export const [count, setCount] = morphShared<number>(0)
```

```tsx
// A screen using both — the only place names meet, and it is explicit
import { count as cartCount } from './team-a/cart'
import { count as notifCount } from './team-b/notifications'
```

**2. Self-documenting.** You discover state by importing, not by grepping for IDs. Type `import { } from './cart'` and the IDE lists exactly what that module shares — the component file *is* the documentation of its state:

```tsx
import { cart, setCart, clearCart } from './ShopStore.mx'
//                ^ IDE autocompletes these from the source module —
//                  no docs page, no ID registry to look up
```

**3. Wrong code doesn't compile.** A typo'd string ID (`'crat'`) silently creates a *new* empty bucket at runtime. A typo'd import (`coutn`) is a build error before anything runs. Renaming a binding renames it for every importer through normal IDE refactoring — try that with a string scattered across files.

In short: the module system already solved global naming decades ago. `morphShared` uses it instead of reinventing a worse one.

## Why have store files if components can export shared state directly — and vice versa?

Both are the same mechanism; the difference is **ownership**: which file is the source of truth, and why does *it* get to decide?

**Colocate in the component** when the state belongs to a piece of UI — one component renders it, owns it, and others merely react to it:

```tsx
// ExitBanner.mx — the banner owns its visibility
import { morphShared } from 'morph'

export const [showBanner, setShowBanner] = morphShared<boolean>(false)

export default function ExitBanner() {
  return (
    <div>
      {showBanner && (
        <div className="banner">
          <text>Please don't leave!</text>
          <button onClick={() => setShowBanner(false)}>Dismiss</button>
        </div>
      )}
    </div>
  )
}
```

```tsx
// Header.mx — a distant consumer; the state still "lives" in ExitBanner
import { setShowBanner } from './ExitBanner.mx'

export default function Header() {
  return <button onClick={() => setShowBanner(true)}>Log Out</button>
}
```

Benefits: the state sits next to the UI it drives, so deleting the component deletes its state — nothing orphaned. And discovery is natural: "who owns the banner flag? the banner file, obviously."

**Extract to a store file** when the state belongs to *no* single UI — several unrelated components read and write it, and assigning it to any one of them would be fake ownership:

```tsx
// themeStore.ts — no UI, just the source of truth
import { morphShared } from 'morph'

export const [theme, setTheme] = morphShared<'light' | 'dark'>('light')
export const [user, setUser] = morphShared<User | null>(null)
```

```tsx
// Header.mx, Settings.mx, Profile.mx — all equal consumers, none the owner
import { theme, setTheme } from './themeStore'
```

Benefits: a neutral home with no UI attached, so components never couple to *each other* just for data (Header importing state from Profile because that's where someone parked it). When two components start importing state back and forth, that web is the smell telling you to extract a store.

**Decision rule:** one component renders and owns it → colocate in that `.mx`. Two or more unrelated components share it and none owns it → `.ts` store. Same mechanism, same performance — the only question is which file a future reader will look in first.

**The deeper reason: shared truth must outlive any single component.** Imagine an auth system where the logged-in flag lives inside the login button, because that's where it was first needed:

```tsx
// LoginButton.mx — DON'T do this for app-wide state
export const [isLoggedIn, setIsLoggedIn] = morphShared<boolean>(false)

export function LoginButton() {
  return <button onClick={() => setIsLoggedIn(true)}>Log in</button>
}
```

It works — until six months later someone redesigns the login flow. The button gets deleted, renamed, or split into three variants, and with it goes the file every guard, header, and profile page imports its auth state from. One UI change cascades into broken imports across the app: everything that asked "is the user logged in?" was secretly asking "does the login button file still exist?". That is the mess — components are the most churned files in any codebase (redesigns, renames, deletions), so hanging shared truth on them means every UI refactor risks a data outage.

A store file inverts the dependency: UI comes and goes, the source of truth stays put.

```tsx
// authStore.ts — survives every redesign of every button
import { morphShared } from 'morph'

export const [isLoggedIn, setIsLoggedIn] = morphShared<boolean>(false)
export const [user, setUser] = morphShared<User | null>(null)
```

Delete the login button, rewrite it twice, A/B test four variants — `authStore.ts` never moves, and no import breaks. **Colocate what a component owns; store what the app depends on.** If deleting a file would break screens that have nothing to do with it, that file holds store-worthy state.

## Does every import create a new copy of the state?

No — the opposite. Every importer of the same binding reads and writes the **same** signal. That is the whole point: import the getter in five components and all five re-render when any one of them calls the setter. No providers, no prop drilling, no store setup.

## Two files declare the same variable name — shared or separate?

Separate. Identity is module path + binding name, so `cart.ts::count` and `notifications.ts::count` are two independent stores. You never have to coordinate names across files.

## What if I import the same name from two different files?

Hard build error (ambiguous import). Rename on import:

```tsx
import { count as cartCount } from './cart'
import { count as notifCount } from './notifications'
```

## Why must it be exported at module scope? Why not inside my component?

Inside a component it would be per-instance state (that is what [`morphState`](../morphState.md) is for). At module scope there is exactly one instance per app, which is what makes it shareable. The linter enforces this (`mx-shared-scope`) because mixing the two up is the most common state bug — and it fails at build time, not at runtime.

## Why must the binding be exported? Can I keep a store file-private?

An unexported binding is invisible to other modules, so nothing else could ever import it — at that point it is just a module-level variable, not shared state. The linter requires `export` so a "shared" store that nobody can reach fails loudly instead of silently behaving like a local.

## Does reading the getter subscribe my component? What about importing only the setter?

Reading the getter inside a component subscribes it — the component re-renders when the setter runs. Importing only the setter (e.g. a button that writes but never displays) does not subscribe, so the writer never re-renders from its own writes.

## Where should store files live — `.mx` or `.ts`?

Both work — use whatever you like. As a convention, prefer components in `.mx` (or `.tsx`) and non-component logic in `.ts`:

- **`.mx`** — component files. Colocate state with the component that owns it (`ExitBanner.mx` exports `showBanner` alongside the banner UI).
- **`.ts`** — plain logic with no UI: auth stores, theme settings, caches, domain state (`authStore.ts`, `themeStore.ts`).

Same rule applies to `morphEvent` channels: UI-owned events (toast, modal) live next to their component; system events (login, logout) live in a `.ts` module.

## What happens if I rename the store file?

The identity includes the file path, so renaming the file creates a **new, freshly-initialized** store. Rename freely during development; just know persisted state does not follow the rename.

## Can two components write at the same time?

Writes are applied synchronously in call order and re-renders are batched per tick, so concurrent writes resolve to the last write — same as any signal system. For read-modify-write, use the updater form (`setCount(c => c + 1)`) so overlapping updates compose instead of clobbering.
