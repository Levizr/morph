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

Same rules as JavaScript, with one exception: `0`, `""`, `null`, `undefined`, `false` are falsy and everything else is truthy — except an **empty array is falsy** in Morph (JavaScript treats it as truthy). See [How JavaScript Comparisons Work in Morph](./js-comparisons.md) for the full truthiness table.

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
| `push(value)` | Append to end |
| `pop()` | Remove from end |
| `index(i)` | Access by index |

## JsObject

Object backed by `shared_ptr<map<string, JsValue>>`:

| Method | Description |
|---|---|
| `has(key)` | Check if key exists |
| `keys()` | Get all keys |
| `index(key)` | Access by key (also `obj["key"]`) |

## JsBoolean

Wrapper around `bool` with truthiness semantics.

## Method Forwarding

`JsValue` forwards the common string methods (`toUpperCase`, `toLowerCase`, `trim`, `charAt`, `substring`, `slice`, `replace`, `split`, `toString`, …), so `obj["name"].toUpperCase()` and `arr[0].toString()` work without unwrapping. Native `std::string` values get the same surface through `morph::str::*` helpers instead — see [JS Methods on Natives](./native-types.md#js-methods-on-natives).

## Formatting

All `Js*` types support `std::format` and `std::println` via formatters in `runtime/cpp/types/js_value_format.h`. Define `MORPH_NO_FORMAT` before including `js_types.h` to opt out of the `<format>` parse cost (~1.5s per TU) in hot-reload critical paths.