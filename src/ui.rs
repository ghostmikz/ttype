use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Axis, Chart, Dataset, GraphType, Paragraph};

use crate::app::{App, Screen};
use crate::test::{MODES, Mode, Summary, Test};
use crate::words::Lang;

// One Dark
const FG: Color = Color::Rgb(0xab, 0xb2, 0xbf);
const DIM: Color = Color::Rgb(0x5c, 0x63, 0x70);
const FAINT: Color = Color::Rgb(0x3e, 0x44, 0x51);
const RED: Color = Color::Rgb(0xe0, 0x6c, 0x75);
const DARK_RED: Color = Color::Rgb(0x9a, 0x3f, 0x47);
const ACCENT: Color = Color::Rgb(0xe5, 0xc0, 0x7b);
const GREEN: Color = Color::Rgb(0x98, 0xc3, 0x79);

const TEXT_WIDTH: u16 = 72;
const TEXT_LINES: usize = 3;

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let [header, _, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(area.inner(ratatui::layout::Margin::new(2, 1)));

    draw_header(frame, header, app);
    match &app.screen {
        Screen::Typing => draw_typing(frame, body, &app.test, app.best),
        Screen::Results {
            summary,
            previous_best,
        } => draw_results(frame, body, summary, *previous_best),
    }
    draw_footer(frame, footer, &app.screen);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let mut spans = vec![
        Span::styled(
            "ttype",
            Style::new().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled("   ", Style::new()),
    ];
    let active = app.test.mode;
    for (i, m) in MODES.iter().enumerate() {
        if i == 4 {
            spans.push(Span::styled(" │ ", Style::new().fg(FAINT)));
        }
        let style = if *m == active {
            Style::new().fg(ACCENT)
        } else {
            Style::new().fg(DIM)
        };
        spans.push(Span::styled(format!(" {} ", m.label()), style));
    }
    spans.push(Span::styled(" │ ", Style::new().fg(FAINT)));
    for lang in [Lang::En, Lang::Mn] {
        let style = if lang == app.test.lang {
            Style::new().fg(ACCENT)
        } else {
            Style::new().fg(DIM)
        };
        spans.push(Span::styled(format!(" {} ", lang.code()), style));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_footer(frame: &mut Frame, area: Rect, screen: &Screen) {
    let keys: &[(&str, &str)] = match screen {
        Screen::Typing => &[
            ("tab", "restart"),
            ("←→", "mode"),
            ("↑↓", "language"),
            ("esc", "quit"),
        ],
        Screen::Results { .. } => &[
            ("tab/enter", "next test"),
            ("←→", "mode"),
            ("↑↓", "language"),
            ("esc", "quit"),
        ],
    };
    let mut spans = Vec::new();
    for (i, (k, v)) in keys.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ·  ", Style::new().fg(FAINT)));
        }
        spans.push(Span::styled(*k, Style::new().fg(FG)));
        spans.push(Span::styled(format!(" {v}"), Style::new().fg(DIM)));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).alignment(Alignment::Center),
        area,
    );
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}

fn draw_typing(frame: &mut Frame, area: Rect, test: &Test, best: Option<f64>) {
    let width = TEXT_WIDTH.min(area.width);
    let box_area = centered(area, width, TEXT_LINES as u16 + 2);
    let [status, _, text] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(TEXT_LINES as u16),
    ])
    .areas(box_area);

    // ---- status line: progress + live numbers
    let progress = match test.mode {
        Mode::Time(_) => format!("{}", test.remaining_secs().unwrap_or(0)),
        Mode::Words(n) => format!("{}/{}", test.current().min(n), n),
    };
    let mut spans = vec![Span::styled(
        progress,
        Style::new().fg(ACCENT).add_modifier(Modifier::BOLD),
    )];
    if test.start.is_some() {
        spans.push(Span::styled(
            format!(
                "   {:.0} wpm   {:.0}%",
                test.live_wpm(),
                test.live_accuracy()
            ),
            Style::new().fg(DIM),
        ));
    } else if let Some(b) = best {
        spans.push(Span::styled(
            format!("   best {b:.0} wpm"),
            Style::new().fg(DIM),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), status);

    // ---- word wrap: each word is as wide as the longer of target / typed
    let cur = test.current();
    let mut lines: Vec<Vec<usize>> = vec![Vec::new()];
    let mut line_w = 0usize;
    for i in 0..test.words.len() {
        let w = display_len(test, i);
        let needed = if line_w == 0 { w } else { line_w + 1 + w };
        if needed > width as usize && line_w > 0 {
            lines.push(Vec::new());
            line_w = w;
        } else {
            line_w = needed;
        }
        lines.last_mut().unwrap().push(i);
        // only lay out a little past what can be shown
        if i > cur && lines.len() > TEXT_LINES + 2 {
            break;
        }
    }
    let cur_line = lines.iter().position(|l| l.contains(&cur)).unwrap_or(0);
    // keep the active line second from the top once past the first line
    let first = cur_line.saturating_sub(1);

    let mut rendered = Vec::new();
    let mut cursor = None;
    for (row, line) in lines.iter().skip(first).take(TEXT_LINES).enumerate() {
        let mut spans = Vec::new();
        let mut col = 0u16;
        for (k, &i) in line.iter().enumerate() {
            if k > 0 {
                spans.push(Span::raw(" "));
                col += 1;
            }
            if i == cur {
                cursor = Some((col + test.typed[i].len() as u16, row as u16));
            }
            spans.extend(word_spans(test, i));
            col += display_len(test, i) as u16;
        }
        rendered.push(Line::from(spans));
    }
    frame.render_widget(Paragraph::new(rendered), text);
    if let Some((x, y)) = cursor {
        frame.set_cursor_position(Position::new(text.x + x, text.y + y));
    }
}

fn display_len(test: &Test, i: usize) -> usize {
    let typed = test.typed.get(i).map_or(0, Vec::len);
    test.words[i].len().max(typed)
}

fn word_spans(test: &Test, i: usize) -> Vec<Span<'static>> {
    let word = &test.words[i];
    let cur = test.current();
    let Some(typed) = test.typed.get(i) else {
        return vec![Span::styled(
            word.iter().collect::<String>(),
            Style::new().fg(DIM),
        )];
    };
    let wrong_past = i < cur && typed != word;
    let mut spans = Vec::new();
    for j in 0..word.len().max(typed.len()) {
        let (ch, mut style) = match (typed.get(j), word.get(j)) {
            (Some(a), Some(b)) if a == b => (*b, Style::new().fg(FG)),
            // show what should have been typed, in red
            (Some(_), Some(b)) => (*b, Style::new().fg(RED)),
            (Some(a), None) => (*a, Style::new().fg(DARK_RED)),
            (None, Some(b)) => (*b, Style::new().fg(DIM)),
            (None, None) => unreachable!(),
        };
        if wrong_past {
            style = style
                .add_modifier(Modifier::UNDERLINED)
                .underline_color(RED);
        }
        spans.push(Span::styled(ch.to_string(), style));
    }
    spans
}

fn draw_results(frame: &mut Frame, area: Rect, s: &Summary, previous_best: Option<f64>) {
    let width = 80.min(area.width);
    let height = 20.min(area.height);
    let box_area = centered(area, width, height);
    let [top, _, chart_area, _, bottom] = Layout::vertical([
        Constraint::Length(4),
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(3),
    ])
    .areas(box_area);

    // ---- headline numbers
    let [left, right] =
        Layout::horizontal([Constraint::Length(22), Constraint::Fill(1)]).areas(top);
    let pb = match previous_best {
        None => Span::styled("first run", Style::new().fg(DIM)),
        Some(b) if s.wpm > b => Span::styled(
            "new personal best",
            Style::new().fg(GREEN).add_modifier(Modifier::BOLD),
        ),
        Some(b) => Span::styled(format!("best {b:.0}"), Style::new().fg(DIM)),
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled("wpm", Style::new().fg(DIM))),
            Line::from(Span::styled(
                format!("{:.0}", s.wpm),
                Style::new().fg(ACCENT).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                format!("acc {:.0}%", s.accuracy),
                Style::new().fg(FG),
            )),
            Line::from(pb),
        ]),
        left,
    );

    let stat = |k: &str, v: String| {
        Line::from(vec![
            Span::styled(format!("{k:<13}"), Style::new().fg(DIM)),
            Span::styled(v, Style::new().fg(FG)),
        ])
    };
    let c = &s.chars;
    frame.render_widget(
        Paragraph::new(vec![
            stat("test", format!("{} {}", s.mode.label(), s.lang.code())),
            stat("raw", format!("{:.0}", s.raw)),
            stat("consistency", format!("{:.0}%", s.consistency)),
            Line::from(vec![
                Span::styled(format!("{:<13}", "characters"), Style::new().fg(DIM)),
                Span::styled(c.correct.to_string(), Style::new().fg(FG)),
                Span::styled("/", Style::new().fg(FAINT)),
                Span::styled(c.incorrect.to_string(), Style::new().fg(RED)),
                Span::styled("/", Style::new().fg(FAINT)),
                Span::styled(c.extra.to_string(), Style::new().fg(DARK_RED)),
                Span::styled("/", Style::new().fg(FAINT)),
                Span::styled(c.missed.to_string(), Style::new().fg(DIM)),
                Span::styled(format!("   {:.1}s", s.seconds), Style::new().fg(DIM)),
            ]),
        ]),
        right,
    );

    let [legend, chart_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(chart_area);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("── ", Style::new().fg(ACCENT)),
            Span::styled("wpm   ", Style::new().fg(DIM)),
            Span::styled("── ", Style::new().fg(FAINT)),
            Span::styled("raw   ", Style::new().fg(DIM)),
            Span::styled("• ", Style::new().fg(RED)),
            Span::styled("errors", Style::new().fg(DIM)),
        ]))
        .alignment(Alignment::Right),
        legend,
    );
    draw_chart(frame, chart_area, s);

    // ---- most missed keys
    let mut spans = vec![Span::styled("missed keys  ", Style::new().fg(DIM))];
    if s.missed_keys.is_empty() {
        spans.push(Span::styled("none", Style::new().fg(GREEN)));
    }
    for (c, n) in s.missed_keys.iter().take(10) {
        spans.push(Span::styled(
            c.to_string(),
            Style::new().fg(RED).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(format!("×{n}  "), Style::new().fg(DIM)));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), bottom);
}

fn draw_chart(frame: &mut Frame, area: Rect, s: &Summary) {
    let wpm: Vec<(f64, f64)> = s.samples.iter().map(|p| (p.second as f64, p.wpm)).collect();
    let raw: Vec<(f64, f64)> = s.samples.iter().map(|p| (p.second as f64, p.raw)).collect();
    // errors plotted along the bottom so spikes line up with slowdowns
    let errs: Vec<(f64, f64)> = s
        .samples
        .iter()
        .filter(|p| p.errors > 0)
        .map(|p| (p.second as f64, 0.0))
        .collect();

    let max_x = s.samples.last().map_or(1.0, |p| p.second as f64).max(1.0);
    let max_y = wpm
        .iter()
        .chain(&raw)
        .map(|p| p.1)
        .fold(0.0, f64::max)
        .max(10.0);
    let max_y = (max_y / 20.0).ceil() * 20.0;

    let datasets = vec![
        Dataset::default()
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::new().fg(FAINT))
            .data(&raw),
        Dataset::default()
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::new().fg(ACCENT))
            .data(&wpm),
        Dataset::default()
            .marker(Marker::Dot)
            .graph_type(GraphType::Scatter)
            .style(Style::new().fg(RED))
            .data(&errs),
    ];
    let label = |t: String| Span::styled(t, Style::new().fg(DIM));
    let chart = Chart::new(datasets)
        .x_axis(
            Axis::default()
                .bounds([1.0, max_x])
                .labels([label("1s".into()), label(format!("{max_x:.0}s"))])
                .style(Style::new().fg(FAINT)),
        )
        .y_axis(
            Axis::default()
                .bounds([0.0, max_y])
                .labels([
                    label("0".into()),
                    label(format!("{:.0}", max_y / 2.0)),
                    label(format!("{max_y:.0}")),
                ])
                .style(Style::new().fg(FAINT)),
        );
    frame.render_widget(chart, area);
}
