// Reactive color parser test (headless): generated effects call
// morph::set_color with arbitrary CSS color strings. Transparent must clear the
// previous color instead of silently leaving stale state behind.
#include "../color.h"

#include <cmath>
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

static bool near(float a, float b)
{
    return std::fabs(a - b) < 1e-3f;
}

static bool equals(const float c[4], float r, float g, float b, float a)
{
    return near(c[0], r) && near(c[1], g) && near(c[2], b) && near(c[3], a);
}

int main()
{
    float c[4] = {0.2f, 0.3f, 0.4f, 1.0f};
    CHECK(morph::set_color(c, "transparent"), "transparent is valid");
    CHECK(equals(c, 0.0f, 0.0f, 0.0f, 0.0f), "transparent clears color");

    morph::set_color(c, "#14202f");
    CHECK(equals(c, 20.0f / 255.0f, 32.0f / 255.0f, 47.0f / 255.0f, 1.0f),
          "six-digit hex");

    morph::set_color(c, "#e8f0fe80");
    CHECK(equals(c, 232.0f / 255.0f, 240.0f / 255.0f, 254.0f / 255.0f, 128.0f / 255.0f),
          "eight-digit hex");

    morph::set_color(c, "#0af");
    CHECK(equals(c, 0.0f, 170.0f / 255.0f, 1.0f, 1.0f), "three-digit hex");

    morph::set_color(c, "#0af8");
    CHECK(equals(c, 0.0f, 170.0f / 255.0f, 1.0f, 136.0f / 255.0f), "four-digit hex");

    float before[4] = {0.1f, 0.2f, 0.3f, 0.4f};
    float invalid[4] = {0.1f, 0.2f, 0.3f, 0.4f};
    CHECK(!morph::set_color(invalid, "not-a-color"), "invalid color reports failure");
    CHECK(equals(invalid, before[0], before[1], before[2], before[3]),
          "invalid color leaves destination untouched");

    std::printf("[color-test] %d checks, %d failures\n", checks, failures);
    return failures == 0 ? 0 : 1;
}
