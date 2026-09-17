use std::time::Instant;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::motion;
use super::{ACCENT, DARK_RED, DIM, FG, RED, centered};
use crate::app::App;
use crate::test::{Mode, Test};

const VISIBLE_LINES: usize = 3;
/// normal text grows with the terminal up to this many columns
const NORMAL_MAX_COLS: u16 = 110;

struct Placed {
    word: usize,
    line: usize,
    col: usize,
}

struct Wrapped {
    words: Vec<Placed>,
    caret_col: usize,
    caret_line: usize,
}

/// each word is as wide as the longer of its target and what was typed
fn display_len(test: &Test, i: usize) -> usize {
    let typed = test.typed.get(i).map_or(0, Vec::len);
    test.words[i].len().max(typed)
}

fn wrap(test: &Test, width: usize) -> Wrapped {
    let cur = test.current();
    let mut out = Wrapped {
        words: Vec::new(),
        caret_col: 0,
        caret_line: 0,
    };
    let (mut line, mut col) = (0, 0);
    for i in 0..test.words.len() {
        let w = display_len(test, i);
        if col > 0 {
            if col + 1 + w > width {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        if i == cur {
            out.caret_col = col + test.typed[i].len();
            out.caret_line = line;
        }
        // lay out one line past what can be shown, for scrolling
        if i > cur && line > out.caret_line + VISIBLE_LINES {
            break;
        }
        out.words.push(Placed { word: i, line, col });
        col += w;
    }
    out
}

/// character and colour for position `j` of word `i`
fn letter(test: &Test, i: usize, j: usize) -> (char, Color) {
    let typed = test.typed.get(i).and_then(|t| t.get(j));
    match (typed, test.words[i].get(j)) {
        (Some(a), Some(b)) if a == b => (*b, FG),
        // show what should have been typed, in red
        (Some(_), Some(b)) => (*b, RED),
        (Some(a), None) => (*a, DARK_RED),
        (None, Some(b)) => (*b, DIM),
        (None, None) => (' ', DIM),
    }
}

fn wrong_past(test: &Test, i: usize) -> bool {
    i < test.current() && test.typed[i] != test.words[i]
}

fn animate(app: &App, wrapped: &Wrapped) -> motion::Frame {
    let target = motion::Frame {
        caret_col: wrapped.caret_col as f64,
        caret_line: wrapped.caret_line as f64,
        // keep the active line second from the top once past the first line
        scroll: wrapped.caret_line.saturating_sub(1) as f64,
    };
    app.motion.borrow_mut().update(target, Instant::now())
}

fn progress(test: &Test) -> String {
    match test.mode {
        Mode::Time(_) => test.remaining_secs().unwrap_or(0).to_string(),
        Mode::Words(n) => format!("{}/{}", test.current().min(n), n),
    }
}

fn live_stats(app: &App) -> Span<'static> {
    let test = &app.test;
    let text = if test.start.is_some() {
        format!("{:.0} wpm   {:.0}%", test.live_wpm(), test.live_accuracy())
    } else if let Some(b) = app.best {
        format!("best {b:.0} wpm")
    } else {
        String::new()
    };
    Span::styled(text, Style::new().fg(DIM))
}

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let test = &app.test;
    let width = (area.width * 3 / 4)
        .clamp(40.min(area.width), NORMAL_MAX_COLS)
        .min(area.width);
    // a blank row between lines once there's space for it
    let gap = u16::from(area.height >= 16);
    let pitch = 1 + gap;
    let text_h = VISIBLE_LINES as u16 * pitch - gap;
    let [status, _, text] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(text_h),
    ])
    .areas(centered(area, width, text_h + 2));

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                progress(test),
                Style::new().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            live_stats(app),
        ])),
        status,
    );

    let wrapped = wrap(test, width as usize);
    let f = animate(app, &wrapped);
    // whole cells only: text scrolls a line at a time, the caret a cell at a time
    let first = f.scroll.round() as usize;
    let buf = frame.buffer_mut();

    for p in &wrapped.words {
        if p.line < first || p.line >= first + VISIBLE_LINES {
            continue;
        }
        let y = text.y + (p.line - first) as u16 * pitch;
        let underline = wrong_past(test, p.word);
        for j in 0..display_len(test, p.word) {
            let x = text.x + (p.col + j) as u16;
            if x >= text.right() {
                break;
            }
            let (ch, color) = letter(test, p.word, j);
            let mut style = Style::new().fg(color);
            if underline {
                style = style
                    .add_modifier(Modifier::UNDERLINED)
                    .underline_color(RED);
            }
            buf[(x, y)].set_char(ch).set_style(style);
        }
    }

    let row = f.caret_line.round() as isize - first as isize;
    let x = text.x + f.caret_col.round() as u16;
    if (0..VISIBLE_LINES as isize).contains(&row) && x < text.right() {
        let y = text.y + row as u16 * pitch;
        buf[(x, y)].set_style(
            Style::new()
                .fg(ACCENT)
                .add_modifier(Modifier::UNDERLINED | Modifier::BOLD)
                .underline_color(ACCENT),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_with(words: &[&str]) -> Test {
        let mut t = Test::new(Mode::Words(words.len()));
        t.words = words.iter().map(|w| w.chars().collect()).collect();
        t
    }

    #[test]
    fn wrap_breaks_lines_and_tracks_caret() {
        let mut t = test_with(&["aaa", "bbb", "ccc", "ddd"]);
        let w = wrap(&t, 7);
        let lines: Vec<(usize, usize)> = w.words.iter().map(|p| (p.line, p.col)).collect();
        assert_eq!(lines, [(0, 0), (0, 4), (1, 0), (1, 4)]);
        assert_eq!((w.caret_col, w.caret_line), (0, 0));

        for c in "aaa bbb c".chars() {
            t.type_char(c);
        }
        let w = wrap(&t, 7);
        assert_eq!((w.caret_col, w.caret_line), (1, 1));
    }

    #[test]
    fn overflowing_word_pushes_the_rest_along() {
        let mut t = test_with(&["ab", "cd"]);
        for c in "abxx".chars() {
            t.type_char(c);
        }
        let w = wrap(&t, 20);
        assert_eq!(w.words[1].col, 5, "ab + xx extras + space");
        assert_eq!(w.caret_col, 4);
    }
}
