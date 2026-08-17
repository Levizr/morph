#pragma once
// ─────────────────────────────────────────────────────────────────────────────
// Morph public C++ API — the single header user .cpp files import.
//
//   #include "morph_api.h"
//
// Everything a native extension needs is exposed here: JS value types,
// reactive signals, node/event access, and async task primitives.
// Because morph compiles .mx (JSX/TS) straight to C++, "calling JSX" from
// C++ is just calling the generated functions — see the generated
// `_morph_state.h` header in your output dir for state setters/getters and
// JSX function declarations.
// ─────────────────────────────────────────────────────────────────────────────

// JS runtime value types (JsValue, JsNumber, JsString, JsArray, ...)
#include "types/js_types.h"
#include "types/js_value.h"
#include "types/js_number.h"
#include "types/js_string.h"
#include "types/js_array.h"
#include "types/js_object.h"
#include "types/js_boolean.h"

// Reactivity: morph::Signal<T>, create_effect(), morph::str()
#include "reactivity/signal.h"

// Async: morph::Task coroutines, next_frame, timers, morph::Result<T>
#include "reactivity/task.h"
#include "reactivity/promise.h"

// Node tree & events (MorphNode*, MorphEvent*, MorphStyle)
#include "core/node.h"
#include "core/event.h"
