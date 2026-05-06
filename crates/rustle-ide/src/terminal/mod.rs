#[allow(dead_code, unused_imports, deprecated, clippy::all)]
#[macro_use]
pub mod bindings;
#[allow(dead_code, unused_imports, deprecated, clippy::all)]
pub mod backend;
#[allow(dead_code, unused_imports, deprecated, clippy::all)]
pub mod font;
#[allow(dead_code, unused_imports, deprecated, clippy::all)]
pub mod settings;
#[allow(dead_code, unused_imports, deprecated, clippy::all)]
pub mod theme;
#[allow(dead_code, unused_imports, deprecated, clippy::all)]
pub mod types;
#[allow(dead_code, unused_imports, deprecated, clippy::all)]
pub mod view;

pub use backend::{BackendCommand, PtyEvent, TerminalBackend, TerminalMode};
pub use settings::BackendSettings;
pub use view::TerminalView;
