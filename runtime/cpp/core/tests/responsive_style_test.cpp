// Width-based @media variant selection (headless, no window).
//
// Mirrors what generated code now emits: each breakpoint variant is a full
// MorphStyle, and the runtime reselects one when the root layout width
// crosses a boundary.
#include "../node.h"
#ifdef MORPH_FEATURE_OUTLINE
#include "../../ui/border_paint.h"
#endif

#include <cmath>
#include <cstdio>

static int failures = 0;
static int checks = 0;

struct TestNode : MorphNode
{
    void draw(Renderer&) override {}
    void layout(float, float, float, float, Renderer* = nullptr) override {}
    void update(float) override {}
    float contentWidth(Renderer*) override { return 0.0f; }
    void layoutIfNeeded(float, float, float, float, Renderer* = nullptr,
                        DirtyStats* = nullptr, bool = false) override {}
    void recordDisplayList(Renderer&) override {}
    void executeDisplayList(Renderer&) override {}
    int flatten(RenderFrame&, int, float = 0.0f, float = 0.0f) override { return 0; }
    int flattenExtra(RenderFrame&, FlatRenderNode&) override { return 0; }
};

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

static bool near(float a, float b) { return std::fabs(a - b) < 1e-3f; }

int main()
{
    TestNode root;

    MediaStyle narrow;
    narrow.minWidth = 0;
    narrow.maxWidth = 600;
    narrow.style.explicitWidth = pxLen(160.0f);
    narrow.style.bgColor[0] = 0.5f;
    narrow.style.bgColor[3] = 1.0f;
#ifdef MORPH_FEATURE_OUTLINE
    narrow.style.outlineSet = true;
    narrow.style.outlineStyle = CSS::BorderStyle::Dashed;
    narrow.style.outlineWidth = pxLen(4.0f);
#endif
    narrow.style.fontSize = pxLen(20.0f);
    narrow.style.letterSpacingSet = true;
    narrow.style.letterSpacing = pxLen(2.0f);
    narrow.colorInherited = true;
    narrow.colorOverridden = false;
#ifdef MORPH_FEATURE_TEXT_SHADOW
    narrow.style.textShadowSet = true;
    narrow.style.textShadowCount = 1;
    narrow.style.textShadows[0].offsetX = pxLen(1.0f);
    narrow.style.textShadows[0].offsetY = pxLen(2.0f);
    narrow.style.textShadows[0].blur = pxLen(3.0f);
    narrow.style.textShadows[0].hasColor = true;
    narrow.style.textShadows[0].color[0] = 0.25f;
    narrow.style.textShadows[0].color[3] = 1.0f;
#endif

    MediaStyle wide;
    wide.minWidth = 601;
    wide.maxWidth = -1;
    wide.style.explicitWidth = pxLen(420.0f);
    wide.style.bgColor[1] = 0.5f;
    wide.style.bgColor[3] = 1.0f;
    wide.style.fontSize = pxLen(30.0f);
    wide.style.letterSpacingSet = true;
    wide.style.letterSpacing = pxLen(6.0f);
    wide.colorInherited = false;
    wide.colorOverridden = true;

    root.mediaStyles.push_back(narrow);
    root.mediaStyles.push_back(wide);

    // Text nodes leave font-size unset and inherit the nearest resolved
    // ancestor; a variant swap must invalidate their cached computed size.
    auto* text = new TestNode();
    root.addChild(text);

    root.applyResponsiveStyles(500.0f);
    CHECK(near(root.style.explicitWidth.value, 160.0f), "narrow width selected");
    CHECK(near(root.style.bgColor[0], 0.5f), "narrow background selected");
#ifdef MORPH_FEATURE_OUTLINE
    CHECK(root.style.outlineSet && root.style.outlineStyle == CSS::BorderStyle::Dashed,
          "outline survives responsive selection");
    CHECK(near(root.style.outlineWidth.value, 4.0f), "outline width selected");
#endif
    CHECK(near(root.style.letterSpacing.value, 2.0f), "letter spacing selected");
    CHECK(root.m_colorInherited, "narrow inherited color marker selected");
    CHECK(!root.m_colorOverridden, "narrow text color marker cleared");
    CHECK(near(text->resolvedFontSize(nullptr), 20.0f), "child inherits narrow font");
#ifdef MORPH_FEATURE_TEXT_SHADOW
    CHECK(root.style.textShadowSet && root.style.textShadowCount == 1,
          "text shadow selected");
    CHECK(near(root.style.textShadows[0].offsetY.value, 2.0f),
          "text shadow offsets selected");
#endif

    root.applyResponsiveStyles(600.0f);
    CHECK(near(root.style.explicitWidth.value, 160.0f), "600px is inclusive narrow bound");

    root.applyResponsiveStyles(601.0f);
    CHECK(near(root.style.explicitWidth.value, 420.0f), "wide width selected at boundary");
    CHECK(near(root.style.bgColor[1], 0.5f), "wide background selected");
    CHECK(near(root.style.letterSpacing.value, 6.0f), "wide letter spacing selected");
    CHECK(!root.m_colorInherited, "wide explicit color marker selected");
    CHECK(root.m_colorOverridden, "wide text color marker selected");
    CHECK(near(text->resolvedFontSize(nullptr), 30.0f), "child re-inherits wide font");

    root.applyResponsiveStyles(1600.0f);
    CHECK(near(root.style.explicitWidth.value, 420.0f), "unbounded wide range persists");

    // currentcolor resolution skips baked inherited colors.
    auto* colorParent = new TestNode();
    colorParent->style.color[0] = 1.0f;
    colorParent->style.color[1] = 0.25f;
    colorParent->style.color[3] = 1.0f;
    auto* colorChild = new TestNode();
    colorChild->style.color[0] = 0.0f;
    colorChild->style.color[1] = 0.0f;
    colorChild->style.color[2] = 0.0f;
    colorChild->style.color[3] = 1.0f;
    colorChild->m_colorInherited = true;
    colorParent->addChild(colorChild);
    const float* inherited = colorChild->effectiveColor();
    CHECK(near(inherited[0], 1.0f) && near(inherited[1], 0.25f),
          "currentcolor walks past inherited hops");

#ifdef MORPH_FEATURE_OUTLINE
    // Patterned outlines must become per-side rings: the shader cannot
    // pattern a full-ring op because it has no per-edge run length.
    MorphStyle dashedOutline;
    dashedOutline.outlineSet = true;
    dashedOutline.outlineStyle = CSS::BorderStyle::Dashed;
    dashedOutline.outlineWidth = pxLen(4.0f);
    dashedOutline.outlineOffset = pxLen(6.0f);
    std::vector<DrawOp> dashedOps;
    UnitEnv env;
    recordOutlineOps(dashedOps, dashedOutline, env, 10.0f, 20.0f, 200.0f,
                     80.0f, false, nullptr);
    CHECK(dashedOps.size() == 4, "dashed outline records four side rings");
    if (dashedOps.size() == 4)
    {
        bool modesOk = true;
        for (int i = 0; i < 4; i++)
        {
            float expected = (float)((i + 1) * 16 + (int)CSS::BorderStyle::Dashed);
            modesOk = modesOk && dashedOps[(size_t)i].type == DrawOp::BorderRing &&
                      near(dashedOps[(size_t)i].data[2], expected);
        }
        CHECK(modesOk, "dashed outline side modes pack side + style");
    }
    MorphStyle solidOutline = dashedOutline;
    solidOutline.outlineStyle = CSS::BorderStyle::Solid;
    std::vector<DrawOp> solidOps;
    recordOutlineOps(solidOps, solidOutline, env, 10.0f, 20.0f, 200.0f,
                     80.0f, false, nullptr);
    CHECK(solidOps.size() == 1 && near(solidOps[0].data[2], 1.0f),
          "solid outline keeps the full-ring fast path");
#endif

    std::printf("[responsive-style-test] %d checks, %d failures\n", checks, failures);
    return failures == 0 ? 0 : 1;
}
