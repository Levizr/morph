// Dev hot-reload support header (included by generated logic sources).
//
// The heavy C++ template instantiations the logic needs (Signal<T> for the
// common state types, SignalStore::get_or_create<T>) live in the morph_devrt
// HOST binary, which links with -rdynamic so a dlopen'ed logic .so resolves
// them from the running process. This header only carries `extern template`
// declarations, so every hot reload compiles just the user's logic instead
// of re-instantiating the whole reactivity machinery.
//
// Any type NOT covered here still works: the compiler silently falls back to
// an implicit instantiation inside the logic TU (only slower).
#pragma once

#include <string>
#include "reactivity/signal.h"
#include "types/js_value.h"

#ifndef MORPH_FEATURE_DEV
// ── Production / static builds ──────────────────────────────
// The devrt host isn't linked in, so console sinks are plain inline helpers
// and templates instantiate normally inside this binary.
#include <cstdio>
namespace morph {
inline void dev_log(const std::string& msg) {
    std::fputs(msg.c_str(), stdout);
    std::fputc('\n', stdout);
}
inline void dev_log_warn(const std::string& msg) {
    std::fputs("[warn] ", stderr); std::fputs(msg.c_str(), stderr); std::fputc('\n', stderr);
}
inline void dev_log_error(const std::string& msg) {
    std::fputs("[error] ", stderr); std::fputs(msg.c_str(), stderr); std::fputc('\n', stderr);
}
} // namespace morph
#else
// ── Dev hot-reload builds ───────────────────────────────────
#include "signal_store.h"

// The heavy C++ template instantiations the logic needs (Signal<T> for the
// common state types, SignalStore::get_or_create<T>) live in the morph_devrt
// HOST binary, which links with -rdynamic so a dlopen'ed logic .so resolves
// them from the running process. This header only carries `extern template`
// declarations, so every hot reload compiles just the user's logic instead
// of re-instantiating the whole reactivity machinery.
//
// Any type NOT covered here still works: the compiler silently falls back to
// an implicit instantiation inside the logic TU (only slower).

namespace morph {

// console.log/warn/error/info sinks (defined in logic_prelude.cpp, resolved
// from the devrt host). Plain string args — no <format>/<print> machinery.
void dev_log(const std::string& msg);
void dev_log_warn(const std::string& msg);
void dev_log_error(const std::string& msg);

extern template class Signal<std::string>;
extern template class Signal<int>;
extern template class Signal<double>;
extern template class Signal<bool>;
extern template class Signal<JsValue>;

} // namespace morph

// Common get_or_create instantiations are emitted by logic_prelude.cpp in
// the devrt host; the logic TU only references them.
extern template morph::Signal<std::string>& SignalStore::get_or_create<std::string>(const std::string&, std::string);
extern template morph::Signal<int>& SignalStore::get_or_create<int>(const std::string&, int);
extern template morph::Signal<double>& SignalStore::get_or_create<double>(const std::string&, double);
extern template morph::Signal<bool>& SignalStore::get_or_create<bool>(const std::string&, bool);
#endif
