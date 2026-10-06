// Border hover/active/transition/keyframe state test (headless, no window).
//
// Drives the real MorphNode state machinery (onHover/onActive,
// updateStateTransition, updateCssAnimations) against per-side border
// widths/colors, corner radii and border-image gradients, plus the
// BorderColor/BorderWidth/BorderGradient keyframe properties.
#include "../node.h"

#include <cmath>
#include <cstdio>

#if !defined(MORPH_FEATURE_BORDER) || !defined(MORPH_FEATURE_GRADIENT) || \
    !defined(MORPH_FEATURE_ANIMATION)
#error "border_state_test requires BORDER, GRADIENT and ANIMATION features"
#endif

static int failures = 0;
static int checks = 0;

// Concrete node with rendering/traversal stripped out: only the style
// state machinery (hover/active/transitions/keyframes) is exercised.
struct TestNode : MorphNode
{
    void draw(Renderer&) override
    {
    }
    void layout(float, float, float, float, Renderer* = nullptr) override
    {
    }
    void update(float) override
    {
    }
    float contentWidth(Renderer*) override
    {
        return 0.0f;
    }
    void layoutIfNeeded(float, float, float, float, Renderer* = nullptr, DirtyStats* = nullptr,
                        bool = false) override
    {
    }
    void recordDisplayList(Renderer&) override
    {
    }
    void executeDisplayList(Renderer&) override
    {
    }
    int flatten(RenderFrame&, int, float = 0.0f) override
    {
        return 0;
    }
    int flattenExtra(RenderFrame&, FlatRenderNode&) override
    {
        return 0;
    }
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

static bool near(float a, float b)
{
    return std::fabs(a - b) < 1e-3f;
}

static bool colorNear(const float* a, const float* b)
{
    return near(a[0], b[0]) && near(a[1], b[1]) && near(a[2], b[2]) &&
           near(a[3], b[3]);
}

static void setColor(float* c, float r, float g, float b, float a)
{
    c[0] = r;
    c[1] = g;
    c[2] = b;
    c[3] = a;
}

// Shorthand hover swap snaps when no transition duration is set.
static void testHoverShorthand()
{
    TestNode node;
    setColor(node.style.borderColor, 1.0f, 1.0f, 1.0f, 1.0f);
    node.style.borderWidth = 4.0f;
    node.hoverStyle = new MorphStyle();
    setColor(node.hoverStyle->borderColor, 0.13f, 0.83f, 0.93f, 1.0f);
    node.hoverStyle->borderWidth = 8.0f;

    node.onHover(true);
    const float cyan[4] = {0.13f, 0.83f, 0.93f, 1.0f};
    CHECK(colorNear(node.style.borderColor, cyan), "hover applies borderColor");
    CHECK(near(node.style.borderWidth, 8.0f), "hover applies borderWidth");
    node.onHover(false);
    const float white[4] = {1.0f, 1.0f, 1.0f, 1.0f};
    CHECK(colorNear(node.style.borderColor, white), "release restores borderColor");
    CHECK(near(node.style.borderWidth, 4.0f), "release restores borderWidth");
}

// Per-side widths/colors and corner radii swap and revert.
static void testHoverPerSide()
{
    TestNode node;
    node.hoverStyle = new MorphStyle();
    setColor(node.hoverStyle->borderTopColor, 1.0f, 0.0f, 0.0f, 1.0f);
    node.hoverStyle->borderTopWidth = 6.0f;
    node.hoverStyle->borderTopLeftRadius = 24.0f;

    node.onHover(true);
    const float red[4] = {1.0f, 0.0f, 0.0f, 1.0f};
    CHECK(colorNear(node.style.borderTopColor, red), "hover applies side color");
    CHECK(near(node.style.borderTopWidth, 6.0f), "hover applies side width");
    CHECK(near(node.style.borderTopLeftRadius, 24.0f), "hover applies corner");
    CHECK(node.style.borderRightWidth < 0.0f, "untouched side stays unset");
    node.onHover(false);
    CHECK(node.style.borderTopColor[0] < 0.0f, "release restores side color");
    CHECK(node.style.borderTopWidth < 0.0f, "release restores side width");
    CHECK(node.style.borderTopLeftRadius < 0.0f, "release restores corner");
}

// Active swaps shorthand color plus style; release reverts both.
static void testActiveSwap()
{
    TestNode node;
    setColor(node.style.borderColor, 1.0f, 1.0f, 1.0f, 1.0f);
    node.style.borderStyle = CSS::BorderStyle::Solid;
    node.activeStyle = new MorphStyle();
    setColor(node.activeStyle->borderColor, 0.66f, 0.33f, 0.97f, 1.0f);
    node.activeStyle->borderStyle = CSS::BorderStyle::Dashed;

    node.onActive(true);
    const float purple[4] = {0.66f, 0.33f, 0.97f, 1.0f};
    CHECK(colorNear(node.style.borderColor, purple), "active applies borderColor");
    CHECK(node.style.borderStyle == CSS::BorderStyle::Dashed, "active applies style");
    node.onActive(false);
    const float white[4] = {1.0f, 1.0f, 1.0f, 1.0f};
    CHECK(colorNear(node.style.borderColor, white), "release restores color");
    CHECK(node.style.borderStyle == CSS::BorderStyle::Solid, "release restores style");
}

// Transitions interpolate per-side color/width and corners linearly.
static void testTransitionLerp()
{
    TestNode node;
    setColor(node.style.borderColor, 0.0f, 0.0f, 0.0f, 1.0f);
    node.style.borderWidth = 4.0f;
    node.m_transitionDuration = 1.0f;
    node.m_transitionEasing = Easing::Linear;
    node.hoverStyle = new MorphStyle();
    setColor(node.hoverStyle->borderColor, 1.0f, 1.0f, 1.0f, 1.0f);
    node.hoverStyle->borderWidth = 8.0f;
    setColor(node.hoverStyle->borderTopColor, 1.0f, 0.0f, 0.0f, 1.0f);
    node.hoverStyle->borderTopLeftRadius = 20.0f;

    node.onHover(true);
    node.updateStateTransition(0.5f);
    const float mid[4] = {0.5f, 0.5f, 0.5f, 1.0f};
    CHECK(colorNear(node.style.borderColor, mid), "transition lerps borderColor");
    CHECK(near(node.style.borderWidth, 6.0f), "transition lerps borderWidth");
    const float midRed[4] = {0.5f, 0.0f, 0.0f, 1.0f};
    CHECK(colorNear(node.style.borderTopColor, midRed), "transition lerps side color");
    CHECK(near(node.style.borderTopLeftRadius, 10.0f), "transition lerps corner");
    node.updateStateTransition(0.6f);
    const float white[4] = {1.0f, 1.0f, 1.0f, 1.0f};
    CHECK(colorNear(node.style.borderColor, white), "transition reaches target");
    CHECK(near(node.style.borderTopLeftRadius, 20.0f), "transition reaches corner");
    node.onHover(false);
    node.updateStateTransition(1.0f);
    const float black[4] = {0.0f, 0.0f, 0.0f, 1.0f};
    CHECK(colorNear(node.style.borderColor, black), "transition releases color");
    CHECK(node.style.borderTopColor[0] < 0.0f, "transition releases side color");
}

// Border-image gradient hover swaps the live gradient and reverts it.
static void testHoverGradient()
{
    TestNode node;
    node.style.borderWidth = 6.0f;
    node.hoverStyle = new MorphStyle();
    node.hoverStyle->borderImageEnabled = true;
    node.hoverStyle->borderImageIsGradient = true;
    node.hoverStyle->borderImageSlice = 1.0f;
    node.hoverStyle->borderGradientSet = true;
    node.hoverStyle->borderGradient.enabled = true;
    node.hoverStyle->borderGradient.stopCount = 2;
    setColor(node.hoverStyle->borderGradient.stops[0].color, 1.0f, 0.0f, 0.0f, 1.0f);
    setColor(node.hoverStyle->borderGradient.stops[1].color, 0.0f, 0.0f, 1.0f, 1.0f);

    node.onHover(true);
    CHECK(node.style.borderImageEnabled, "hover enables border image");
    CHECK(node.style.borderGradientSet, "hover sets border gradient");
    CHECK(node.style.borderGradient.stopCount == 2, "hover carries stops");
    node.onHover(false);
    CHECK(!node.style.borderImageEnabled, "release disables border image");
    CHECK(!node.style.borderGradientSet, "release clears border gradient");
}

// BorderColor keyframes interpolate and revert when finished.
static void testKeyframeBorderColor()
{
    morphClearKeyframes();
    morphAddKeyframe("borderpulse", 0.0f, {{KeyframeProperty::BorderColor, {1.0f, 0.0f, 0.0f, 1.0f}}});
    morphAddKeyframe("borderpulse", 1.0f, {{KeyframeProperty::BorderColor, {0.0f, 0.0f, 1.0f, 1.0f}}});
    TestNode node;
    setColor(node.style.borderColor, 1.0f, 1.0f, 1.0f, 1.0f);
    CssAnimation anim;
    anim.name = "borderpulse";
    anim.duration = 1.0f;
    anim.easing = Easing::Linear;
    anim.iterations = 1.0f;
    anim.fillMode = AnimFillMode::None;
    node.style.animations.push_back(anim);

    node.updateCssAnimations(0.5f);
    const float mid[4] = {0.5f, 0.0f, 0.5f, 1.0f};
    CHECK(colorNear(node.style.borderColor, mid), "keyframe lerps borderColor");
    node.updateCssAnimations(0.6f);
    const float white[4] = {1.0f, 1.0f, 1.0f, 1.0f};
    CHECK(colorNear(node.style.borderColor, white), "finished keyframe restores color");
    morphClearKeyframes();
}

// BorderWidth keyframes interpolate layout-affecting widths.
static void testKeyframeBorderWidth()
{
    morphClearKeyframes();
    morphAddKeyframe("bordergrow", 0.0f, {{KeyframeProperty::BorderWidth, {2.0f}}});
    morphAddKeyframe("bordergrow", 1.0f, {{KeyframeProperty::BorderWidth, {10.0f}}});
    TestNode node;
    node.style.borderWidth = 2.0f;
    CssAnimation anim;
    anim.name = "bordergrow";
    anim.duration = 1.0f;
    node.style.animations.push_back(anim);

    node.updateCssAnimations(0.5f);
    CHECK(near(node.style.borderWidth, 6.0f), "keyframe lerps borderWidth");
    node.updateCssAnimations(0.6f);
    CHECK(near(node.style.borderWidth, 2.0f), "finished keyframe restores width");
    morphClearKeyframes();
}

// BorderGradient keyframes sample the gradient css and set live flags.
static void testKeyframeBorderGradient()
{
    morphClearKeyframes();
    morphAddKeyframe("bordershift", 0.0f,
                     {{KeyframeProperty::BorderGradient,
                       {},
                       "linear-gradient(to right, #ff0000, #0000ff)"}});
    morphAddKeyframe("bordershift", 1.0f,
                     {{KeyframeProperty::BorderGradient,
                       {},
                       "linear-gradient(to right, #0000ff, #ff0000)"}});
    TestNode node;
    CssAnimation anim;
    anim.name = "bordershift";
    anim.duration = 1.0f;
    anim.easing = Easing::Linear;
    anim.iterations = 1.0f;
    anim.fillMode = AnimFillMode::Forwards;
    node.style.animations.push_back(anim);

    node.updateCssAnimations(0.5f);
    CHECK(node.style.borderGradientSet, "keyframe sets border gradient");
    CHECK(node.style.borderImageEnabled, "keyframe enables border image");
    CHECK(node.style.borderGradient.stopCount == 2, "keyframe parses stops");
    morphClearKeyframes();
}

// Border gradients interpolate when both sides set, else flip at
// the midpoint like browsers.
static void testTransitionGradient()
{
    MorphStyle a;
    MorphStyle b;
    b.borderGradientSet = true;
    b.borderGradient.enabled = true;
    b.borderGradient.stopCount = 2;
    setColor(b.borderGradient.stops[0].color, 1.0f, 0.0f, 0.0f, 1.0f);
    setColor(b.borderGradient.stops[1].color, 0.0f, 0.0f, 1.0f, 1.0f);
    MorphStyle out;
    MorphNode::interpolateStyles(out, a, b, 0.25f);
    CHECK(!out.borderGradientSet, "unset gradient holds before midpoint");
    MorphNode::interpolateStyles(out, a, b, 0.75f);
    CHECK(out.borderGradientSet, "unset gradient flips after midpoint");

    MorphStyle c;
    c.borderGradientSet = true;
    c.borderGradient.enabled = true;
    c.borderGradient.stopCount = 2;
    setColor(c.borderGradient.stops[0].color, 1.0f, 1.0f, 1.0f, 1.0f);
    setColor(c.borderGradient.stops[1].color, 1.0f, 1.0f, 1.0f, 1.0f);
    MorphNode::interpolateStyles(out, c, b, 0.5f);
    CHECK(out.borderGradientSet, "compatible gradients stay set");
    const float mid[4] = {1.0f, 0.5f, 0.5f, 1.0f};
    CHECK(colorNear(out.borderGradient.stops[0].color, mid), "gradients lerp stops");
}

int main()
{
    testHoverShorthand();
    testHoverPerSide();
    testActiveSwap();
    testTransitionLerp();
    testTransitionGradient();
    testHoverGradient();
    testKeyframeBorderColor();
    testKeyframeBorderWidth();
    testKeyframeBorderGradient();
    std::printf("[border-state-test] %d checks, %d failures\n", checks, failures);
    return failures == 0 ? 0 : 1;
}
