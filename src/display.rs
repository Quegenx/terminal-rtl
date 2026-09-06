//! The parser always owns logical text. Only these disposable display rows are reordered.
use std::io::{self, Write};

use clap::ValueEnum;
use unicode_bidi::{BidiInfo, Level};
use unicode_bidi_mirroring::get_mirrored;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Direction {
    #[default]
    Auto,
    Ltr,
    Rtl,
}

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
    fn write(&self, out: &mut impl Write) -> io::Result<()> {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Glyph {
    pub text: String,
    pub width: u16,
    pub style: Style,
    pub(crate) logical_col: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisualRow {
    pub glyphs: Vec<Glyph>,
    pub logical_to_visual: Vec<u16>,
    pub visual_to_logical: Vec<u16>,
}

fn formatting_control(c: char) -> bool {
    matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

fn is_space(glyph: &Glyph) -> bool {
    glyph.text == " "
}

fn is_border(glyph: &Glyph) -> bool {
    glyph
        .text
        .chars()
        .any(|c| matches!(c, '\u{2500}'..='\u{257f}'))
}

/// Reorder text fields independently. Box borders, indentation, common prompt
/// markers and gaps of two or more spaces remain fixed in their terminal columns.
/// This is a layout heuristic, not a semantic parser for every possible TUI.
pub fn visual_row(cells: &[vt100::Cell], enabled: bool, direction: Direction) -> VisualRow {
    formatted_row(
        cells,
        enabled,
        direction,
        &crate::pretty::RowFormat::default(),
    )
}

fn formatted_row(
    cells: &[vt100::Cell],
    enabled: bool,
    direction: Direction,
    format: &crate::pretty::RowFormat,
) -> VisualRow {
    let cols = cells.len();
    let mut glyphs: Vec<Glyph> = cells
        .iter()
        .enumerate()
        .filter(|(_, cell)| !cell.is_wide_continuation())
        .map(|(col, cell)| Glyph {
            text: if cell.has_contents() {
                cell.contents().to_owned()
            } else {
                " ".into()
            },
            width: if cell.is_wide() { 2 } else { 1 },
            style: Style::from(cell),
            logical_col: col as u16,
        })
        .collect();
    format.apply(&mut glyphs);
    let mut logical_to_visual = (0..cols as u16).collect::<Vec<_>>();
    let mut visual_to_logical = logical_to_visual.clone();
    let mut rtl_endings = Vec::new();

    if enabled {
        let mut start = 0;
        while start < glyphs.len() {
            if is_border(&glyphs[start]) || is_space(&glyphs[start]) {
                start += 1;
                continue;
            }
            // Preserve common one-character prompt/bullet markers plus their gap.
            if matches!(
                glyphs[start].text.as_str(),
                ">" | "❯" | "›" | "●" | "•" | "-" | "+" | "*"
            ) && glyphs.get(start + 1).is_some_and(is_space)
            {
                start += 2;
                continue;
            }
            let mut end = start;
            while end < glyphs.len() && !is_border(&glyphs[end]) {
                if is_space(&glyphs[end]) && glyphs.get(end + 1).is_some_and(is_space) {
                    break;
                }
                end += 1;
            }
            while end > start && is_space(&glyphs[end - 1]) {
                end -= 1;
            }
            if end == start {
                start += 1;
                continue;
            }
            let text: String = glyphs[start..end].iter().map(|g| g.text.as_str()).collect();
            let base = match direction {
                Direction::Auto => None,
                Direction::Ltr => Some(Level::ltr()),
                Direction::Rtl => Some(Level::rtl()),
            };
            let bidi = BidiInfo::new(&text, base);
            if bidi.has_rtl() {
                let levels = bidi.reordered_levels(&bidi.paragraphs[0], 0..text.len());
                let mut offset = 0;
                let glyph_levels: Vec<_> = glyphs[start..end]
                    .iter()
                    .map(|g| {
                        let level = levels[offset];
                        offset += g.text.len();
                        level
                    })
                    .collect();
                let original = glyphs[start..end].to_vec();
                let order = BidiInfo::reorder_visual(&glyph_levels);
                for (visual, logical) in order.into_iter().enumerate() {
                    let mut glyph = original[logical].clone();
                    if glyph_levels[logical].is_rtl() {
                        glyph.text = glyph
                            .text
                            .chars()
                            .map(|c| get_mirrored(c).unwrap_or(c))
                            .collect();
                    }
                    // Do not let formatting controls trigger a second reorder in the host.
                    glyph.text.retain(|c| !formatting_control(c));
                    glyphs[start + visual] = glyph;
                }
                let last = original.last().unwrap();
                if glyph_levels.last().unwrap().is_rtl() {
                    rtl_endings.push((last.logical_col + last.width, last.logical_col));
                }
            }
            start = end;
        }
    }
    let mut visual_col = 0;
    for glyph in &glyphs {
        for part in 0..glyph.width {
            let logical = usize::from(glyph.logical_col + part);
            let visual = usize::from(visual_col + part);
            if logical < cols && visual < cols {
                logical_to_visual[logical] = visual as u16;
                visual_to_logical[visual] = logical as u16;
            }
        }
        visual_col += glyph.width;
    }
    // At the end of an RTL input run, the insertion point is at its visual left.
    // Only adjust an unoccupied cell, never a real following glyph or UI border.
    for (boundary, last) in rtl_endings {
        if cells
            .get(usize::from(boundary))
            .is_some_and(|c| !c.has_contents())
        {
            logical_to_visual[usize::from(boundary)] = logical_to_visual[usize::from(last)];
        }
    }
    VisualRow {
        glyphs,
        logical_to_visual,
        visual_to_logical,
    }
}

#[derive(Default)]
pub struct Renderer {
    source: Vec<Vec<vt100::Cell>>,
    rows: Vec<VisualRow>,
    cursor: Option<(u16, u16, bool)>,
    settings: Option<(bool, Direction, (u16, u16), usize)>,
    pretty: bool,
    agent_label: Option<String>,
    labels: Vec<Option<String>>,
    formats: Vec<crate::pretty::RowFormat>,
}

impl Renderer {
    pub fn set_agent_label(&mut self, agent: Option<&str>) {
        let label = agent
            .filter(|name| matches!(*name, "codex" | "grok"))
            .map(str::to_owned);
        if self.agent_label != label {
            self.agent_label = label;
            self.invalidate();
        }
    }

    pub fn margin(&self) -> u16 {
        if self.agent_label.is_some() { 8 } else { 0 }
    }

    pub fn set_pretty(&mut self, enabled: bool) {
        if self.pretty != enabled {
            self.pretty = enabled;
            self.invalidate();
        }
    }
    pub fn invalidate(&mut self) {
        self.source.clear();
        self.rows.clear();
        self.cursor = None;
        self.settings = None;
        self.formats.clear();
        self.labels.clear();
    }

    pub fn logical_column(&self, row: u16, col: u16) -> u16 {
        let col = col.saturating_sub(self.margin());
        self.rows
            .get(usize::from(row))
            .and_then(|r| r.visual_to_logical.get(usize::from(col)))
            .copied()
            .unwrap_or(col)
    }

    /// Emit changed rows only. Host autowrap is disabled by the session guard:
    /// writing the bottom-right cell must never scroll the host's screen.
    pub fn render(
        &mut self,
        screen: &vt100::Screen,
        enabled: bool,
        direction: Direction,
        out: &mut impl Write,
    ) -> io::Result<()> {
        let (height, width) = screen.size();
        let settings = (enabled, direction, (height, width), screen.scrollback());
        if self.settings != Some(settings) {
            self.invalidate();
            self.settings = Some(settings);
        }
        let mut updates = Vec::new();
        let grid: Vec<Vec<_>> = (0..height)
            .map(|row| {
                (0..width)
                    .map(|col| screen.cell(row, col).unwrap().clone())
                    .collect()
            })
            .collect();
        let formats = if self.pretty {
            crate::pretty::detect(
                &grid,
                (!screen.hide_cursor() && screen.scrollback() == 0)
                    .then_some(screen.cursor_position().0),
            )
        } else {
            vec![crate::pretty::RowFormat::default(); usize::from(height)]
        };
        let labels = crate::pretty::speaker_labels(&grid, self.agent_label.as_deref());
        for row in 0..height {
            let index = usize::from(row);
            let cells = &grid[index];
            if self.source.get(index) == Some(cells)
                && self.formats.get(index) == Some(&formats[index])
                && self.labels.get(index) == Some(&labels[index])
            {
                continue;
            }
            let visual = formatted_row(cells, enabled, direction, &formats[index]);
            if self.rows.get(index) != Some(&visual)
                || self.labels.get(index) != Some(&labels[index])
            {
                write!(updates, "\x1b[{};1H\x1b[0m", row + 1)?;
                if self.margin() > 0 {
                    let color = if labels[index].as_deref() == Some("you:") {
                        6
                    } else {
                        5
                    };
                    write!(
                        updates,
                        "\x1b[1;38;5;{color}m{:<8}\x1b[0m",
                        labels[index].as_deref().unwrap_or("")
                    )?;
                }
                let mut style = Style::default();
                let used = visual
                    .glyphs
                    .iter()
                    .rposition(|g| g.text != " " || g.style != Style::default())
                    .map_or(0, |i| i + 1);
                for glyph in &visual.glyphs[..used] {
                    if glyph.style != style {
                        glyph.style.write(&mut updates)?;
                        style = glyph.style.clone();
                    }
                    updates.write_all(glyph.text.as_bytes())?;
                }
                if used < visual.glyphs.len() {
                    updates.write_all(b"\x1b[0m\x1b[K")?;
                }
            }
            if index == self.source.len() {
                self.source.push(cells.clone());
                self.rows.push(visual);
            } else {
                self.source[index] = cells.clone();
                self.rows[index] = visual;
            }
        }
        self.formats = formats;
        self.labels = labels;
        let (row, col) = screen.cursor_position();
        let row = row.min(height - 1);
        let col = col.min(width - 1);
        let visual_col =
            self.rows[usize::from(row)].logical_to_visual[usize::from(col)] + self.margin();
        let cursor = (
            row,
            visual_col,
            screen.hide_cursor() || screen.scrollback() > 0,
        );
        if !updates.is_empty() || self.cursor != Some(cursor) {
            out.write_all(b"\x1b[?25l")?;
            out.write_all(&updates)?;
            write!(out, "\x1b[0m\x1b[{};{}H", row + 1, visual_col + 1)?;
            if !cursor.2 {
                out.write_all(b"\x1b[?25h")?;
            }
            out.flush()?;
            self.cursor = Some(cursor);
        }
        Ok(())
    }
}
