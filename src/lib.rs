pub mod display;
mod terminal;

// Preserve the public module paths while grouping implementation by domain.
pub use display::formatting as pretty;
pub use terminal::{input, protocol};
