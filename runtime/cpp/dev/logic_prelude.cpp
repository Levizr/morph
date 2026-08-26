// Dev hot-reload support: single home for the heavy template instantiations
// that generated logic sources would otherwise re-instantiate on every
// reload. Compiled into the morph_devrt host (linked -rdynamic), so a
// dlopen'ed logic .so binds these symbols from the running process.
#include "logic_prelude.h"
#include "signal_store.h"

#include <cstdio>
#include <string>

namespace morph {

void dev_log(const std::string& msg) {
    std::fputs(msg.c_str(), stdout);
    std::fputc('\n', stdout);
    std::fflush(stdout);
}

void dev_log_warn(const std::string& msg) {
    std::fputs("[warn] ", stderr);
    std::fputs(msg.c_str(), stderr);
    std::fputc('\n', stderr);
    std::fflush(stderr);
}

void dev_log_error(const std::string& msg) {
    std::fputs("[error] ", stderr);
    std::fputs(msg.c_str(), stderr);
    std::fputc('\n', stderr);
    std::fflush(stderr);
}

// Explicit instantiation definitions — emitted once, here, in the host.
template class Signal<std::string>;
template class Signal<int>;
template class Signal<double>;
template class Signal<bool>;
template class Signal<JsValue>;

} // namespace morph

template morph::Signal<std::string>& SignalStore::get_or_create<std::string>(const std::string&, std::string);
template morph::Signal<int>& SignalStore::get_or_create<int>(const std::string&, int);
template morph::Signal<double>& SignalStore::get_or_create<double>(const std::string&, double);
template morph::Signal<bool>& SignalStore::get_or_create<bool>(const std::string&, bool);
