# Is Morph for Games or Apps?

**Part of:** [Questions & Answers](index.md) · [The Story of Morph](../story/index.md)

Apps.

Morph is a UI framework: windows, pages, forms, data-heavy interfaces, utilities, kiosks, tools. The layout engine is designed for document and application UI — flexbox, boxes, text, images — not physics, sprites, or 3D scenes.

If you want a game engine, that's a different tool (Godot, Unity, Unreal — or raw SDL/OpenGL if you like pain).

That said, apps increasingly embed 2D/3D content — maps, charts, product previews, visualizations. The planned `<morph-viewport>` element is exactly that: an embedded OpenGL canvas inside a normal Morph app where you can draw anything you want. See [Viewport (Native OpenGL Canvas)](../future/viewport.md).

So: **apps first, with a canvas for graphics-heavy content inside them.**