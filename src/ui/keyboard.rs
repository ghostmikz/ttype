//! Keyboard heatmap: colours each key by how often it was mistyped.

use std::collections::BTreeMap;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::{DIM, FAINT, FG};
use crate::test::KeyStat;

/// a key needs this many attempts across all runs before its error rate is ranked
pub const MIN_ATTEMPTS: u32 = 20;

const ROWS: [&str; 4] = ["1234567890-=", "qwertyuiop[]", "asdfghjkl;'", "zxcvbnm,./"];
/// row stagger of a real keyboard, in key widths
const STAGGER: [f64; 4] = [0.0, 0.5, 0.75, 1.25];

const BG: (u8, u8, u8) = (0x28, 0x2c, 0x34);
const UNUSED: (u8, u8, u8) = (0x21, 0x25, 0x2b);
const CLEAN: (u8, u8, u8) = (0x2f, 0x34, 0x3f);
const COOL: (u8, u8, u8) = (0x5a, 0x3a, 0x40);
const HOT: (u8, u8, u8) = (0xe0, 0x6c, 0x75);

pub enum Scale {
    /// raw miss counts -- a single test is too small for rates to mean much
    Count,
    /// misses / attempts, ignoring keys with too few attempts
    Rate,
}

/// key dimensions in cells: face width, height, and blank rows between rows
struct KeySize {
    width: u16,
    height: u16,
    row_gap: u16,
}

const COMPACT: KeySize = KeySize {
    width: 3,
    height: 1,
    row_gap: 1,
};
const LARGE: KeySize = KeySize {
    width: 5,
    height: 3,
    row_gap: 1,
};

impl KeySize {
    fn pitch(&self) -> u16 {
        self.width + 1
    }

    fn board_width(&self) -> u16 {
        ROWS.iter()
            .zip(STAGGER)
            .map(|(r, st)| self.offset(st) + r.len() as u16 * self.pitch() - 1)
            .max()
            .unwrap_or(0)
    }

    fn offset(&self, stagger: f64) -> u16 {
        (stagger * self.pitch() as f64).round() as u16
    }

    /// keyboard rows plus a blank line and the colour scale
    fn total_height(&self) -> u16 {
        4 * self.height + 3 * self.row_gap + 2
    }
}

fn rgb((r, g, b): (u8, u8, u8)) -> Color {
    Color::Rgb(r, g, b)
}

fn lerp(a: (u8, u8, u8), b: (u8, u8, u8), t: f64) -> (u8, u8, u8) {
    let mix = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * t).round() as u8;
    (mix(a.0, b.0), mix(a.1, b.1), mix(a.2, b.2))
}

/// 0..=1 heat for every key that has any misses, relative to the worst key
fn heat(keys: &BTreeMap<char, KeyStat>, scale: &Scale) -> BTreeMap<char, f64> {
    let score = |k: &KeyStat| match scale {
        Scale::Count => k.misses as f64,
        Scale::Rate if k.attempts >= MIN_ATTEMPTS => k.rate(),
        Scale::Rate => 0.0,
    };
    let max = keys.values().map(score).fold(0.0, f64::max);
    keys.iter()
        .filter(|(_, k)| score(k) > 0.0)
        .map(|(&c, k)| (c, score(k) / max))
        .collect()
}

fn key_style(stat: Option<&KeyStat>, heat: Option<f64>) -> Style {
    match (stat, heat) {
        (_, Some(t)) => {
            // start from a visible tint so a single miss still stands out
            let bg = lerp(COOL, HOT, t);
            let fg = if t > 0.55 { rgb(BG) } else { FG };
            Style::new().bg(rgb(bg)).fg(fg)
        }
        (Some(k), None) if k.attempts > 0 => Style::new().bg(rgb(CLEAN)).fg(FG),
        _ => Style::new().bg(rgb(UNUSED)).fg(FAINT),
    }
}

pub fn draw(frame: &mut Frame, area: Rect, keys: &BTreeMap<char, KeyStat>, scale: Scale) {
    let size = if area.width >= LARGE.board_width() + 4 && area.height > LARGE.total_height() {
        LARGE
    } else {
        COMPACT
    };
    let heat = heat(keys, &scale);
    let pad = " ".repeat(area.width.saturating_sub(size.board_width()) as usize / 2);

    let mut lines = Vec::new();
    // breathing room under the heading when there's a spare row
    if area.height > size.total_height() {
        lines.push(Line::default());
    }
    for (row, stagger) in ROWS.iter().zip(STAGGER) {
        // a tall key has its label on the middle line and blank face above/below
        for line in 0..size.height {
            let mut spans = vec![Span::raw(format!(
                "{pad}{}",
                " ".repeat(size.offset(stagger) as usize)
            ))];
            for c in row.chars() {
                let face = if line == size.height / 2 {
                    format!("{c:^w$}", w = size.width as usize)
                } else {
                    " ".repeat(size.width as usize)
                };
                spans.push(Span::styled(
                    face,
                    key_style(keys.get(&c), heat.get(&c).copied()),
                ));
                spans.push(Span::raw(" "));
            }
            lines.push(Line::from(spans));
        }
        for _ in 0..size.row_gap {
            lines.push(Line::default());
        }
    }

    // colour scale
    let swatch = " ".repeat(size.width as usize);
    let mut spans = vec![
        Span::raw(pad),
        Span::styled("clean ", Style::new().fg(DIM)),
        Span::styled(swatch.clone(), Style::new().bg(rgb(CLEAN))),
        Span::raw("  "),
    ];
    for t in [0.0, 0.33, 0.66, 1.0] {
        spans.push(Span::styled(
            swatch.clone(),
            Style::new().bg(rgb(lerp(COOL, HOT, t))),
        ));
    }
    let hot_label = match scale {
        Scale::Count => " most misses",
        Scale::Rate => " highest error rate",
    };
    spans.push(Span::styled(hot_label, Style::new().fg(DIM)));
    lines.push(Line::from(spans));

    frame.render_widget(Paragraph::new(lines), area);
}

/// keys ordered worst first, for the text summary under the heatmap
pub fn worst(keys: &BTreeMap<char, KeyStat>, scale: &Scale) -> Vec<(char, KeyStat)> {
    let mut out: Vec<(char, KeyStat)> = keys
        .iter()
        .filter(|(_, k)| match scale {
            Scale::Count => k.misses > 0,
            Scale::Rate => k.misses > 0 && k.attempts >= MIN_ATTEMPTS,
        })
        .map(|(&c, &k)| (c, k))
        .collect();
    out.sort_by(|a, b| {
        let by = |k: &KeyStat| match scale {
            Scale::Count => k.misses as f64,
            Scale::Rate => k.rate(),
        };
        by(&b.1).total_cmp(&by(&a.1)).then(a.0.cmp(&b.0))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stat(attempts: u32, misses: u32) -> KeyStat {
        KeyStat { attempts, misses }
    }

    #[test]
    fn every_word_character_is_on_the_keyboard() {
        let on_board: String = ROWS.concat();
        for word in crate::words::generate(2000, &mut crate::words::Rng::seeded()) {
            for c in word {
                assert!(on_board.contains(c), "{c} missing from keyboard");
            }
        }
    }

    #[test]
    fn large_keys_fit_in_a_fullscreen_results_panel() {
        assert!(LARGE.board_width() <= 100);
        assert_eq!(COMPACT.board_width(), 49);
    }

    #[test]
    fn rate_scale_ignores_rarely_typed_keys() {
        let keys = BTreeMap::from([('a', stat(100, 10)), ('b', stat(2, 2))]);
        let ranked = worst(&keys, &Scale::Rate);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].0, 'a');
        assert_eq!(heat(&keys, &Scale::Rate).get(&'a'), Some(&1.0));
    }

    #[test]
    fn count_scale_is_relative_to_worst_key() {
        let keys = BTreeMap::from([('a', stat(5, 4)), ('b', stat(5, 1)), ('c', stat(5, 0))]);
        let h = heat(&keys, &Scale::Count);
        assert_eq!(h.get(&'a'), Some(&1.0));
        assert_eq!(h.get(&'b'), Some(&0.25));
        assert_eq!(h.get(&'c'), None);
        assert_eq!(worst(&keys, &Scale::Count)[0].0, 'a');
    }
}
