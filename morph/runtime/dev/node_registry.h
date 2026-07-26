#pragma once
#include <cstdio>
#include <string>
#include <unordered_map>
#include "../core/node.h"

struct NodeRegistry {
    std::unordered_map<std::string, MorphNode*> nodes;

    void put(const std::string& id, MorphNode* node) {
        nodes[id] = node;
    }

    MorphNode* get(const std::string& id) const {
        auto it = nodes.find(id);
        return it != nodes.end() ? it->second : nullptr;
    }

    void clear() {
        nodes.clear();
    }

    size_t size() const {
        return nodes.size();
    }

    void debug_print() const {
        fprintf(stderr, " [");
        int count = 0;
        for (auto& [id, _] : nodes) {
            if (count++ > 0) fprintf(stderr, ",");
            // Only print first 5 and last few
            if (count <= 5 || nodes.size() - count <= 3) {
                fprintf(stderr, "%s", id.c_str());
            } else if (count == 6) {
                fprintf(stderr, "...");
            }
        }
        fprintf(stderr, "]");
    }
};
