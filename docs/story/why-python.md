# Why Python First

**Part of:** [The Story of Morph](index.md)

People ask this a lot. The toolchain (parser, compiler, code generator) is Python — why not something "fast"?

The answer is in the [compiler page](../future/compiler.md) ("Why Python first — and why the move now"), but in short:

## Concept validation came first

At the start, the goal was proving the idea — *"can web syntax really be compiled down to a native binary?"* — not shipping the fastest compiler on earth. Python let me iterate on the pipeline (parse → IR → codegen) in hours, not days. Every wrong turn cost minutes, not a rewrite.

## No recompile loops

The toolchain is separate from the runtime. I can change the compiler and rebuild without touching the C++ side. That separation is what made the project possible to build alone — the two halves evolve independently.

## Text-data tooling

A compiler is mostly "source text → AST → IR → text". Python is genuinely excellent at that: tree-sitter bindings, dictionaries as a natural IR, string generation for codegen. It fit the job.

## It never ships

The most important point: **Python is build-time only**. It never runs in the app. The final binary is 100% C++ — no interpreter, no Python runtime, nothing. Using Python for the compiler costs you nothing at runtime.

## The catch

Python is slow at *compile time*, and as real projects get bigger, that starts to hurt — hot reload rebuilds the IR constantly, and interpreter startup + dict-heavy IR adds up. That's exactly why the roadmap moves the compiler to Rust (SWC/Oxc) for a native CLI ([Rust Compiler & Native CLI](../future/compiler.md)). But Python is the reason Morph exists at all — it was the fastest possible way to find out whether the idea could work.