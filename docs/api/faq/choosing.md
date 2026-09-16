# Which API do I need?

A decision guide for the four core APIs. Full references: [`morphState`](../morphState.md), [`morphShared`](../morphShared.md), [`morphEvent`](../morphEvent.md), [`morphEffect`](../morphEffect.md).

## Start here

| Question | Answer |
|---|---|
| Does exactly one component instance need it? | `morphState` |
| Do distant components need the same value? | `morphShared` |
| Do distant components need to know something *happened*? | `morphEvent` |
| Do I need to touch something outside Morph (DOM, timers, network, storage)? | `morphEffect` |

## Common scenarios

**Form input, toggle, counter, local UI** → `morphState`. One instance, one owner, dies with the component.

**Theme, auth user, cart total, modal open-flag** → `morphShared` in a store module. Many readers, many writers, one value.

**"Show a toast", "refresh the list", "user logged out"** → `morphEvent`. Transient, no value to keep, subscribers react and move on. If any subscriber also needs the *data* (not just the nudge), pair the event with a `morphShared` write.

**Document title, `localStorage` sync, timers, manual subscriptions** → `morphEffect`, reading whichever state it depends on.

## Combinations that work well

- **Event + shared**: emitter writes the store *and* emits; listeners react immediately, late components read the store. Covers both "when" and "what".
- **Shared + effect**: component reads a global store inside an effect to sync an external system whenever the store changes.
- **State + effect**: local state drives a timer or fetch scoped to one component instance.

## What the linter will tell you

| You wrote… | Error | Meaning |
|---|---|---|
| `morphState` at module scope | `mx-state-scope` | Local state needs a component instance — move it inside |
| `morphShared` / `morphEvent` inside a component | `mx-shared-scope` / `mx-event-scope` | Shared things live at module scope — move them out |
| Same name imported from two modules | ambiguous import | Two different identities claim one local name — rename on import |
