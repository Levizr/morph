# Writing a Morph Package

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
