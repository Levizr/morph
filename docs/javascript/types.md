# Runtime Types

Morph's C++ runtime provides JavaScript-compatible types that handle the dynamic nature of JSX expressions.

When you get a wrapper vs a native (`std::string`, `int64_t`, `std::vector<T>`) is the compiler's choice, not yours: under the default `--type infer` the analyzer picks natives wherever the usage allows and only reaches for these wrappers for genuinely dynamic values; `--type strict` keeps your annotations verbatim. See [Native C++ Types](./native-types.md#how---type-picks-native-vs-wrapper).

## JsValue

The universal value type. Every expression in JSX resolves to a `JsValue` which can hold:

| Type | C++ | Example |
|---|---|---|
| `undefined` | `JsUndefined` | `morphState()` with no arg |
| `null` | `JsNull` | `null` |
| `boolean` | `JsBoolean` | `true`, `false` |
| `number` | `JsNumber` | `42`, `3.14` |
| `string` | `JsString` | `"hello"` |
| `array` | `JsArray` | `[1, 2, 3]` |
| `object` | `JsObject` | `{ key: "value" }` |

### typeof

`typeof` works like JavaScript:

```
typeof(undefined)  → "undefined"
typeof(null)       → "object"
typeof(42)         → "number"
typeof("hello")    → "string"
typeof(true)       → "boolean"
```

### Truthiness

Same rules as JavaScript: `false`, `0`, `NaN`, `""`, `null`, and `undefined` are falsy; everything else — arrays (empty ones included) and objects — is truthy. See [How JavaScript Comparisons Work in Morph](./js-comparisons.md) for the full truthiness table.

### Equality

`==` and `!=` follow JavaScript coercion rules, `===` and `!==` check type + value — for `Js*` wrappers and native types alike. The translator generates `morph::js_cmp` helpers wherever plain C++ would answer differently. See [How JavaScript Comparisons Work in Morph](./js-comparisons.md).

## JsNumber

A numeric type that handles integers, doubles, and big-number strings:

```cpp
JsNumber(42)         // int
JsNumber(3.14)       // double
JsNumber("12345678901234567890")  // big string
```

Arithmetic operators (`+`, `-`, `*`, `/`) work with proper coercion. Number formatting renders cleanly — `8` not `8.0`, `2.5` not `2.500000`, `"Error"` not `nan`/`inf`.

### Number Formatting

| Method | Description |
|---|---|
| `toString()` | Shortest round-tripping decimal — `0.1`, `1e+21`, `NaN`, `Infinity`, `-0` → `"0"` |
| `toFixed(digits)` | Fixed-point string, 0–100 digits; ties round away from zero (`(2.5).toFixed(0)` → `"3"`) |
| `toPrecision(sig)` | 1–100 significant digits; switches to exponential when the exponent exceeds `sig` |
| `toExponential(frac)` | Exponential notation, 0–100 fraction digits; no argument gives the shortest mantissa |

Out-of-range digits throw a catchable `RangeError` like V8 (detectable with `try`/`catch`).

## JsString

String type with JS-compatible methods:

| Method | Description |
|---|---|
| `toUpperCase()` | Convert to uppercase |
| `toLowerCase()` | Convert to lowercase |
| `trim()` | Remove whitespace |
| `charAt(i)` | Get character at index |
| `indexOf(s)` | Find substring position |
| `substring(start, end)` | Extract substring |
| `slice(start, end)` | Extract substring |
| `replace(old, new)` | Replace first occurrence |
| `split(sep)` | Split into array |

String concatenation with `+` works: `"count: " + 42` produces `"count: 42"`.

## JsArray

Array backed by `shared_ptr<vector<JsValue>>`:

| Method | Description |
|---|---|
| `push(value)` / `pop()` | Append / remove at end |
| `join(sep?)` | Join elements into a string (default `,`); `null`/`undefined` elements join empty |
| `slice(start, end?)` | Copy a subarray; negative indices count from the end |
| `concat(x, …)` | Concatenate values or arrays into a new array (one level deep) |
| `includes(v, from?)` | Strict-equality membership |
| `indexOf(v, from?)` / `lastIndexOf(v, from?)` | Strict-equality search |
| `at(i)` | Index access with JS negative indexing; out of range → `undefined` |
| `reverse()` | Reverse in place, returns the array |
| `toString()` | Same as `join()` |
| `index(i)` | Access by index |

`Array.isArray(x)` answers `true` for arrays however they are bound — literal `JsArray`, `JsValue`, or a mutated `shared_ptr<JsArray>` local.

## JsObject

Object backed by `shared_ptr<unordered_map<string, JsValue>>`:

| Method | Description |
|---|---|
| `has(key)` | Check if key exists |
| `keys()` / `values()` / `entries()` | Enumerate properties |
| `index(key)` | Access by key (also `obj["key"]`) |

Object enumeration — `Object.keys`/`values`/`entries` and `JSON.stringify` — walks keys in **sorted order**, not JavaScript insertion order. This is a deliberate trade-off of the unordered-map runtime (O(1) point lookups, see [`docs/future/shipped/kill-runtime-strings.md`](../future/shipped/kill-runtime-strings.md)); output is deterministic, but don't rely on insertion order.

## Conversions

`String(x)`, `Number(x)`, and `Boolean(x)` coerce with JavaScript rules:

| Call | Notes |
|---|---|
| `String(x)` | Arrays join with `,` — `String([1,[2,3]])` → `"1,2,3"` |
| `Number(x)` | `""`/`null`/`[]` → `0`, `[5]` → `5`, `[[5]]` → `5`, garbage → `NaN`; parses `0x`/`0b`/`0o` prefixes |
| `Boolean(x)` | The falsy set above; arrays and objects are always truthy |
| `parseInt(s, radix?)` | Optional sign, stops at the first invalid character; `parseInt("08")` is `8` |
| `parseFloat(s)` | Like `parseInt` but accepts decimals/exponents; `"Infinity"` works |
| `Number.isFinite(x)` / `isNaN(x)` / `isInteger(x)` | No coercion — `Number.isFinite("1")` is `false` |

## Math

`Math.min`/`max` (variadic, and `...spread` over arrays), `abs`, `floor`, `ceil`, `round`, `pow`, `sqrt`. Arguments coerce through `Number()`; `round` rounds half toward +∞ (`Math.round(-2.5)` → `-2`); empty `min`/`max` return `Infinity`/`-Infinity`.

## JSON

`JSON.parse(text)` and `JSON.stringify(value)`:

- `parse` is strict JSON — no trailing garbage or comments — depth-capped at 100, and throws a catchable `SyntaxError` on bad input.
- `stringify` follows JS: `undefined` array elements become `null`, object properties with `undefined` values are omitted, top-level `undefined` returns `undefined`. No reviver or replacer.
- Key order is sorted (see the JsObject note), not insertion order.

Everything on this page is checker-gated: unsupported globals, members, or methods stay a hard `morph check` error rather than compiling to something surprising.

## JsBoolean

Wrapper around `bool` with truthiness semantics.

## Method Forwarding

`JsValue` forwards the common string methods (`toUpperCase`, `toLowerCase`, `trim`, `charAt`, `substring`, `slice`, `replace`, `split`, `toString`, …), so `obj["name"].toUpperCase()` and `arr[0].toString()` work without unwrapping. Native `std::string` values get the same surface through `morph::str::*` helpers instead — see [JS Methods on Natives](./native-types.md#js-methods-on-natives).

## Formatting

All `Js*` types support `std::format` and `std::println` via formatters in `runtime/cpp/types/js_value_format.h`. Define `MORPH_NO_FORMAT` before including `js_types.h` to opt out of the `<format>` parse cost (~1.5s per TU) in hot-reload critical paths.