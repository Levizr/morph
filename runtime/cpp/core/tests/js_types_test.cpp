// JS runtime types test (headless): Number toString formatting
// (toFixed/toPrecision/toExponential), String/Number/Boolean conversions,
// parseInt/parseFloat, Number predicates, Array/Object methods, Math and
// JSON. Every expected value was produced by node v24.14.1; the only
// intentional deviations are object key enumeration (sorted here, see
// docs/future/shipped/kill-runtime-strings.md) and non-number receivers
// of the number formatters (defensive "NaN" instead of TypeError).
#include "../../types/js_types.h"

#include <cmath>
#include <cstdio>
#include <limits>
#include <memory>
#include <string>

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

// String view of a JsValue with JS interpolation semantics.
static std::string sv(const JsValue& v)
{
    return morph::js_to_string(v).value;
}

static std::string num_str(double d)
{
    return JsNumber(d).as_string();
}

static void testNumberToString()
{
    CHECK(num_str(0.1) == "0.1", "0.1 stringifies shortest");
    CHECK(num_str(1e21) == "1e+21", "1e21 uses exponential");
    CHECK(num_str(1e20) == "100000000000000000000", "1e20 stays plain");
    CHECK(num_str(5e-7) == "5e-7", "5e-7 uses exponential");
    CHECK(num_str(1e-6) == "0.000001", "1e-6 stays plain");
    CHECK(num_str(-0.0) == "0", "-0 stringifies as 0");
    CHECK(num_str(std::nan("")) == "NaN", "NaN stringifies");
    CHECK(num_str(-std::numeric_limits<double>::infinity()) == "-Infinity",
          "-Infinity stringifies");
    CHECK(num_str(123.0) == "123", "integral double stays integral");
}

static void testToFixed()
{
    CHECK(JsNumber(1.005).toFixed(2).value == "1.00", "1.005 is 1.00499.. at 2 digits");
    CHECK(JsNumber(2.5).toFixed(0).value == "3", "ties round away from zero");
    CHECK(JsNumber(-2.5).toFixed(0).value == "-3", "negative ties round away");
    CHECK(JsNumber(0.5).toFixed(0).value == "1", "0.5 rounds to 1");
    CHECK(JsNumber(1.25).toFixed(1).value == "1.3", "1.25 to 1 digit");
    CHECK(JsNumber(1.35).toFixed(1).value == "1.4", "1.35 is above the tie in binary");
    CHECK(JsNumber(1e21).toFixed(2).value == "1e+21", ">=1e21 falls back to toString");
    CHECK(JsNumber(0.0).toFixed(2).value == "0.00", "zero keeps fraction zeros");
    CHECK(JsNumber(-0.0).toFixed(2).value == "0.00", "negative zero has no sign");
    CHECK(JsNumber(-0.4).toFixed(0).value == "-0", "rounded-to-zero keeps sign");
    CHECK(JsNumber(5).toFixed().value == "5", "no-arg toFixed is 0 digits");
}

static void testPrecisionFormats()
{
    CHECK(JsNumber(123.456).toPrecision(4).value == "123.5", "toPrecision rounds");
    CHECK(JsNumber(100).toPrecision(1).value == "1e+2", "toPrecision exponential");
    CHECK(JsNumber(0).toPrecision(1).value == "0", "zero precision 1");
    CHECK(JsNumber(0).toPrecision(3).value == "0.00", "zero precision 3");
    CHECK(JsNumber(5).toPrecision().value == "5", "no-arg toPrecision is toString");
    CHECK(JsNumber(12345).toPrecision(2).value == "1.2e+4", "toPrecision e >= sig");
    CHECK(JsNumber(0.000123).toPrecision(2).value == "0.00012", "toPrecision small");
    CHECK(JsNumber(0.00000123).toPrecision(2).value == "0.0000012",
          "toPrecision e = -6 stays plain");
    CHECK(JsNumber(123456).toExponential(2).value == "1.23e+5", "toExponential frac");
    CHECK(JsNumber(0.000123).toExponential().value == "1.23e-4",
          "no-arg toExponential shortest");
    CHECK(JsNumber(0).toExponential().value == "0e+0", "zero exponential");
    CHECK(JsNumber(0).toExponential(2).value == "0.00e+0", "zero with fraction");
    CHECK(JsNumber(999).toExponential(1).value == "1.0e+3",
          "mantissa carry bumps exponent");
    CHECK(JsNumber(123.456).toExponential(0).value == "1e+2", "zero fraction digits");
    CHECK(JsNumber(123456).toExponential().value == "1.23456e+5",
          "shortest mantissa round-trips");
}

static void testRangeErrors()
{
#if defined(__cpp_exceptions)
    try
    {
        (void)JsNumber(2.5).toFixed(200);
        CHECK(false, "toFixed(200) throws");
    }
    catch (const JsValue& e)
    {
        CHECK(sv(e["name"]) == "RangeError", "toFixed(200) RangeError");
    }
    try
    {
        (void)JsNumber(0).toExponential(200);
        CHECK(false, "toExponential(200) throws even at zero");
    }
    catch (const JsValue& e)
    {
        CHECK(sv(e["name"]) == "RangeError", "toExponential(200) RangeError");
    }
    try
    {
        (void)JsNumber(5).toExponential(-1);
        CHECK(false, "toExponential(-1) throws");
    }
    catch (const JsValue& e)
    {
        CHECK(sv(e["name"]) == "RangeError", "toExponential(-1) RangeError");
    }
    try
    {
        (void)JsNumber(5).toPrecision(0);
        CHECK(false, "toPrecision(0) throws");
    }
    catch (const JsValue& e)
    {
        CHECK(sv(e["name"]) == "RangeError", "toPrecision(0) RangeError");
    }
    try
    {
        (void)JsNumber(0).toPrecision(200);
        CHECK(false, "toPrecision(200) throws");
    }
    catch (const JsValue& e)
    {
        CHECK(sv(e["name"]) == "RangeError", "toPrecision(200) RangeError");
    }
#endif
}

static void testConversions()
{
    CHECK(morph::js_to_string().value.empty(), "String() is empty");
    CHECK(sv(JsValue(42)) == "42", "String(42)");
    CHECK(morph::js_to_string(JsValue(JsArray{1, 2, 3})).value == "1,2,3",
          "String(array) joins with commas");
    CHECK(morph::js_to_string(JsValue(JsArray{1, JsArray{2, 3}})).value == "1,2,3",
          "nested arrays join flat");
    CHECK(sv(JsValue(JsArray{JsValue(JsNull{}), JsValue(JsUndefined{}), JsValue(1)})) == ",,1",
          "null/undefined elements join empty");

    CHECK(morph::js_to_number().as_string() == "0", "Number() is 0");
    CHECK(morph::js_to_number(JsValue("")).as_string() == "0", "Number('') is 0");
    CHECK(morph::js_to_number(JsValue(" ")).as_string() == "0", "Number(' ') is 0");
    CHECK(morph::js_to_number(JsValue("0x10")).as_string() == "16", "hex literal");
    CHECK(morph::js_to_number(JsValue("0b101")).as_string() == "5", "binary literal");
    CHECK(morph::js_to_number(JsValue("0o17")).as_string() == "15", "octal literal");
    CHECK(morph::js_to_number(JsValue("3.14")).as_string() == "3.14", "decimal parse");
    CHECK(std::isnan(morph::js_to_number(JsValue("abc")).as_double()), "garbage is NaN");
    CHECK(morph::js_to_number(JsValue(JsNull{})).as_string() == "0", "null is 0");
    CHECK(std::isnan(morph::js_to_number(JsValue(JsUndefined{})).as_double()),
          "undefined is NaN");
    CHECK(morph::js_to_number(JsValue(JsArray{})).as_string() == "0", "empty array is 0");
    CHECK(morph::js_to_number(JsValue(JsArray{5})).as_string() == "5", "single item array");
    CHECK(morph::js_to_number(JsValue(JsArray{JsArray{5}})).as_string() == "5",
          "nested single item array");
    CHECK(std::isnan(morph::js_to_number(JsValue(JsArray{1, 2})).as_double()),
          "multi item array is NaN");
    CHECK(morph::js_to_number(JsValue("  42  ")).as_string() == "42", "trims whitespace");
    CHECK(morph::js_to_number(JsValue("1e3")).as_string() == "1000", "exponent text");

    CHECK(morph::js_to_boolean() == false, "Boolean() is false");
    CHECK(morph::js_to_boolean(JsValue("")) == false, "empty string falsy");
    CHECK(morph::js_to_boolean(JsValue("x")) == true, "non-empty string truthy");
    CHECK(morph::js_to_boolean(JsValue(0)) == false, "zero falsy");
    CHECK(morph::js_to_boolean(morph::js_to_number(JsValue("abc"))) == false, "NaN falsy");
    CHECK(morph::js_to_boolean(JsValue(JsUndefined{})) == false, "undefined falsy");
    CHECK(morph::js_to_boolean(JsValue(JsNull{})) == false, "null falsy");
    CHECK(morph::js_to_boolean(JsValue(JsArray{})) == true, "empty array truthy");
}

static void testParseFunctions()
{
    CHECK(morph::js_parse_int(JsValue("42px")).as_string() == "42", "parseInt stops at junk");
    CHECK(morph::js_parse_int(JsValue("101"), 2).as_string() == "5", "parseInt radix 2");
    CHECK(morph::js_parse_int(JsValue("0x1F")).as_string() == "31", "parseInt hex prefix");
    CHECK(morph::js_parse_int(JsValue("08")).as_string() == "8", "legacy octal lookalike");
    CHECK(morph::js_parse_int(JsValue("z"), 36).as_string() == "35", "parseInt radix 36");
    CHECK(std::isnan(morph::js_parse_int(JsValue("")).as_double()), "parseInt empty is NaN");
    CHECK(morph::js_parse_int(JsValue("12.9")).as_string() == "12", "parseInt truncates");
    CHECK(morph::js_parse_int(JsValue("+5")).as_string() == "5", "parseInt plus sign");
    CHECK(morph::js_parse_int(JsValue("-7")).as_string() == "-7", "parseInt minus sign");
    CHECK(morph::js_parse_float(JsValue("3.14abc")).as_string() == "3.14",
          "parseFloat stops at junk");
    CHECK(morph::js_parse_float(JsValue(".5")).as_string() == "0.5", "parseFloat leading dot");
    CHECK(morph::js_parse_float(JsValue("1e3x")).as_string() == "1000", "parseFloat exponent");
    CHECK(morph::js_parse_float(JsValue("Infinity")).as_double() ==
              std::numeric_limits<double>::infinity(),
          "parseFloat Infinity");
    CHECK(std::isnan(morph::js_parse_float(JsValue("abc")).as_double()),
          "parseFloat garbage is NaN");
}

static void testPredicates()
{
    CHECK(morph::js_is_finite(morph::js_to_number(JsValue(1))) == true, "isFinite(1)");
    CHECK(morph::js_is_finite(morph::js_to_number(
              std::numeric_limits<double>::infinity())) == false,
          "isFinite(Infinity)");
    CHECK(morph::js_is_finite(JsValue("1")) == false, "isFinite does not coerce");
    CHECK(morph::js_is_nan(morph::js_to_number(JsValue("abc"))) == true, "isNaN(NaN)");
    CHECK(morph::js_is_nan(morph::js_to_number(JsValue(1))) == false, "isNaN(1)");
    CHECK(morph::js_is_nan(JsValue("x")) == false, "isNaN does not coerce");
    CHECK(morph::js_is_integer(morph::js_to_number(JsValue(1.5))) == false, "isInteger(1.5)");
    CHECK(morph::js_is_integer(morph::js_to_number(JsValue(2))) == true, "isInteger(2)");
    CHECK(morph::js_is_integer(morph::js_to_number(JsValue(-0.0))) == true, "isInteger(-0)");
}

static void testIsArray()
{
    CHECK(morph::js_is_array(JsValue(JsArray{})) == true, "isArray on JsValue array");
    CHECK(morph::js_is_array(JsArray{}) == true, "isArray on JsArray");
    std::shared_ptr<JsArray> pa = std::make_shared<JsArray>(JsArray{1});
    CHECK(morph::js_is_array(pa) == true, "isArray on shared_ptr<JsArray>");
    CHECK(morph::js_is_array(JsValue("x")) == false, "isArray on string");
    CHECK(morph::js_is_array(42) == false, "isArray on number");
}

static void testArrayMethods()
{
    JsArray a{1, 2, 3};
    CHECK(a.join(JsValue("-")).value == "1-2-3", "join with separator");
    CHECK(a.join().value == "1,2,3", "join default separator");
    CHECK((JsArray{JsValue(JsNull{}), JsValue(1)}.join(JsValue("-")).value == "-1"),
          "null joins empty");
    CHECK((JsArray{JsArray{1, 2}, 3}.join(JsValue("|")).value == "1,2|3"),
          "nested array element joins with comma");
    CHECK(a.toString().value == "1,2,3", "array toString");

    CHECK(a.slice(1).join().value == "2,3", "slice from index");
    CHECK(a.slice(-2).join().value == "2,3", "slice negative start");
    CHECK(a.slice(1, 2).join().value == "2", "slice range");
    CHECK(a.slice(5).length() == 0, "slice past end is empty");
    CHECK(a.length() == 3, "slice does not mutate");

    CHECK(a.concat(JsArray{4}).join().value == "1,2,3,4", "concat array");
    CHECK(a.concat({5, 6}).join().value == "1,2,3,5,6", "concat values");
    CHECK(a.concat(JsArray{JsArray{4, 5}}).join().value == "1,2,3,4,5",
          "concat spreads one level");
    std::shared_ptr<JsArray> pa = std::make_shared<JsArray>(JsArray{4});
    CHECK(a.concat(pa).join().value == "1,2,3,4", "concat shared_ptr array");

    CHECK(a.includes(JsValue(2)) == true, "includes found");
    CHECK(a.includes(JsValue(9)) == false, "includes missing");
    CHECK(a.includes(JsValue(2), JsValue(-2)) == true, "includes negative from");
    CHECK(a.includes(JsValue(2), JsValue(2)) == false, "includes from past hit");
    CHECK(a.includes(JsValue("2")) == false, "includes is strict");
    CHECK(a.indexOf(JsValue(3)) == 2, "indexOf found");
    CHECK(a.indexOf(JsValue(9)) == -1, "indexOf missing");
    CHECK(a.indexOf(JsValue(1), JsValue(1)) == -1, "indexOf from");
    JsArray r{1, 2, 1};
    CHECK(r.lastIndexOf(JsValue(1)) == 2, "lastIndexOf found");
    CHECK(r.lastIndexOf(JsValue(1), JsValue(1)) == 0, "lastIndexOf from");

    CHECK(sv(a.at(-1)) == "3", "at negative");
    CHECK(a.at(9).is_undefined(), "at out of range is undefined");
    CHECK(a.at(-4).is_undefined(), "at too negative is undefined");

    JsArray rev{1, 2, 3};
    rev.reverse();
    CHECK(rev.join().value == "3,2,1", "reverse mutates in place");

    CHECK(JsValue("abc").includes(JsValue("b")) == true, "JsValue string includes");
    CHECK(JsValue("abc").includes(JsValue("d")) == false, "JsValue string includes miss");
}

static void testObjectMethods()
{
    JsObject one;
    one.set("x", JsValue(9));
    CHECK(JsValue(one).keys().size() == 1 && JsValue(one).keys()[0] == "x", "Object.keys");
    CHECK(JsValue(one).values().join().value == "9", "Object.values");
    CHECK(JsValue(one).entries().join(JsValue(";")).value == "x,9", "Object.entries");

    // Enumeration is sorted by key (kill-runtime-strings decision), not
    // JS insertion order: {b:1,a:2} enumerates a before b.
    JsObject two{{"b", JsValue(1)}, {"a", JsValue(2)}};
    CHECK(JsValue(two).sorted_keys() == std::vector<std::string>({"a", "b"}),
          "sorted_keys orders keys");
    CHECK(JsValue(two).values().join().value == "2,1", "values follow sorted enumeration");
    CHECK(JsValue(two).entries().join(JsValue(";")).value == "a,2;b,1",
          "entries follow sorted enumeration");
    CHECK(sv(morph::json_stringify(JsValue(two))) == "{\"a\":2,\"b\":1}",
          "stringify follows the runtime's sorted enumeration");

    // JS drops undefined-valued properties from objects but renders them
    // as null inside arrays (the array case is pinned above).
    JsObject holes{{"a", JsValue(1)}, {"b", JsValue(JsUndefined{})}};
    CHECK(sv(morph::json_stringify(JsValue(holes))) == "{\"a\":1}",
          "undefined object properties are omitted");
}

static void testJson()
{
    JsValue parsed = morph::json_parse("{\"x\":1,\"y\":[2,3]}");
    CHECK(sv(morph::json_stringify(parsed)) == "{\"x\":1,\"y\":[2,3]}", "round trip");
    CHECK(sv(morph::json_stringify(JsValue(0.1))) == "0.1", "shortest number text");
    CHECK(sv(morph::json_stringify(JsValue(1e21))) == "1e+21", "exponent number text");
    CHECK(sv(morph::json_stringify(JsValue(-0.0))) == "0", "-0 stringifies as 0");
    CHECK((sv(morph::json_stringify(JsValue(JsArray{JsValue(1), JsValue(JsUndefined{})}))) ==
              "[1,null]"),
          "undefined array element is null");
    CHECK(morph::json_stringify(JsValue(JsUndefined{})).is_undefined(),
          "top-level undefined yields undefined");
    CHECK(sv(morph::json_stringify(JsValue("a\"b\n"))) == "\"a\\\"b\\n\"", "escapes");
    CHECK(sv(morph::json_stringify(JsValue(true))) == "true", "booleans");
    CHECK(sv(morph::json_stringify(JsValue(JsNull{}))) == "null", "null");

    CHECK(morph::json_parse("true").truthy() == true, "parse true");
    CHECK(sv(morph::json_parse("1.5e3")) == "1500", "parse exponent number");
    CHECK(sv(morph::json_parse("\"hi\"")) == "hi", "parse string");
    CHECK(sv(morph::json_parse("-3")) == "-3", "parse negative int");
    CHECK(morph::json_parse("null").is_null(), "parse null");
    CHECK(sv(morph::json_parse("\"\\u0041\"")) == "A", "parse unicode escape");
    CHECK(sv(morph::json_parse("\"\\ud83d\\ude00\"")) == "\xF0\x9F\x98\x80",
          "parse surrogate pair to UTF-8");
    CHECK(sv(morph::json_parse("[1] ")) == "1", "trailing whitespace ok");

#if defined(__cpp_exceptions)
    try
    {
        (void)morph::json_parse("{bad}");
        CHECK(false, "parse {bad} throws");
    }
    catch (const JsValue& e)
    {
        CHECK(sv(e["name"]) == "SyntaxError", "parse {bad} SyntaxError");
    }
    try
    {
        (void)morph::json_parse("01");
        CHECK(false, "parse 01 throws");
    }
    catch (const JsValue& e)
    {
        CHECK(sv(e["name"]) == "SyntaxError", "parse 01 SyntaxError");
    }
    try
    {
        (void)morph::json_parse("[1,");
        CHECK(false, "parse [1, throws");
    }
    catch (const JsValue& e)
    {
        CHECK(sv(e["name"]) == "SyntaxError", "parse [1, SyntaxError");
    }
#endif
}

static void testMath()
{
    using namespace morph::js_math;
    CHECK(sv(max({1, 9, 3})) == "9", "max over list");
    CHECK(sv(min({1, 9, 3})) == "1", "min over list");
    CHECK(sv(max({})) == "-Infinity", "empty max");
    CHECK(sv(min({})) == "Infinity", "empty min");
    CHECK(sv(max({1, std::nan("")})) == "NaN", "NaN poisons max");
    CHECK(sv(abs(JsValue(-5))) == "5", "abs int");
    CHECK(sv(abs(JsValue(-5.5))) == "5.5", "abs double");
    CHECK(sv(abs(JsValue("-3"))) == "3", "abs coerces strings");
    CHECK(sv(abs(JsValue("x"))) == "NaN", "abs garbage is NaN");
    CHECK(sv(floor(JsValue(2.9))) == "2", "floor");
    CHECK(sv(floor(JsValue(-2.1))) == "-3", "floor negative");
    CHECK(sv(ceil(JsValue(2.1))) == "3", "ceil");
    CHECK(sv(ceil(JsValue(-2.9))) == "-2", "ceil negative");
    CHECK(sv(round(JsValue(2.5))) == "3", "round half up");
    CHECK(sv(round(JsValue(-2.5))) == "-2", "round half toward +inf");
    CHECK(sv(round(JsValue(-0.5))) == "0", "round -0.5 is zero");
    CHECK(sv(pow(JsValue(2), JsValue(10))) == "1024", "pow");
    CHECK(sv(pow(JsValue(2), JsValue(-1))) == "0.5", "pow negative exponent");
    CHECK(sv(pow(JsValue(-1), JsValue(0.5))) == "NaN", "pow fractional NaN");
    CHECK(sv(sqrt(JsValue(2))) == "1.4142135623730951", "sqrt shortest form");
    CHECK(sv(sqrt(JsValue(-1))) == "NaN", "sqrt negative is NaN");
    CHECK(sv(sqrt(JsValue(16))) == "4", "sqrt perfect square");

    // Spread lowerings: Math.max(...arr) and mutated (shared_ptr) bindings.
    CHECK(sv(max(JsArray{4, 1, 7})) == "7", "max single-spread form");
    std::shared_ptr<JsArray> sp = std::make_shared<JsArray>(JsArray{4, 1, 7});
    CHECK(sv(max(sp)) == "7", "max spread over shared_ptr array");
    CHECK(sv(min(sp)) == "1", "min spread over shared_ptr array");
}

int main()
{
    testNumberToString();
    testToFixed();
    testPrecisionFormats();
    testRangeErrors();
    testConversions();
    testParseFunctions();
    testPredicates();
    testIsArray();
    testArrayMethods();
    testObjectMethods();
    testJson();
    testMath();
    std::printf("[js-types-test] %d checks, %d failures\n", checks, failures);
    return failures == 0 ? 0 : 1;
}
