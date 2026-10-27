// Signal overload-resolution test (headless): a dynamic JsValue written to a
// typed signal must use the value overload, not the functional-updater
// overload. JsValue is callable, so the updater template used to win and
// recurse until the stack overflowed on the first text-input keystroke.
#include "../../reactivity/signal.h"

#include <cstdio>
#include <string>

#if !defined(MORPH_FEATURE_REACTIVITY)
#error "signal_test requires the REACTIVITY feature"
#endif

static int failures = 0;
static int checks = 0;

static void check(bool cond, const char* name, int line)
{
    checks++;
    if (!cond)
    {
        failures++;
        std::printf("FAIL %s (line %d)\n", name, line);
    }
}

#define CHECK(cond, name) check((cond), (name), __LINE__)

int main()
{
    morph::Signal<std::string> text("seed");
    text.set(JsValue("typed"));
    CHECK(text.get() == "typed", "JsValue selects the string value overload");

    // Functional updaters still work for real callables.
    text.set([](const std::string& prev) { return prev + "!"; });
    CHECK(text.get() == "typed!", "lambda updater still selected for callables");

    morph::Signal<JsValue> dynamic(JsValue("a"));
    dynamic.set(JsValue("b"));
    CHECK(dynamic.get().as_string() == "b", "JsValue signal keeps the value overload");

    morph::Signal<int> count(1);
    count.set(2);
    count.set([](int prev) { return prev + 1; });
    CHECK(count.get() == 3, "numeric value and lambda updater both work");

    std::printf("[signal-test] %d checks, %d failures\n", checks, failures);
    return failures == 0 ? 0 : 1;
}
