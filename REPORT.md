# Rustle Project Report

## Introduction to the Idea

Rustle is a statically typed scripting language for creative graphics programming. The idea behind it is to make small visual experiments feel quick and expressive, while still keeping some of the safety normally associated with larger compiled languages. A user writes a `.rustle` script, runs it in the editor or standalone runner, and sees the script's draw commands rendered as 2D graphics.

The project is motivated by a gap between two common kinds of tools. On one side, languages such as Rust and C++ provide performance and strong correctness guarantees, but they are often too verbose for sketching visual ideas. On the other side, creative coding environments such as Processing or p5.js are approachable, but they usually rely on dynamic typing and runtime errors. Rustle tries to combine the more playful workflow of creative coding with a static analysis step that catches many mistakes before execution.

The main programming model is based on explicit output and a simple lifecycle. Scripts can define persistent state in a `state {}` block and lifecycle functions such as `on_init`, `on_update`, and `on_exit`. Every frame, `on_update` receives the current state and input data, modifies state, and pushes shapes to the output stream using `out <<`. The renderer then consumes those draw commands. This keeps the language predictable: nothing is drawn implicitly, and the program state that survives between frames is visible in one place.

The final project is split into four main crates:

- `rustle-lang`: lexer, parser, semantic analysis, compiler, bytecode VM, runtime API, built-in namespaces, and draw command generation.
- `rustle-renderer`: a `wgpu` renderer that turns draw commands into GPU draw calls.
- `rustle-ide`: an `egui`/`eframe` desktop IDE with editor, preview, console, tabs, file handling, terminal support, and a separate document model for editing behavior.
- `rustle-cli`: a standalone script runner that opens a window and runs a `.rustle` file.

## Requirements in More Detail

The first requirement was to build a usable language core. Rustle needed its own lexer and parser, a typed abstract syntax tree, static analysis, and runtime execution. The language supports common control flow (`if`, `else if`, `while`, `for`, `foreach`, `match`, `break`, `continue`), functions, lambdas, structs, enums, optional values, string interpolation, list operations, type conversions, and several built-in graphics-related types such as vectors, colors, matrices, transforms, and shapes.

The second requirement was static checking. Before a script runs, Rustle performs semantic analysis to catch undefined symbols, type mismatches, invalid calls, unknown fields, visibility errors, invalid lifecycle signatures, non-exhaustive enum matches, and similar mistakes. Error messages include error codes and, in some cases, hints such as suggested names or available fields. This was important because the project is meant to feel interactive, but not fragile.

The third requirement was live graphical output. The language runtime produces a `Vec<DrawCommand>` each frame. These commands include shape drawing and console output. The renderer supports multiple shape paths, including filled polygons, lines, SDF-based shapes, and MSDF text. The IDE embeds the renderer inside the preview panel, while the CLI creates its own window and render loop.

The fourth requirement was a practical editor. The IDE provides a code editing workspace, run and stop controls, a preview canvas, console/error output, file opening and saving, editor tabs, suggestions, syntax highlighting helpers, and a terminal panel. The newer editor layer also separates document editing concerns into modules for document text, selections, undo/redo history, and markers such as search results or bracket pairs. The IDE is not responsible for interpreting the language directly; instead, it calls the public API exposed by `rustle-lang`.

The fifth requirement was testing and examples. The project contains a large test suite covering resolver behavior, runtime behavior, the language specification, VM internals, renderer preparation, terminal bindings, editor document behavior, and performance-oriented scripts. There are also example `.rustle` programs such as calculator, mandala, galaxy, typing, circle, and showcase examples.

## Design Diagram

```text
                       .rustle source file
                              |
                              v
                    +--------------------+
                    |    rustle-lang     |
                    | lexer and parser   |
                    +--------------------+
                              |
                              v
                    +--------------------+
                    | semantic analysis  |
                    | symbols and types  |
                    +--------------------+
                              |
                              v
                    +--------------------+
                    | bytecode compiler  |
                    | optimized VM       |
                    +--------------------+
                              |
                 Runtime::tick(input) per frame
                              |
                              v
                    +--------------------+
                    |  Vec<DrawCommand>  |
                    +--------------------+
                       |              |
                       v              v
          +--------------------+   +--------------------+
          |   rustle-renderer  |   | console messages   |
          |    wgpu backend    |   | errors and output  |
          +--------------------+   +--------------------+
                       |
                       v
          +-------------------------------+
          | rustle-ide preview or CLI app |
          +-------------------------------+
```

At a higher level, the project separates language execution from presentation. The IDE and CLI do not need to know how lexing, type checking, or bytecode execution work. They compile a source string, create a `Runtime`, pass input to `tick`, and render the returned commands.

## Design Choices

One important choice was to make Rustle statically typed. The alternative would have been a simpler dynamically typed interpreter, which would probably have been faster to implement at first. However, static typing fits the goal of catching mistakes early in an editor. It also makes graphical APIs clearer: a `circle` expects a `vec2` position and a numeric radius, list operations have predictable element types, and enum matches can be checked for exhaustiveness.

Another major choice was to use a bytecode VM instead of only walking the AST. A tree-walking interpreter is easier to start with, but bytecode gives a cleaner execution boundary and better performance potential. The public API still stays simple: `compile(source)` returns a compiled program, and `Runtime::tick(input)` runs a frame. Internally, the VM uses stack values, heap objects, compiled chunks, globals, and lifecycle chunks.

After a later code review pass, the VM design was also tightened. Several repeated helper routines were moved into shared VM utilities, color constants were centralized, field access was rewritten to reduce unnecessary cloning, and new stack-manipulation opcodes fixed side-effecting increment/decrement cases without re-evaluating expressions. This made the implementation cleaner and improved some hot paths without changing the public language model.

The project also chooses explicit drawing with `out <<` instead of automatic scene state. A more traditional creative coding model might expose global functions that draw immediately. Rustle instead accumulates draw commands per frame. This makes the renderer independent from the interpreter and makes it possible to test emitted shapes without opening a window.

For state, Rustle uses a dedicated `state {}` block and lifecycle functions. The alternative would have been normal mutable globals. The explicit state block makes frame-persistent data easier to reason about, especially in an editor where users need to understand why a value survives between frames. It also gives the runtime a clear place to expose state for tests and tooling.

For rendering, the project uses `wgpu` rather than relying only on `egui` painting primitives. The IDE still uses `egui` for the application UI, but rendering is isolated in `rustle-renderer`. This makes the renderer reusable by both the IDE and CLI, and it leaves room for GPU-oriented features such as text atlases, SDF shapes, bigger scenes, and more advanced visual effects.

The workspace is split into crates instead of being a single large binary. This adds some project structure overhead, but it keeps responsibilities clean. The language crate can be tested independently. The renderer can evolve without changing the parser. The CLI can be small because it only wires together the language runtime, input events, a window, and the renderer.

For the IDE editor, the project now uses a rope-based document model rather than treating the source code as one plain `String` everywhere. A plain string would be simpler, but a rope is a better fit for repeated edits in larger files. It also allowed the editor logic to model character offsets, selections, multi-cursor edits, undo/redo groups, search matches, and bracket-pair markers more deliberately.

## Dependencies and Their Purpose

- `wgpu`: used by `rustle-renderer`, `rustle-ide`, and `rustle-cli` for GPU rendering.
- `bytemuck`: used by the renderer to safely cast vertex and instance data into byte slices for GPU buffers.
- `serde` and `serde_json`: used by the renderer to load structured atlas data.
- `image`: used by the renderer to load PNG assets, especially the MSDF font atlas texture.
- `eframe`: used by the IDE as the native desktop application framework and event loop integration.
- `egui`: used by the IDE for panels, controls, editor UI, console UI, tabs, and general immediate-mode interface layout.
- `egui-wgpu`: used to integrate custom `wgpu` rendering with the `egui`/`eframe` renderer.
- `rfd`: used by the IDE for native file dialogs.
- `alacritty_terminal`: used by the IDE terminal panel.
- `open`: used by the IDE for opening files or external locations through the operating system.
- `ropey`: used by the IDE document model for efficient text storage and character-indexed editing.
- `winit`: used by the CLI for window creation and input events.
- `pollster`: used by the CLI to block on async `wgpu` initialization.

The project also uses a Cargo workspace with shared lint settings. Clippy lints such as `redundant_clone`, `large_enum_variant`, and `needless_collect` are enabled as warnings to encourage more idiomatic Rust.

## Evaluation

Overall, the project went well in terms of architecture. Splitting the system into `rustle-lang`, `rustle-renderer`, `rustle-ide`, and `rustle-cli` made the boundaries clear. The language runtime exposes a small public API, which made it possible to test scripts without starting the GUI. The draw-command model also worked well because it decouples language execution from rendering.

The language implementation became quite feature-rich. Rustle now includes not only basic expressions and control flow, but also structs, enums, optionals, closures, list operations, input handling, file I/O, text shapes, and a namespace system. The test suite is a strong part of the result: the project now contains more than 1,700 Rust tests across language specification, runtime behavior, resolver behavior, VM internals, renderer preparation, editor behavior, terminal bindings, and performance experiments.

The IDE also reached a useful shape. It provides the main workflow expected from the project: writing `.rustle` code, running it, seeing output, and reading console/errors. Separating core application state from UI modules was a good design choice because it keeps the editor easier to extend. The newer `editor` module improves this further by giving text editing its own tested core, including rope-backed storage, undo/redo, multi-cursor insertion, find/replace helpers, Unicode-aware character offsets, and marker tracking.

The parts that went less smoothly were the complexity-heavy ones: language design, diagnostics, VM correctness, and rendering integration. Small syntax decisions can have large consequences in the parser, type checker, VM, tests, and documentation. Implementing structs, enums, closures, and match behavior required careful coordination between static analysis and runtime representation. A later review found exactly the kind of issues that appear in larger interpreters: duplicated helper logic, unnecessary cloning, stack manipulation edge cases, and performance-sensitive opcode design. Rendering also introduced a different kind of complexity: GPU resources, buffers, shader pipelines, text atlases, and integration with `egui` all need more setup than simple CPU drawing.

Some features are still future-facing or could use more polish. The renderer has a solid base, but bigger rendering features such as richer gradients, image textures, advanced blending, and more complete z-order control could be expanded. The editor could also grow into a more complete language environment with deeper autocomplete, formatting, debugger-like tools, and better inline diagnostics. The review also left useful future work in the language core, such as more defensive compiler checks, reducing parser string cloning, improving span precision, and documenting more public APIs.

Implementing a bigger project in Rust felt different from using more dynamic languages. Rust made early development slower in places because ownership, lifetimes, enum design, and borrowing rules force many decisions to be explicit. This was especially noticeable in the VM and renderer, where values, heap objects, shared state, and GPU resources all have different lifetimes and ownership needs.

At the same time, Rust helped a lot once the project became larger. Strong enums were excellent for tokens, AST nodes, values, errors, draw commands, and shape descriptions. Pattern matching made compiler and runtime code clearer. The type system made refactoring less risky because many broken assumptions were caught at compile time. Compared with a language like JavaScript or Python, iteration can feel less free at the beginning, but the project feels more stable as it grows.

In short, Rustle became a small but serious creative programming environment: it has a language, compiler pipeline, optimized VM, renderer, IDE, CLI runner, examples, tests, documentation, and review-driven cleanup work. The project also showed why Rust is demanding but rewarding for this kind of work. It asks for more discipline up front, but that discipline pays back when many moving parts have to keep working together.
