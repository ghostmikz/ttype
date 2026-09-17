//! Keyboard heatmap: colours each key by how often it was mistyped.

use std::collections::BTreeMap;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::test::KeyStat;
use crate::words::Lang;

/// a key needs this many attempts across all runs before its error rate is ranked
pub const MIN_ATTEMPTS: u32 = 20;

/// rows of the physical keyboard, each with its stagger in columns
struct KeyRows {
    rows: [&'static str; 4],
    offsets: [u16; 4],
}

const US: KeyRows = KeyRows {
    rows: ["1234567890-=", "qwertyuiop[]", "asdfghjkl;'", "zxcvbnm,./"],
    offsets: [0, 2, 3, 5],
};

/// the standard Mongolian Cyrillic layout (xkb `mn`): е and щ sit on - and =
const MN: KeyRows = KeyRows {
    rows: ["1234567890ещ", "фцужэнгшүзкъ", "йыбөахролдп", "ячёсмитьвю"],
    offsets: [0, 2, 3, 5],
};

const KEY_W: u16 = 4; // " k " plus a gap

const BG: (u8, u8, u8) = (0x28, 0x2c, 0x34);
const UNUSED: (u8, u8, u8) = (0x21, 0x25, 0x2b);
const CLEAN: (u8, u8, u8) = (0x2f, 0x34, 0x3f);
const COOL: (u8, u8, u8) = (0x5a, 0x3a, 0x40);
const HOT: (u8, u8, u8) = (0xe0, 0x6c, 0x75);
const FG: Color = Color::Rgb(0xab, 0xb2, 0xbf);
const DIM: Color = Color::Rgb(0x5c, 0x63, 0x70);
const FAINT: Color = Color::Rgb(0x3e, 0x44, 0x51);

pub enum Scale {
    /// raw miss counts -- a single test is too small for rates to mean much
    Count,
    /// misses / attempts, ignoring keys with too few attempts
    Rate,
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

pub fn draw(
    frame: &mut Frame,
    area: Rect,
    lang: Lang,
    keys: &BTreeMap<char, KeyStat>,
    scale: Scale,
) {
    let layout = match lang {
        Lang::En => &US,
        Lang::Mn => &MN,
    };
    let heat = heat(keys, &scale);

    let width = layout
        .rows
        .iter()
        .zip(layout.offsets)
        .map(|(r, off)| off + r.chars().count() as u16 * KEY_W - 1)
        .max()
        .unwrap_or(0);
    let pad = " ".repeat(area.width.saturating_sub(width) as usize / 2);

    let mut lines = Vec::new();
    // breathing room under the heading when the terminal is tall enough
    if area.height > 9 {
        lines.push(Line::default());
    }
    for (row, off) in layout.rows.iter().zip(layout.offsets) {
        let mut spans = vec![Span::raw(pad.clone()), Span::raw(" ".repeat(off as usize))];
        for c in row.chars() {
            spans.push(Span::styled(
                format!(" {c} "),
                key_style(keys.get(&c), heat.get(&c).copied()),
            ));
            spans.push(Span::raw(" "));
        }
        lines.push(Line::from(spans));
        lines.push(Line::default());
    }
    lines.pop();
    lines.push(Line::default());

    // colour scale
    let mut spans = vec![
        Span::raw(pad),
        Span::styled("clean ", Style::new().fg(DIM)),
        Span::styled("   ", Style::new().bg(rgb(CLEAN))),
        Span::raw("  "),
    ];
    for t in [0.0, 0.33, 0.66, 1.0] {
        spans.push(Span::styled(
            "   ",
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
    fn every_word_character_is_on_its_keyboard() {
        for (lang, layout) in [(Lang::En, &US), (Lang::Mn, &MN)] {
            let on_board: String = layout.rows.concat();
            for word in crate::words::generate(lang, 2000, &mut crate::words::Rng::seeded()) {
                for c in word {
                    assert!(on_board.contains(c), "{c} missing from {lang:?} keyboard");
                }
            }
        }
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
