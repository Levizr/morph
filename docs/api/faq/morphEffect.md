# morphEffect FAQ

## Why explicit dependencies? Why not auto-detect like Solid or Svelte?

Auto-detection sounds convenient until it guesses wrong — and it guesses wrong in both directions. Explicit deps in `[]` mean one thing: **when to run**. What you read inside the body is free and never adds triggers.

**Case 1: auto-detect misses what you want to track.** You want to log every time `count` changes, but the body never mentions it:

```tsx
morphEffect(() => {
  console.log("count changed")
}, [count])   // ✓ runs whenever count changes — nothing to "detect" in the body
```

An auto-detecting framework scans the body, finds no `count` read, subscribes to nothing, and the effect never re-runs. There is no value to detect, yet `count` is exactly what you want to track. With explicit deps you just say so.

**Case 2: auto-detect tracks what you don't want.** You read both `cart` and `count` inside, but only `count` should re-trigger:

```tsx
morphEffect(() => {
  console.log(cart.total, count)   // uses both...
}, [count])                        // ...but only count re-runs it
```

Auto-track frameworks subscribe to everything read — both signals — and you must actively opt *out* (untrack/ignore wrappers around `cart`) to stop the extra runs. Morph inverts the default: declare in `[]` only what should trigger, and use anything else inside without worrying whether you need to declare it or suppress it.

Under the hood, the deps list builds a change-signature: the effect re-runs only when a listed dependency's value changes. Body reads don't extend that signature. So the rule is simple — `[]` answers "when should this run?", the body answers "what should it do?", and the two never leak into each other.

## If explicit deps are so good, why did Svelte and Solid remove them? Are deps useless?

They didn't remove the *problem* — they moved it. And both ended up adding manual controls back, which is the strongest proof the problem is real.

**Svelte promised "no boilerplate".** Its core pitch is less code than React, so it *cannot* ask you to write a deps array — that would be React with extra steps and break the promise. Auto-tracking is the price of that pitch. The cost shows up as dummy reads: referencing a value in the body just so the compiler subscribes to it, even though the logic doesn't need it:

```tsx
// Svelte-style workaround: mention count() so it gets tracked,
// even though the log line doesn't use the value
$effect(() => {
  count()
  console.log("count changed")
})
```

That dummy line *is* a dependency declaration — just an unreadable one, hidden inside the body where no reader can tell "trigger" apart from "logic".

Worse: it looks exactly like dead code. A junior doing cleanup, a linter flagging unused expressions, a reviewer skimming the diff — all of them see `count()` sitting alone, doing nothing visible, and delete it. Nothing fails. No error, no warning, no test catches it at the deletion site. The effect just silently stops re-running, and the bug surfaces far away as stale UI with no trail back to the removed line. The person who deleted it did everything right by every visible signal; the framework punished them anyway.

Compare that with Morph: deleting `count` from `[count]` is a visible change to an explicit tracking contract — reviewers see a deps-array diff and ask "should this still run on count?". And a listed dep never *looks* unused, because the array's job is declaration, not execution. Nothing load-bearing ever disguises itself as clutter.

**Solid reacted differently: explicit opt-outs.** Solid kept auto-tracking but added `on()` (declare deps explicitly, like a deps array by another name) and `untrack()` (read without subscribing). Svelte 5 added `untrack()` too. So in both frameworks you still manage tracking by hand — only through secondary escape hatches instead of the primary API:

```tsx
// Solid: same idea as deps, spelled differently
createEffect(on(count, () => console.log("count changed")))

// Solid/Svelte: suppressing what you read but don't want to track
untrack(() => console.log(cart.total))
```

Count the concepts: auto-tracking *plus* `on` *plus* `untrack` *plus* dummy reads *plus* knowing when each applies. They tried to remove the deps array but got stuck — breaking the "no boilerplate" promise was not an option, so they piled new APIs on top to solve the same problem the array already solved. And notice what that leaves you with: you must now explicitly define **both** sides — what you want to track (`on(...)`, dummy reads) **and** what you don't want to track (`untrack(...)`). Two-sided bookkeeping, forever, for every effect.

Morph asks for one side: define what you need to track. Simple:

```tsx
morphEffect(() => {
  // read anything here — cart, count, whatever the logic needs.
  // none of it becomes a trigger unless listed below.
}, [count])   // ← the entire tracking contract, in one place
```

No opt-outs, no dummy reads, no second API to learn for the cases auto-tracking gets wrong. The frameworks that dropped explicit deps reintroduced them piece by piece; Morph never removed them.

And each attempted fix made things worse, not better. One mechanism (the deps array) became four overlapping ones — auto-tracking, `on()`, `untrack()`, dummy reads — that interact with each other: what does `on()` inside `untrack()` mean? Does a dummy read inside a nested closure subscribe? Every new escape hatch added rules, edge cases, and docs pages, so now there are *more* ways to get tracking wrong than the single array they called boilerplate. They tried to fix the deps array out of existence and ended up with a bigger, subtler version of it.

## When exactly does my effect run?

- **No dependency array** (or `[]`): once, after the first render.
- **With dependencies**: after the first render, then again after any render where a listed dependency changed.

```tsx
morphEffect(() => { /* once */ })
morphEffect(() => { /* on mount + whenever count changes */ }, [count])
```

## When does the cleanup function run?

The function you return runs **before** the effect re-runs and when the component unmounts. Classic use is tearing down what the effect set up:

```tsx
morphEffect(() => {
  const id = setInterval(() => setSeconds(s => s + 1), 1000)
  return () => clearInterval(id)   // runs before re-run / on unmount
}, [])
```

If the effect has no setup to undo, return nothing.

## My effect runs in an infinite loop. Why?

Almost always: the effect writes a signal it also reads (directly or through deps), so each run schedules the next run. Fix by narrowing the dependency array to only what should *trigger* the effect, and reading everything else without subscribing — or restructure so the write happens in an event handler instead of an effect.

```tsx
// ✗ Loop: writes count, and count re-triggers the effect
morphEffect(() => { setCount(count + 1) }, [count])
```

## Can effects read shared state?

Yes. Reading a `morphShared` getter inside an effect body subscribes the effect, so it re-runs when that store changes — same as local state. This is the standard way to sync side effects (logging, storage, network) with global stores:

```tsx
morphEffect(() => {
  console.log("Theme changed:", theme)
}, [theme])
```

## Effect vs. event handler — where does this logic go?

- **Event handler** (`onClick`, etc.): logic that runs *because the user did something*. Prefer handlers for writes — they never loop.
- **Effect**: logic that runs *because the rendered output changed* (syncing with non-reactive systems: timers, storage, network, logging).

If you can do it in the handler, do it there. Reach for an effect when the trigger is "this value changed", not "the user clicked".

## Can I make the effect async?

Effects themselves are synchronous — declare the body as a normal function and handle async work inside it (promise chains or an inner async call). Don't return the promise where the cleanup function goes.
