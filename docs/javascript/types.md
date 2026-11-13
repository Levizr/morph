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

Constants: `PI`, `E`, `LN2`, `LN10`, `LOG2E`, `LOG10E`, `SQRT1_2`, `SQRT2`.

Functions: `min`/`max` (variadic, and `...spread` over arrays), `abs`, `floor`, `ceil`, `round`, `trunc`, `sign`, `pow`, `sqrt`, `cbrt`, `exp`, `expm1`, `log`, `log2`, `log10`, `log1p`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `sinh`, `cosh`, `tanh`, `hypot`, `fround`, and `random`. Arguments coerce through `Number()` (so `Math.abs("x")` is `NaN`); `round` rounds half toward +∞ (`Math.round(-2.5)` → `-2`); empty `min`/`max` return `Infinity`/`-Infinity`; `hypot` takes fixed arguments (spread is limited to `min`/`max`).

## JSON

`JSON.parse(text)` and `JSON.stringify(value)`:

- `parse` is strict JSON — no trailing garbage or comments — depth-capped at 100, and throws a catchable `SyntaxError` on bad input.
- `stringify` follows JS: `undefined` array elements become `null`, object properties with `undefined` values are omitted, top-level `undefined` returns `undefined`. No reviver or replacer.
- Key order is sorted (see the JsObject note), not insertion order.
- Both return a `JsValue`; dot access reads the same field as bracket access, including on inline call results: `JSON.parse(text).name` is `JSON.parse(text)["name"]`.

Everything on this page is checker-gated: unsupported globals, members, or methods stay a hard `morph check` error rather than compiling to something surprising.

## Date

`new Date(...)` lowers to the native `morph::Date` value type (`runtime/cpp/types/js_date.h`). Construction:

- `new Date()` — current time.
- `new Date(ms)` — milliseconds since the epoch. Values outside ±8.64e15 are an Invalid Date; fractional milliseconds truncate toward zero.
- `new Date(text)` — the ISO subset below; anything else is an Invalid Date.
- `new Date(year, month, day?, hours?, minutes?, seconds?, ms?)` — local calendar fields. Years `0`–`99` map to `1900`–`1999`; out-of-range month/day/hour values roll over like JS. At most seven arguments.

`Date.now()` returns the current epoch milliseconds and `Date.parse(text)` returns the parsed timestamp (`NaN` when invalid). Both produce a `JsNumber`.

Accepted `Date.parse` / `new Date(text)` inputs:

- `YYYY-MM-DD` — date-only parses as UTC, matching the spec.
- `YYYY-MM-DDTHH:MM[:SS[.sss]][Z|±HH:MM|±HHMM]` — a datetime without an offset is local time; offsets accept `+HHMM` and `+HH:MM`.
- `24:00` rolls to the next day; fractional seconds accept more than three digits and truncate to milliseconds.

Instance methods: `getTime`, `valueOf`, `toISOString`, plus local and UTC getters — `getFullYear`/`getUTCFullYear`, `getMonth`/`getUTCMonth`, `getDate`/`getUTCDate`, `getDay`/`getUTCDay`, `getHours`/`getUTCHours`, `getMinutes`/`getUTCMinutes`, `getSeconds`/`getUTCSeconds`, `getMilliseconds`/`getUTCMilliseconds`.

Invalid dates return `NaN` from `getTime`/`valueOf`/getters; `toISOString()` throws a catchable `RangeError`. Timezone-aware formatting (`toString`, `toLocaleString`, `toLocaleDateString`, …), `getTimezoneOffset`, and setters are not implemented — `morph check` rejects them with `mx-js-method`. Unsupported statics (`Date.UTC`, `Date.prototype`, …) are `mx-js-member`; `Date()` without `new`, spreads, and over-arity calls are `mx-js-syntax`. Runtime tests run with `TZ=UTC`, so local and UTC getters agree there.

Date values are native value types, not `JsValue`s. They can be constructed, stored in Date-typed locals and fields, passed to functions that take `Date`, returned, logged with `console.log`, and used with the methods above. Every boundary must be explicit: a parameter, return type, class field, or variable that holds a Date needs an exact `: Date` annotation. Unannotated boundaries (generic parameters, untyped fields and locals, unannotated returns, constructor parameters, and callbacks), destructuring a Date, assigning a Date to an untyped target, and exporting a Date or a Date-producing function/class across modules are rejected by `morph check`. Generic JS value positions have no lowering and are rejected too: `String(date)`, `Number(date)`, `Boolean(date)`, `parseInt`/`parseFloat(date)`, `Math.*`/`Number.is*` with a Date argument, `JSON.stringify(date)`, `Date.parse(date)`, template interpolation, string concatenation/arithmetic/comparisons, conditions, `switch`, array/object literals, JSX text, `for...of`/`for...in`, and unsupported members. Convert explicitly with `.getTime()` or `.toISOString()` first.

## JsBoolean

Wrapper around `bool` with truthiness semantics.

## Method Forwarding

`JsValue` forwards the common string methods (`toUpperCase`, `toLowerCase`, `trim`, `charAt`, `substring`, `slice`, `replace`, `split`, `toString`, …), so `obj["name"].toUpperCase()` and `arr[0].toString()` work without unwrapping. Native `std::string` values get the same surface through `morph::str::*` helpers instead — see [JS Methods on Natives](./native-types.md#js-methods-on-natives).

## Formatting

All `Js*` types support `std::format` and `std::println` via formatters in `runtime/cpp/types/js_value_format.h`. Define `MORPH_NO_FORMAT` before including `js_types.h` to opt out of the `<format>` parse cost (~1.5s per TU) in hot-reload critical paths.