#pragma once
// Morph JS Runtime Types — umbrella header
// Include this in generated C++ code for full JS semantics support.

#include "js_value.h"

// std::format / std::println support for Js* types.
// Included by default so `std::println("{}", JsNumber)` etc. work out of the box.
// Define MORPH_NO_FORMAT before including this header to opt-out of <format>
// parsing cost (~1.5s per TU) in hot-reload critical paths.
// Codegen still adds <format> only when a `${}` template or formatting is needed.
#ifndef MORPH_NO_FORMAT
#include "js_value_format.h"
#endif
