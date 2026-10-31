// Net runtime test (headless): Response.json() must parse the body with
// JS JSON semantics instead of returning the raw string.
#include "../../net/net.h"

#include <cstdio>

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

static std::string sv(const JsValue& v)
{
    return morph::js_to_string(v).value;
}

static void testResponseJson()
{
    morph::net::Response response;
    response.body = "{\"x\":1,\"items\":[2,3]}";
    JsValue parsed = response.json();
    CHECK(parsed.is_object(), "Response.json returns an object");
    CHECK(sv(parsed["x"]) == "1", "Response.json parses fields");
    CHECK(sv(parsed["items"][1]) == "3", "Response.json parses nested arrays");
    CHECK(response.bodyUsed, "Response.json marks the body consumed");
}

static void testInvalidJson()
{
#if defined(__cpp_exceptions)
    morph::net::Response response;
    response.body = "{bad}";
    try
    {
        (void)response.json();
        CHECK(false, "Response.json rejects malformed JSON");
    }
    catch (const JsValue& e)
    {
        CHECK(sv(e["name"]) == "SyntaxError", "Response.json throws SyntaxError");
    }
#endif
}

int main()
{
    testResponseJson();
    testInvalidJson();
    std::printf("[net-test] %d checks, %d failures\n", checks, failures);
    return failures == 0 ? 0 : 1;
}
