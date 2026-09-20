#include "signal.h"
#include <mutex>
#include <vector>

namespace morph {

thread_local EffectContext g_effect_ctx;
thread_local MountScopeState g_mount_scope;

// Global effect pool + pending queue
static std::vector<EffectNode*> s_pool;
static std::vector<EffectNode*> s_pending;
static std::mutex s_pending_mutex;

void SignalBase::notify_all() {
    // Caller (set()) already holds m_mutex — no deadlock
    std::lock_guard<std::mutex> lock(s_pending_mutex);
    for (auto* sub : subscribers_) {
        if (!sub->pending) {
            sub->pending = true;
            s_pending.push_back(sub);
        }
    }
}

EffectNode* create_effect(std::function<void()> fn) {
    auto* node = new EffectNode();
    node->fn = std::move(fn);
    {
        std::lock_guard<std::mutex> lock(s_pending_mutex);
        s_pool.push_back(node);
    }
    node->run();
    return node;
}

void EffectNode::cleanup() {
    for (auto& dep : deps) {
        std::lock_guard<std::mutex> lock(dep.sig->m_mutex);
        dep.sig->unsubscribe(this);
    }
    deps.clear();
}

void EffectNode::run() {
    if (cleanup_fn) {
        auto tmp = std::move(cleanup_fn);
        cleanup_fn = nullptr;
        tmp();
    }
    cleanup();
    g_effect_ctx.current = this;
    fn();
    g_effect_ctx.current = nullptr;
    pending = false;
}

void run_pending_effects() {
    // Swap under lock so effects can re-enqueue during run()
    std::vector<EffectNode*> batch;
    {
        std::lock_guard<std::mutex> lock(s_pending_mutex);
        batch.swap(s_pending);
    }
    if (batch.empty()) return;
    for (auto* node : batch) {
        if (!node->dead) {
            node->run();
        }
    }
    // Any effects that were enqueued during runs are in s_pending
    // Drain them too (recursive, but finite because pending flag is cleared in run())
    if (!s_pending.empty()) {
        run_pending_effects();
    }
}

void destroy_effect(EffectNode* node) {
    if (!node)
        return;
    // Mark dead first (the run path skips dead nodes), then release the
    // lock before cleanup: cleanup takes per-signal locks, and set()
    // already holds a signal lock when taking the pool lock — nesting
    // them the other way round would invert the lock order.
    {
        std::lock_guard<std::mutex> lock(s_pending_mutex);
        node->dead = true;
    }
    if (node->cleanup_fn) {
        auto fn = std::move(node->cleanup_fn);
        node->cleanup_fn = nullptr;
        fn();
    }
    node->cleanup();
    {
        std::lock_guard<std::mutex> lock(s_pending_mutex);
        for (size_t i = 0; i < s_pool.size(); i++) {
            if (s_pool[i] == node) {
                s_pool[i] = s_pool.back();
                s_pool.pop_back();
                break;
            }
        }
        for (size_t i = 0; i < s_pending.size(); i++) {
            if (s_pending[i] == node) {
                s_pending[i] = s_pending.back();
                s_pending.pop_back();
                break;
            }
        }
    }
    delete node;
}

void destroy_all_effects() {
    std::lock_guard<std::mutex> lock(s_pending_mutex);
    for (auto* node : s_pool) {
        if (node->cleanup_fn) node->cleanup_fn();
        node->cleanup();
        delete node;
    }
    s_pool.clear();
    s_pending.clear();
}

} // namespace morph