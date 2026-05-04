# Rustle — Project Proposal

## Team Members

- Mykhailo Zinenko, Ihor Poprushko

## Introduction

**Rustle** is a statically-typed scripting language designed for creative graphics programming. The core problem it addresses is the gap between general-purpose programming languages and creative coding tools: languages like Rust or C++ offer performance but are too verbose for quick visual experiments, while tools like Processing or p5.js lack static typing and the safety guarantees that come with it.

With Rustle, users write `.rustle` scripts and immediately see 2D shapes rendered live in a built-in editor. The language features a familiar C-like syntax, a static type system with compile-time error checking, and a simple lifecycle model (`on_init`, `on_update`, `on_exit`) that makes interactive scenes straightforward to build.

**What we hope to learn:**

- How to design and implement a programming language from scratch in Rust — lexing, parsing, type checking, and interpretation.
- How real-time rendering pipelines work by integrating a custom language runtime with a GPU-accelerated renderer (wgpu).
- How to build a desktop application with an integrated code editor using egui/eframe.
- How language design decisions (static typing, reference semantics, coordinate systems) affect usability in a creative coding context.

## Requirements

For the project to be considered successful, Rustle must deliver the following:

- **Language runtime (`rustle-lang`)** — A complete scripting language with:
  - Lexer, parser, and 3-pass static analysis (symbol collection, type checking, semantic validation)
  - Core types: `float`, `bool`, `string`, `vec2`, `vec3`, `vec4`, `color`, `mat3`, `mat4`, `list<T>`, `res<T>`
  - Control flow: `if`/`else if`/`else`, `while`, `for..in`, `match` with value matching
  - Functions, closures, and a namespace system for organizing built-in APIs
  - Shape primitives (`circle`, `rect`, `line`, `polygon`) with rendering modes (`fill`, `outline`, `stroke`, `sdf`)
  - A tree-walking interpreter with persistent state across frames
  - `null`/`none` value and type coercion / truthiness rules
  - String interpolation and string operations (`split`, `trim`, `contains`, `replace`, `len`, etc.)
  - Type conversions (`to_string()`, `to_float()`)
  - Structs (custom data types) and enums with `match` destructuring
  - Higher-order array operations (`map`, `filter`, `reduce`, `find`, `any`, `all`)
  - Input handling (mouse position, mouse buttons, keyboard)
  - File I/O (`file::read`, `file::write`, `file::append`)
- **Editor application (`rustle-app`)** — A VS Code-like desktop IDE built with egui that provides:
  - A code editor with syntax highlighting, line numbers, and indentation support
  - A live canvas panel that renders the script output in real time
  - Run/Stop controls with proper lifecycle management
  - A console panel for script output (`console <<`, `console.warn <<`, `console.error <<`)
  - A file explorer panel for navigating and opening project files
  - Tab-based editing for working with multiple files
- **Rendering (`rustle-renderer`)** — A wgpu-based rendering backend that:
  - Takes draw commands from the language runtime and renders them to the screen
  - Supports background color, z-index draw ordering, text rendering, gradients, blend modes, and image/texture loading
- **Standalone runner** — `rustle run myscript.rustle` CLI command that opens a window and runs a script as a standalone app
- **Test suite** — Comprehensive tests covering the resolver (semantic analysis) and runtime behavior

## Dependencies

- [`eframe`](https://lib.rs/crates/eframe) — Desktop application framework (windowing, event loop, wgpu context)
- [`egui`](https://lib.rs/crates/egui) — Immediate-mode GUI library for the editor UI (panels, buttons, text editor)
- [`wgpu`](https://lib.rs/crates/wgpu) — Cross-platform GPU abstraction for rendering shapes to the canvas
- [`thiserror`](https://lib.rs/crates/thiserror) — Ergonomic custom error types for compiler and runtime errors
