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

use crate::syntax::ast::Program as AstProgram;
use namespaces::NamespaceRegistry;
use analysis::resolve;

// ─── Public API types ─────────────────────────────────────────────────────────

/// Persistent script state between frames. Defined by the `state {}` block.
#[derive(Debug, Clone, Default)]
pub struct State(pub HashMap<String, Value>);

/// Per-frame input passed into `update`.
#[derive(Debug, Clone, Default)]
pub struct Input {
    pub dt: f64,
}

/// A compiled Rustle program. Produced by `compile`.
pub struct Program {
    pub(crate) ast: AstProgram,
    pub(crate) registry: NamespaceRegistry,
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Parse and type-check source text. Returns a compiled program ready for execution.
pub fn compile(source: &str) -> Result<Program, Vec<Error>> {
    let tokens = syntax::lexer::Lexer::new(source).tokenize()?;
    let ast = syntax::parser::Parser::new(tokens).parse()?;
    let registry = NamespaceRegistry::standard();
    resolve(&ast, &registry)?;
    Ok(Program { ast, registry })
}

// ─── Runtime ──────────────────────────────────────────────────────────────────

/// Persistent runtime that owns the program, state, and frame config between ticks.
///
/// Lifecycle:
///   1. `Runtime::new(program)` — runs top-level config (`resolution`, `origin`),
///      evaluates `state {}` field initializers, and calls `init(state)` if present.
///   2. `runtime.tick(input)` — runs `update(state, input)` each frame, persisting
///      both state and coord_meta (resolution/origin) across ticks.
pub struct Runtime {
    program: Program,
    state: State,
    runtime_state: RuntimeState,
}

impl Runtime {
    pub fn new(program: Program) -> Result<Self, RuntimeError> {
        use runtime::interpreter::Interpreter;

        let mut interp = Interpreter::new(&program.ast, &program.registry);

        // 1. Run top-level stmts — resolution(), origin(), etc. These set
        //    runtime_state.coord_meta which persists for all subsequent ticks.
        interp.run_top_level()?;

        // 2. Evaluate state{} field initializers.
        let mut state = State::default();
        if let Some(ref state_block) = program.ast.state {
            for field in &state_block.fields {
                let val = interp.eval_expr(&field.initializer).unwrap_or(Value::Float(0.0));
                state.0.insert(field.name.clone(), val);
            }
        }

        // 3. Run init(state) if present — full imperative setup (loops, push, etc.).
        state = interp.run_init(state)?;

        let runtime_state = interp.take_runtime_state();

        Ok(Self { program, state, runtime_state })
    }

    /// Execute one frame. Runs `update(state, input)` if present, otherwise
    /// re-runs top-level draw statements.
    pub fn tick(&mut self, input: &Input) -> Result<Vec<DrawCommand>, RuntimeError> {
        use runtime::interpreter::Interpreter;
        use syntax::ast::Item;

        let mut interp = Interpreter::new(&self.program.ast, &self.program.registry)
            .with_runtime_state(self.runtime_state.clone());

        if self.program.ast.items.iter().any(|i| matches!(i, Item::FnDef(f) if f.name == "update")) {
            self.state = interp.run_update(self.state.clone(), input)?;
        } else {
            interp.run_top_level()?;
        }

        self.runtime_state = interp.take_runtime_state();
        Ok(interp.take_output())
    }

    pub fn state(&self) -> &State { &self.state }
}
