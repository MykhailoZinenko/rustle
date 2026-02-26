# Rustle

A statically-typed scripting language for creative graphics programming. Write `.rustle` scripts, see shapes rendered live in the editor.

## Purpose

Rustle is designed for generative art and interactive 2D scenes. You get explicit control: nothing is drawn unless you push it to the output stream; no coordinate system is assumed — you declare the one you want. Types are checked at compile time.

## Documentation

**[Full documentation →](https://mykhailozinenko.github.io/rustle/)**

---

## Roadmap

### Phase 1 — Language Foundations

- [x] Lifecycle hooks: `on_init`, `on_update`, `on_exit`
- [x] `else if`
- [x] Compound assignment: `+=`, `-=`, `*=`, `/=`
- [x] Index assignment: `list[i] = x`, `list[i] += 1`
- [ ] `switch` / `match` (simple value matching)
- [ ] `++`, `--`
- [ ] Comments (`//` and `/* */`)
- [ ] `null` / `none` value
- [ ] Type coercion / truthiness rules
- [ ] Console output: `console << x`, `console.warn`, `console.error`
- [ ] String interpolation and operations

### Phase 2 — Language Expressiveness

- [ ] Structs (custom data types)
- [ ] Enums + `match` with destructuring
- [ ] Array operations: `map`, `find`, `filter`, `reduce`, `any`, `all`
- [ ] Input handling (mouse, keyboard)
- [ ] File I/O
- [ ] Console input stream

### Phase 3 — wgpu Renderer

- [ ] Implement `rustle-renderer` as a proper wgpu crate
- [ ] Replace egui tessellation in `rustle-app`
- [ ] `rustle run myscript.rustle` standalone CLI runner

### Phase 4 — Rendering Features

- [ ] Background color, z-index
- [ ] Text rendering
- [ ] Gradients, blend modes, images/textures

### Editor

- [x] Run / Stop buttons, no auto-run on edit
- [x] Stop triggers `on_exit`
- [ ] Console panel
