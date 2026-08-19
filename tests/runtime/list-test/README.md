# List Test

Keyed list rendering with runtime reconciliation.

## What it shows

- `morphState([])` — array state
- `[...items, item]` — array spread to append
- `[...items].slice(0, -1)` — array spread + slice to remove last
- `items.map(item => <div key={item.id}>...)` — keyed reconciliation
- Conditional JSX inside an item template (`{show ? <span/> : null}`)
- Reordering/mutations trigger O(n) keyed diff in `morph::ListContainer::reconcile`

## Run

```bash
cd tests/runtime/list-test
morph dev
# or
morph run
```
