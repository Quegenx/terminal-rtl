use std::io::{self, Write};

use super::row::formatted_row;
use super::{Direction, Layout, Style, VisualRow, visual_row_with_layout};

#[derive(Default)]
pub struct Renderer {
    source: Vec<Vec<vt100::Cell>>,
    rows: Vec<VisualRow>,
    cursor: Option<(u16, u16, bool)>,
    settings: Option<(bool, Direction, (u16, u16), usize)>,
    pretty: bool,
    layout: Layout,
    attribution: bool,
    attribution_drawn: bool,
    agent_label: Option<String>,
    labels: Vec<Option<String>>,
    formats: Vec<super::formatting::RowFormat>,
}

impl Renderer {
    pub fn set_layout(&mut self, layout: Layout) {
        if self.layout != layout {
            self.layout = layout;
            self.invalidate();
        }
    }

    pub fn set_attribution(&mut self, enabled: bool) {
        if self.attribution != enabled {
            self.attribution = enabled;
            self.invalidate();
        }
    }

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
        self.attribution_drawn = false;
    }

    pub fn logical_column(&self, row: u16, col: u16) -> u16 {
        let col = col.saturating_sub(self.margin());
        self.rows
            .get(usize::from(row))
            .and_then(|r| r.visual_to_logical.get(usize::from(col)))
            .copied()
            .unwrap_or(col)
    }

    pub(crate) fn write_row(
        &self,
        visual: &VisualRow,
        label: Option<&str>,
        out: &mut impl Write,
    ) -> io::Result<()> {
        out.write_all(b"\x1b]8;;\x1b\\")?;
        if self.margin() > 0 {
            let color = if label == Some("you:") { 6 } else { 5 };
            write!(out, "\x1b[1;38;5;{color}m{:<8}\x1b[0m", label.unwrap_or(""))?;
        }
        let mut style = Style::default();
        let mut hyperlink = None;
        let used = visual
            .glyphs
            .iter()
            .rposition(|g| g.text != " " || g.style != Style::default() || g.hyperlink.is_some())
            .map_or(0, |i| i + 1);
        for glyph in &visual.glyphs[..used] {
            if glyph.hyperlink.as_deref() != hyperlink {
                if let Some(link) = glyph.hyperlink.as_deref() {
                    write!(out, "\x1b]8;id={};{}\x1b\\", link.id(), link.uri())?;
                } else {
                    out.write_all(b"\x1b]8;;\x1b\\")?;
                }
                hyperlink = glyph.hyperlink.as_deref();
            }
            if glyph.style != style {
                glyph.style.write(out)?;
                style = glyph.style.clone();
            }
            out.write_all(glyph.text.as_bytes())?;
        }
        if hyperlink.is_some() {
            out.write_all(b"\x1b]8;;\x1b\\")?;
        }
        if used < visual.glyphs.len() {
            out.write_all(b"\x1b[0m\x1b[K")?;
        }
        Ok(())
    }

    /// Push finalized transcript rows into the host's normal scrollback, then
    /// invalidate the live frame so its cursor, composer and footer are restored.
    pub fn append_history(
        &mut self,
        rows: &[Vec<vt100::Cell>],
        host_size: (u16, u16),
        enabled: bool,
        direction: Direction,
        out: &mut impl Write,
    ) -> io::Result<()> {
        if rows.is_empty() {
            return Ok(());
        }
        let formats = if self.pretty {
            super::formatting::detect(rows, None)
        } else {
            vec![super::formatting::RowFormat::default(); rows.len()]
        };
        let labels = super::formatting::speaker_labels(rows, self.agent_label.as_deref());
        out.write_all(b"\x1b[?25l\x1b[r")?;
        for (index, cells) in rows.iter().enumerate() {
            let width = host_size.0.saturating_sub(self.margin());
            let cells = &cells[..cells.len().min(usize::from(width))];
            let visual = formatted_row(cells, enabled, direction, self.layout, &formats[index]);
            out.write_all(b"\x1b[1;1H\x1b[0m")?;
            self.write_row(&visual, labels[index].as_deref(), out)?;
            // SU (CSI S) discards rows in hosts such as xterm.js. A line feed
            // at the bottom of the full viewport saves the top row to history.
            write!(out, "\x1b[0m\x1b[K\x1b[{};1H\r\n", host_size.1)?;
        }
        self.shift_cache_after_host_scroll(rows.len());
        Ok(())
    }

    fn shift_cache_after_host_scroll(&mut self, count: usize) {
        let source = count.min(self.source.len());
        self.source.drain(..source);
        let rows = count.min(self.rows.len());
        self.rows.drain(..rows);
        let formats = count.min(self.formats.len());
        self.formats.drain(..formats);
        let labels = count.min(self.labels.len());
        self.labels.drain(..labels);
        self.cursor = None;
        self.attribution_drawn = false;
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
        self.render_scrollback(screen, screen.scrollback(), enabled, direction, out)
    }

    pub fn render_scrollback(
        &mut self,
        screen: &vt100::Screen,
        offset: usize,
        enabled: bool,
        direction: Direction,
        out: &mut impl Write,
    ) -> io::Result<()> {
        let offset = offset.min(screen.retained_rows());
        let (height, width) = screen.size();
        let settings = (enabled, direction, (height, width), offset);
        if self.settings != Some(settings) {
            self.invalidate();
            self.settings = Some(settings);
        }
        let mut updates = Vec::new();
        let grid = screen.viewport_rows_at(offset);
        let formats = if self.pretty {
            super::formatting::detect(
                &grid,
                (!screen.hide_cursor() && offset == 0).then_some(screen.cursor_position().0),
            )
        } else {
            vec![super::formatting::RowFormat::default(); usize::from(height)]
        };
        let labels = super::formatting::speaker_labels(&grid, self.agent_label.as_deref());
        for row in 0..height {
            let index = usize::from(row);
            let cells = &grid[index];
            if self.source.get(index) == Some(cells)
                && self.formats.get(index) == Some(&formats[index])
                && self.labels.get(index) == Some(&labels[index])
            {
                continue;
            }
            let visual = formatted_row(cells, enabled, direction, self.layout, &formats[index]);
            if self.rows.get(index) != Some(&visual)
                || self.labels.get(index) != Some(&labels[index])
            {
                write!(updates, "\x1b[{};1H\x1b[0m", row + 1)?;
                self.write_row(&visual, labels[index].as_deref(), &mut updates)?;
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
        let cursor = (row, visual_col, screen.hide_cursor() || offset > 0);
        if !updates.is_empty() || self.cursor != Some(cursor) {
            out.write_all(b"\x1b[?25l")?;
            out.write_all(&updates)?;
            if self.attribution && !self.attribution_drawn {
                let credit = "Powered by: Gal Havkin";
                let visible = &credit[..credit.len().min(usize::from(width + self.margin()))];
                write!(
                    out,
                    "\x1b[{};1H\x1b[0;1;36m{}\x1b[0m\x1b[K",
                    height + 1,
                    visible
                )?;
                self.attribution_drawn = true;
            }
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

/// Original text and hyperlinks for exit replay; Markdown decoration is omitted.
pub fn replay(
    screen: &vt100::Screen,
    enabled: bool,
    direction: Direction,
    layout: Layout,
) -> Vec<u8> {
    let renderer = Renderer::default();
    let mut out = Vec::new();
    for cells in screen.history_since(0).chain(screen.live_rows()) {
        let mut visual = visual_row_with_layout(&cells, enabled, direction, layout);
        for glyph in &mut visual.glyphs {
            glyph.style = Style::default();
        }
        renderer
            .write_row(&visual, None, &mut out)
            .expect("writing to Vec");
        out.extend_from_slice(b"\x1b[0m\r\n");
    }
    out
}
