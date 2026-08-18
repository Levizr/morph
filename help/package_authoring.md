# Writing a Morph Package

**Status: Package CLI (`morph pkg add/remove/install/list/search`) works. JS/C++ integration pipeline is stubbed — packages can be downloaded but not yet compiled into the binary.**

## Required Files

```
my-package/
├── morph.pkg.json
├── js/index.js
└── runtime/renderer.h
```

## morph.pkg.json

```json
{
  "name": "my-package",
  "version": "1.0.0",
  "type": "morph-native",
  "js_entry": "js/index.js",
  "runtime_headers": ["runtime/renderer.h"],
  "github": "you/my-package"
}
```

## JS API Annotations

```js
export class MyWidget {
    // @morph-component: MyWidgetNode
    // @morph-header: runtime/renderer.h
    constructor(props) { ... }
}
```

## C++ Node

```cpp
#include <morph/morph_node.h>

class MyWidgetNode : public MorphNode {
public:
    void draw(Renderer& r) override { ... }
};
```

## Current Implementation

| Component | Status |
|---|---|---|
| `morph pkg add <name>` — download + extract | ✅ |
| Registry fetch (registry.levizr.com/morph) | ✅ |
| Manifest parsing (`morph.pkg.json`) | ✅ |
| `morph pkg list` — show installed | ✅ |
| `morph pkg install` — restore from config | ✅ |
| `morph pkg remove` | ✅ |
| `morph pkg search <query>` — registry search | ✅ |
| Dependency resolver (versions, topological sort) | ⚠️ Stub |
| JS → C++ bridge at build time | ❌ Not implemented |
