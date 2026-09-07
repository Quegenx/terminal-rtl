//! The parser always owns logical text. Only these disposable display rows are reordered.
use clap::ValueEnum;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Direction {
    #[default]
    Auto,
    Ltr,
    Rtl,
}

/// Whitespace is ambiguous: callers can choose prose or fixed terminal fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Layout {
    #[default]
    Columns,
    Prose,
}

pub mod formatting;
mod renderer;
mod row;
mod style;

pub use renderer::{Renderer, replay};
pub use row::{Glyph, VisualRow, visual_row, visual_row_with_layout};
pub use style::Style;
