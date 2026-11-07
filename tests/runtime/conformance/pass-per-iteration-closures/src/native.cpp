// Callable storage for the conformance fixture. JS array storage for
// closures is a separate language gap (function values in containers);
// this native vector lets the fixture exercise the loop emitter's
// per-iteration bindings without depending on it.
#include <functional>
#include <string>

using NumFn = std::function<int64_t()>;
using StrFn = std::function<std::string()>;

void touchNative() {}
