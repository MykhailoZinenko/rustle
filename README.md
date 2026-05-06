# Rustle

A statically-typed scripting language for creative graphics programming. Write `.rustle` scripts, see shapes rendered live in the editor.

## Purpose

Rustle is designed for generative art and interactive 2D scenes. You get explicit control: nothing is drawn unless you push it to the output stream; no coordinate system is assumed — you declare the one you want. Types are checked at compile time.

## Documentation

**[Full documentation →](https://mykhailozinenko.github.io/rustle/)**

---

## Roadmap

### Phase 1 — Language Foundations ✅

- [x] Lifecycle hooks: `on_init`, `on_update`, `on_exit`
- [x] `else if`
- [x] Compound assignment: `+=`, `-=`, `*=`, `/=`
- [x] Index assignment: `list[i] = x`, `list[i] += 1`
- [x] `match` (simple value matching)
- [x] `++`, `--`
- [x] `break`, `continue`
- [x] Comments (`//` and `/* */`)
- [x] `none` value with optional types (`T?`, `??`, `if let`, `?.`)
- [x] Type coercion / truthiness rules
- [x] Boolean arithmetic (`true + 1` → `2`)
- [x] Console output: `console << x`, `console.warn`, `console.error`
- [x] String interpolation (`` `hello ${name}` ``)
- [x] String operations (`len`, `contains`, `trim`, `replace`, `split`, `to_upper`, `to_lower`, `starts_with`, `ends_with`, `+`)
- [x] Type conversions (`x as string`, `x as float`, `x as bool`)
- [x] Better error messages with hints and suggestions

### Phase 2 — Language Expressiveness

- [x] Structs (`+let`/`#let` fields, `+fn`/`#fn` methods, `this`, reference semantics, `.clone()`)
- [x] Enums + `match` with type narrowing and field access in arms
- [x] Array operations: `map`, `filter`, `search`, `bsearch`, `sort`, `take`, `drop`, `cut`, `paste`, `any`, `all`
- [x] Input handling (mouse position, buttons, keyboard)
- [x] File I/O (`file.read`, `file.read_lines`, `file.write`, `file.append`)
- [ ] Console input stream (deferred to CLI phase)

### Phase 3 — wgpu Renderer

- [ ] Implement `rustle-renderer` as a proper wgpu crate
- [ ] Replace egui tessellation in `rustle-ide`
- [ ] `rustle run myscript.rustle` standalone CLI runner

### Phase 4 — Rendering Features

- [ ] Background color, z-index
- [ ] Text rendering
- [ ] Gradients, blend modes, images/textures

### Editor

- [x] Run / Stop buttons, no auto-run on edit
- [x] Stop triggers `on_exit`
- [x] Console panel
