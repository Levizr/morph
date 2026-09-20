// renderers/flash/flash.cpp
#include "flash/flash.h"
#include "../core/window.h"

void flash::flashCommit(MorphWindow& win)
{
    if (!win.hasRoot())
        return;

    // Phase 1-2: Layout + paint (GL context is already current on main thread)
    win.renderer().ensureReady();

    auto& stats = win.dirtyStats();
    stats.reset();
    win.root()->layoutIfNeeded(0.0f, 0.0f, win.contentWidth(), win.contentHeight(),
                               &win.renderer(), &stats);
    stats.fullTreeCount = countNodes(win.root());

#ifdef MORPH_FEATURE_DEV
    syncPaintDirtyTree(win.root());
#endif

    recordPaintTree(win.root(), win.renderer(), stats);

    // Phase 3: Flatten into render frame (no GL needed)
    auto& channel = win.frameChannel();
    int backIdx = channel.backIndex.load();
    RenderFrame &frame = channel.backFrames[backIdx];
    frame.nodes.clear();
    frame.drawOps.clear();
    frame.animations.clear();
    frame.textOps.clear();
    frame.culledCount = 0;
    frame.frameId++;
    auto now = std::chrono::steady_clock::now().time_since_epoch();
    frame.timestamp = std::chrono::duration<double>(now).count();

    frame.viewW = win.contentWidth();
    frame.viewH = win.contentHeight();

    win.root()->flatten(frame, -1);
    stats.culledCount = frame.culledCount;

    // Phase 4: Atomic swap — compositor will interpolate, then main thread renders
    channel.frontFrame.store(&frame, std::memory_order_release);
    channel.backIndex.store((backIdx + 1) % 2, std::memory_order_release);
    channel.framePending.store(true, std::memory_order_release);

    // This frame has been consumed; only re-render on a new dirty event.
    win.clearPendingRender();
}

void flash::flashPresent(MorphWindow& win)
{
    win.renderFrame();
}
