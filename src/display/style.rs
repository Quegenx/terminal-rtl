use std::io::{self, Write};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub fg: vt100::Color,
    pub bg: vt100::Color,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
}

impl From<&vt100::Cell> for Style {
    fn from(cell: &vt100::Cell) -> Self {
        Self {
            fg: cell.fgcolor(),
            bg: cell.bgcolor(),
            bold: cell.bold(),
            dim: cell.dim(),
            italic: cell.italic(),
            underline: cell.underline(),
            inverse: cell.inverse(),
        }
    }
}

impl Style {
    pub(super) fn write(&self, out: &mut impl Write) -> io::Result<()> {
        write!(out, "\x1b[0")?;
        for (enabled, code) in [
            (self.bold, 1),
            (self.dim, 2),
            (self.italic, 3),
            (self.underline, 4),
            (self.inverse, 7),
        ] {
            if enabled {
                write!(out, ";{code}")?;
            }
        }
        for (color, base) in [(self.fg, 38), (self.bg, 48)] {
            match color {
                vt100::Color::Default => {}
                vt100::Color::Idx(index) => write!(out, ";{base};5;{index}")?,
                vt100::Color::Rgb(r, g, b) => write!(out, ";{base};2;{r};{g};{b}")?,
            }
        }
        write!(out, "m")
    }
}
