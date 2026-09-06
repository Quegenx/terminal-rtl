//! Conservative, display-only recognition. Never change the agent's terminal grid.
use crate::display::Glyph;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RowFormat {
    pipes: Vec<u16>,
    separator: bool,
    header: bool,
    heading: u8,
    quote: bool,
    marker: Option<u16>,
    fence: bool,
    code: bool,
}

fn pipes(cells: &[vt100::Cell]) -> Vec<u16> {
    cells
        .iter()
        .enumerate()
        .filter_map(|(i, c)| (c.contents() == "|").then_some(i as u16))
        .collect()
}

fn table_bounds(cells: &[vt100::Cell], positions: &[u16]) -> bool {
    positions.len() >= 3
        && cells[..usize::from(positions[0])]
            .iter()
            .all(|c| c.contents().trim().is_empty())
        && cells[usize::from(*positions.last().unwrap()) + 1..]
            .iter()
            .all(|c| c.contents().trim().is_empty())
}

pub fn detect(rows: &[Vec<vt100::Cell>], cursor: Option<u16>) -> Vec<RowFormat> {
    let mut formats = vec![RowFormat::default(); rows.len()];
    let mut fence = None;
    for (i, cells) in rows.iter().enumerate() {
        let text: String = cells.iter().map(|c| c.contents()).collect();
        let trimmed = text.trim_start();
        if trimmed.starts_with("```") {
            if let Some(start) = fence.take() {
                if cursor.is_none_or(|row| !(start..=i).contains(&usize::from(row))) {
                    for format in &mut formats[start..=i] {
                        format.code = true;
                    }
                    formats[start].fence = true;
                    formats[i].fence = true;
                }
            } else {
                fence = Some(i);
            }
        } else if fence.is_none() && cursor != Some(i as u16) {
            let count = trimmed.chars().take_while(|c| *c == '#').count();
            if (1..=6).contains(&count) && trimmed.as_bytes().get(count) == Some(&b' ') {
                formats[i].heading = count as u8;
            }
            formats[i].quote = trimmed.starts_with("> ") && !trimmed[2..].trim().is_empty();
            if ["- ", "* ", "+ "]
                .iter()
                .any(|prefix| trimmed.starts_with(prefix))
            {
                formats[i].marker = cells
                    .iter()
                    .position(|c| !c.contents().trim().is_empty())
                    .map(|col| col as u16);
            }
        }
        if i == 0 || fence.is_some() || formats[i].code {
            continue;
        }
        let positions = pipes(cells);
        if !table_bounds(cells, &positions) {
            continue;
        }
        let separator = positions.windows(2).all(|pair| {
            let segment = &cells[usize::from(pair[0]) + 1..usize::from(pair[1])];
            segment.iter().filter(|c| c.contents() == "-").count() >= 3
                && segment
                    .iter()
                    .all(|c| matches!(c.contents(), "-" | ":" | " " | ""))
        });
        if !separator || pipes(&rows[i - 1]) != positions || !table_bounds(&rows[i - 1], &positions)
        {
            continue;
        }
        let mut end = i + 1;
        while end < rows.len()
            && pipes(&rows[end]) == positions
            && table_bounds(&rows[end], &positions)
        {
            end += 1;
        }
        if end == i + 1 || cursor.is_some_and(|row| (i - 1..end).contains(&usize::from(row))) {
            continue;
        }
        for (index, format) in formats.iter_mut().enumerate().take(end).skip(i - 1) {
            format.pipes = positions.clone();
            format.header = index == i - 1;
            format.separator = index == i;
        }
    }
    formats
}

impl RowFormat {
    pub fn apply(&self, glyphs: &mut [Glyph]) {
        for glyph in glyphs {
            let col = glyph.logical_col;
            if self.pipes.contains(&col) {
                glyph.text = if self.separator {
                    if self.pipes.first() == Some(&col) {
                        "├"
                    } else if self.pipes.last() == Some(&col) {
                        "┤"
                    } else {
                        "┼"
                    }
                } else {
                    "│"
                }
                .into();
                glyph.style.dim = true;
            } else if self.separator && col > self.pipes[0] && col < *self.pipes.last().unwrap() {
                glyph.text = "─".into();
                glyph.style.dim = true;
            } else if self.header || self.heading > 0 || self.marker == Some(col) {
                glyph.style.bold = true;
            } else if self.fence && glyph.text != " " {
                glyph.style.dim = true;
            }
            if self.heading > 0 {
                glyph.style.underline |= self.heading == 1;
                if glyph.style.fg == vt100::Color::Default {
                    glyph.style.fg = vt100::Color::Idx(match self.heading {
                        1 => 6,
                        2 => 4,
                        _ => 5,
                    });
                }
            }
            glyph.style.italic |= self.quote;
        }
    }
}

/// Native message markers are heuristics; unknown rows keep an empty margin.
pub fn speaker_labels(rows: &[Vec<vt100::Cell>], agent: Option<&str>) -> Vec<Option<String>> {
    let mut fenced = false;
    rows.iter()
        .map(|cells| {
            let agent = agent?;
            let text: String = cells
                .iter()
                .map(|c| if c.has_contents() { c.contents() } else { " " })
                .collect();
            let trimmed = text.trim_start();
            if trimmed.starts_with("```") {
                fenced = !fenced;
                return None;
            }
            if fenced {
                return None;
            }
            let indent = text.chars().take_while(|c| *c == ' ').count();
            let grok_composer = agent == "grok" && indent == 2 && trimmed.starts_with("│ ❯ ");
            let grok_message = agent == "grok" && indent == 5;
            if (indent <= 2 && trimmed.starts_with("› "))
                || (grok_message && trimmed.starts_with("❯ "))
                || grok_composer
            {
                Some("you:".into())
            } else if (indent <= 2
                && ["• ", "● "]
                    .iter()
                    .any(|prefix| trimmed.starts_with(prefix)))
                || (grok_message && timestamped(trimmed))
            {
                Some(format!("{agent}:"))
            } else {
                None
            }
        })
        .collect()
}

// Grok's native transcript puts a clock at the right edge of message starts.
fn timestamped(text: &str) -> bool {
    let Some((body, clock)) = text.trim_end().rsplit_once("  ") else {
        return false;
    };
    if body.trim().is_empty() {
        return false;
    }
    let clock = clock.trim().trim_end_matches(" AM").trim_end_matches(" PM");
    let Some((hour, minute)) = clock.split_once(':') else {
        return false;
    };
    (1..=2).contains(&hour.len())
        && minute.len() == 2
        && hour.parse::<u8>().is_ok_and(|h| h < 24)
        && minute.parse::<u8>().is_ok_and(|m| m < 60)
}
