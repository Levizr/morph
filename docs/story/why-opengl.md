# Why OpenGL

**Part of:** [The Story of Morph](index.md)

Morph renders its entire UI — boxes, text, images, shapes — with OpenGL 3.3. Why OpenGL instead of a game engine, Skia, Cairo, or something else?

## It's easy — and that matters

OpenGL is simple to work with. You hand it vertices and textures and it draws them. There's no engine to learn, no scene graph, no huge framework deciding how your app works. For a solo project it's the difference between "I can build this" and "I'm fighting the tool." Easy was a feature, not a compromise.

## It's cross-platform

OpenGL runs on Windows, macOS, and Linux out of the box. One rendering codebase, no per-platform renderer to write. Vulkan and DirectX would mean separate code paths per OS — OpenGL meant I could build one renderer and ship it everywhere.

## Why 3.3 specifically — the 99% version

OpenGL 3.3 is old enough to be **everywhere** and new enough to do everything Morph needs.

- It runs on **almost every device made in the last 15+ years** — integrated graphics included
- It's supported by the default drivers on Windows, macOS, and Linux — no extra downloads, no GPU vendor setup
- Newer versions (4.x) add features Morph simply doesn't use — 3.3 gives the full core profile with a far bigger install base

The choice was literally: *the newest version that still works on nearly everything.* That's 3.3.

## The honest reason: it had to run on *my* machine

Here's the part I usually don't mention first: **I don't have a graphics card.**

I'm developing Morph on a second-gen Intel i5 processor — from around **2011–12** — with integrated graphics. No dedicated GPU. That machine is the floor:

- If it runs on a decade-and-a-half-old integrated chip
- If it stays smooth on hardware that was mid-range in 2012
- Then it will run on virtually anything anyone in the world actually has

**If it works for me, it works for the world.** That's not a slogan — it's the testing strategy. Old hardware is the best performance test there is, and Morph passed it.

## The GPU does the work

Layout is CPU-side (Morph's own engine), but the actual pixels are drawn on the GPU via batching — text runs, rectangles, and images grouped into a handful of draw calls per frame. That's what makes smooth scrolling and animations possible even on that old integrated chip.

## It keeps the binary tiny

No browser engine, no WebView, no huge UI framework. The renderer is a focused batch of GL calls. That's a big part of why Morph binaries are lean.

## The future

OpenGL is the v1 backend — the "it works everywhere" choice. The renderer is designed so the backend can be swapped: Vulkan, Metal, and DirectX are planned behind the same layout engine and API, selected automatically per platform. When they ship, Morph will get the newest APIs on machines that have them — while OpenGL 3.3 stays the floor that runs on everything. See [Graphics APIs](../future/graphics-api.md).