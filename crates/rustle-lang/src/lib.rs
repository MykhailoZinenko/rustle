//! Rustle scripting language runtime.
//!
//! Provides compilation and execution of `.rustle` scripts for 2D interactive scenes.
//! The main entry points are [`compile`] and [`Runtime`].

pub mod syntax;
pub mod types;
pub mod runtime;
pub mod analysis;
pub mod error;
pub mod namespaces;

pub use types::draw::{CoordMeta, DrawCommand, Origin, RenderMode, ShapeData, ShapeDesc, TransformData, origin_offset};
pub use error::{Error, ErrorCode, RuntimeError};
pub use syntax::token::{Token, TokenKind};
pub use runtime::value::Value;
pub use namespaces::RuntimeState;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::syntax::ast::Program as AstProgram;
use namespaces::NamespaceRegistry;
use types::binop_registry::BinopRegistry;
use types::registry::TypeRegistry;
use analysis::resolve;

// ─── Public API types ─────────────────────────────────────────────────────────

/// Persistent script state between frames. Defined by the `state {}` block.
#[derive(Debug, Clone, Default)]
pub struct State(pub HashMap<String, Value>);

/// Per-frame input passed into `on_update`.
#[derive(Debug, Clone)]
pub struct Input {
    /// Elapsed time in seconds since the last frame.
    pub dt: f64,
    /// Mouse X position (in script coordinates).
    pub mouse_x: f64,
    /// Mouse Y position (in script coordinates).
    pub mouse_y: f64,
    /// Left mouse button currently held.
    pub mouse_down: bool,
    /// Left mouse button just pressed this frame.
    pub mouse_pressed: bool,
    /// Left mouse button just released this frame.
    pub mouse_released: bool,
    /// Key just pressed this frame (empty if none).
    pub key_pressed: String,
    /// Key currently held (empty if none).
    pub key_down: String,
    /// Key just released this frame (empty if none).
    pub key_released: String,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            dt: 0.0,
            mouse_x: 0.0,
            mouse_y: 0.0,
            mouse_down: false,
            mouse_pressed: false,
            mouse_released: false,
            key_pressed: String::new(),
            key_down: String::new(),
            key_released: String::new(),
        }
    }
}

/// A compiled Rustle program. Produced by `compile`.
pub struct Program {
    pub(crate) ast: AstProgram,
    pub(crate) registry: NamespaceRegistry,
    pub(crate) types: TypeRegistry,
    pub(crate) binops: BinopRegistry,
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Parse and type-check source text. Returns a compiled program ready for execution.
///
/// # Errors
/// Returns `Vec<Error>` if the source has lexer, parser, or semantic errors.
///
/// # Examples
/// ```
/// let program = rustle_lang::compile("state { let x: float = 0.0 }").unwrap();
/// ```
pub fn compile(source: &str) -> Result<Program, Vec<Error>> {
    let tokens = syntax::lexer::Lexer::new(source).tokenize()?;
    let ast = syntax::parser::Parser::new(tokens).parse()?;
    let registry = NamespaceRegistry::standard();
    resolve(&ast, &registry)?;
    Ok(Program { ast, registry, types: TypeRegistry::default(), binops: BinopRegistry::default() })
}

// ─── Runtime ──────────────────────────────────────────────────────────────────

/// Persistent runtime that owns the program, state, and frame config between ticks.
///
/// Lifecycle:
///   1. `Runtime::new(program)` — runs top-level config (`resolution`, `origin`),
///      evaluates `state {}` field initializers, and calls `init(state)` if present.
///   2. `runtime.tick(input)` — runs `update(state, input)` each frame, persisting
///      both state and `coord_meta` (resolution/origin) across ticks.
#[expect(clippy::struct_field_names, reason = "runtime_state is the clearest name for the field; abbreviating would reduce clarity")]
pub struct Runtime {
    program: Program,
    state: State,
    runtime_state: RuntimeState,
    base_env: Option<crate::runtime::interpreter::Env>,
    cancel: Option<Arc<AtomicBool>>,
}

impl Runtime {
    /// # Errors
    /// Returns an error if the script's `on_init` fails at runtime.
    pub fn new(program: Program) -> Result<Self, RuntimeError> {
        Self::new_inner(program, None)
    }

    /// Create a runtime with a cancellation token. When the flag is set to `true`,
    /// the interpreter aborts at the next loop iteration or function call boundary.
    ///
    /// # Errors
    /// Returns an error if the script's `on_init` fails at runtime.
    pub fn new_cancellable(program: Program, cancel: Arc<AtomicBool>) -> Result<Self, RuntimeError> {
        Self::new_inner(program, Some(cancel))
    }

    fn new_inner(program: Program, cancel: Option<Arc<AtomicBool>>) -> Result<Self, RuntimeError> {
        use runtime::interpreter::Interpreter;

        let mut interp = Interpreter::new(&program.ast, &program.registry, &program.types, &program.binops);
        if let Some(ref c) = cancel {
            interp = interp.with_cancel(c.clone());
        }

        // 1. Run top-level stmts — resolution(), origin(), etc. These set
        //    runtime_state.coord_meta which persists for all subsequent ticks.
        interp.run_top_level()?;

        // Cache the environment after top-level setup (imports + constants).
        // This avoids re-running imports and top-level statements on every tick.
        let base_env = Some(interp.take_env());

        // 2. Evaluate state{} field initializers.
        let mut state = State::default();
        if let Some(ref state_block) = program.ast.state {
            for field in &state_block.fields {
                let val = interp.eval_expr(&field.initializer)?;
                state.0.insert(field.name.clone(), val);
            }
        }

        // 3. Run init(state) if present — full imperative setup (loops, push, etc.).
        state = interp.run_init(state)?;

        let runtime_state = interp.take_runtime_state();

        Ok(Self { program, state, runtime_state, base_env, cancel })
    }

    /// Execute one frame. Runs `update(state, input)` if present, otherwise
    /// re-runs top-level draw statements.
    ///
    /// # Errors
    /// Returns an error if the script's `on_update` fails at runtime.
    pub fn tick(&mut self, input: &Input) -> Result<Vec<DrawCommand>, RuntimeError> {
        use runtime::interpreter::Interpreter;

        let mut interp = Interpreter::new(&self.program.ast, &self.program.registry, &self.program.types, &self.program.binops)
            .with_runtime_state(self.runtime_state.clone());
        if let Some(ref c) = self.cancel {
            interp = interp.with_cancel(c.clone());
        }

        if interp.has_fn("on_update") {
            // Restore cached env (imports + top-level vars) instead of re-running top_level
            if let Some(ref env) = self.base_env {
                interp = interp.with_env(env.clone());
            }
            self.state = interp.run_update(self.state.clone(), input)?;
        } else {
            // Static scripts (no on_update): must re-run top-level each frame to emit shapes
            interp.run_top_level()?;
        }

        self.runtime_state = interp.take_runtime_state();
        Ok(interp.take_output())
    }

    /// Return a reference to the script's persistent state.
    #[must_use]
    pub fn state(&self) -> &State { &self.state }

    /// Run `on_exit(s)` if defined, then drop. Call when the app stops.
    ///
    /// # Errors
    /// Returns an error if the script's `on_exit` fails at runtime.
    pub fn exit(&mut self) -> Result<(), RuntimeError> {
        use runtime::interpreter::Interpreter;

        let mut interp = Interpreter::new(&self.program.ast, &self.program.registry, &self.program.types, &self.program.binops)
            .with_runtime_state(self.runtime_state.clone());
        if let Some(ref c) = self.cancel {
            interp = interp.with_cancel(c.clone());
        }
        self.state = interp.run_on_exit(self.state.clone())?;
        Ok(())
    }
}
