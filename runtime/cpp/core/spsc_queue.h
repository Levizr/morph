#pragma once
#include <atomic>
#include <cstddef>
#include <optional>

// Lock-free single-producer single-consumer queue
// Producer: push(). Consumer: pop(). Both wait-free.
template<typename T, size_t Capacity = 256>
class SPSCQueue {
    static_assert((Capacity & (Capacity - 1)) == 0, "Capacity must be power of 2");

    T m_buffer[Capacity];
    alignas(64) std::atomic<size_t> m_head{0};
    alignas(64) std::atomic<size_t> m_tail{0};

    size_t mask(size_t idx) const { return idx & (Capacity - 1); }

public:
    bool push(const T& item) {
        size_t tail = m_tail.load(std::memory_order_relaxed);
        size_t head = m_head.load(std::memory_order_acquire);
        if (mask(tail + 1) == mask(head)) return false;
        m_buffer[mask(tail)] = item;
        m_tail.store(tail + 1, std::memory_order_release);
        return true;
    }

    bool pop(T& item) {
        size_t head = m_head.load(std::memory_order_relaxed);
        size_t tail = m_tail.load(std::memory_order_acquire);
        if (head == tail) return false;
        item = m_buffer[mask(head)];
        m_head.store(head + 1, std::memory_order_release);
        return true;
    }

    std::optional<T> pop() {
        T item;
        if (pop(item)) return item;
        return std::nullopt;
    }
};
