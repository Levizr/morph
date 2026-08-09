// renderers/forge/forge.h
#pragma once

#include <functional>
#include "../core/window.h"

namespace forge {
    void forgeCommit(MorphWindow& win);
    // overlayFn is the devtools overlay draw callback (drawn on the default FB
    // after the damage-limited blit). Null unless the dev tools panel is active.
    void forgePresent(MorphWindow& win,
                      std::function<void(GLRenderer&, DirtyStats&)> overlayFn = {});
}