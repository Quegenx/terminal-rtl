use unicode_bidi::{BidiInfo, Level};
use unicode_bidi_mirroring::get_mirrored;

use super::{Direction, Layout, Style};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Glyph {
    pub text: String,
    pub width: u16,
    pub style: Style,
    pub hyperlink: Option<std::sync::Arc<vt100::Hyperlink>>,
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
    visual_row_with_layout(cells, enabled, direction, Layout::Columns)
}

pub fn visual_row_with_layout(
    cells: &[vt100::Cell],
    enabled: bool,
    direction: Direction,
    layout: Layout,
) -> VisualRow {
    formatted_row(
        cells,
        enabled,
        direction,
        layout,
        &super::formatting::RowFormat::default(),
    )
}

pub(super) fn formatted_row(
    cells: &[vt100::Cell],
    enabled: bool,
    direction: Direction,
    layout: Layout,
    format: &super::formatting::RowFormat,
) -> VisualRow {
    let cols = cells.len();
    let mut glyphs: Vec<Glyph> = cells
        .iter()
        .enumerate()
        .filter(|(_, cell)| !cell.is_wide_continuation())
        .map(|(col, cell)| Glyph {
            text: if cell.is_wide() && col + 1 >= cols {
                " ".into()
            } else if cell.has_contents() {
                cell.contents().to_owned()
            } else {
                " ".into()
            },
            width: if cell.is_wide() && col + 1 < cols {
                2
            } else {
                1
            },
            style: Style::from(cell),
            hyperlink: cell.hyperlink().cloned(),
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
                if layout == Layout::Columns
                    && is_space(&glyphs[end])
                    && glyphs.get(end + 1).is_some_and(is_space)
                {
                    break;
                }
                end += 1;
            }
            while end > start && is_space(&glyphs[end - 1]) {
                end -= 1;
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
