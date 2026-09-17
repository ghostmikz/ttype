use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Axis, Chart, Dataset, GraphType, Paragraph};

use super::keyboard::{self, Scale};
use super::{ACCENT, DARK_RED, DIM, FAINT, FG, GREEN, RED, centered};
use crate::app::{ResultView, Results};
use crate::test::Summary;

pub fn draw(frame: &mut Frame, area: Rect, r: &Results) {
    let s = &r.summary;
    // the chart and heatmap grow with the terminal; the numbers stay put above them
    let width = area.width.min(120);
    let height = area.height.min(42);
    let [top, _, middle, _, bottom] = Layout::vertical([
        Constraint::Length(5),
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(centered(area, width, height));

    draw_headline(frame, top, r);

    let [title, middle] =
        Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(middle);
    let (keys, scale) = match r.view {
        ResultView::Chart => {
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
                title,
            );
            draw_chart(frame, middle, s);
            (&s.keys, Scale::Count)
        }
        ResultView::Heatmap => {
            frame.render_widget(heading("missed keys · this test"), title);
            keyboard::draw(frame, middle, &s.keys, Scale::Count);
            (&s.keys, Scale::Count)
        }
        ResultView::AllTime => {
            let runs = if r.all_time_runs == 1 { "run" } else { "runs" };
            let text = format!("error rate · all {} {runs}", r.all_time_runs);
            frame.render_widget(heading(&text), title);
            keyboard::draw(frame, middle, &r.all_time, Scale::Rate);
            (&r.all_time, Scale::Rate)
        }
    };

    draw_worst_keys(frame, bottom, keys, &scale);
}

fn draw_headline(frame: &mut Frame, area: Rect, r: &Results) {
    let s = &r.summary;
    let [left, right] =
        Layout::horizontal([Constraint::Length(22), Constraint::Fill(1)]).areas(area);

    let best = match r.previous_best {
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
            Line::from(best),
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
            stat("test", s.mode.label()),
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
            ]),
            stat("time", format!("{:.1}s", s.seconds)),
        ]),
        right,
    );
}

fn draw_worst_keys(
    frame: &mut Frame,
    area: Rect,
    keys: &std::collections::BTreeMap<char, crate::test::KeyStat>,
    scale: &Scale,
) {
    let ranked = keyboard::worst(keys, scale);
    let mut spans = vec![Span::styled(
        match scale {
            Scale::Count => "missed keys   ",
            Scale::Rate => "weakest keys  ",
        },
        Style::new().fg(DIM),
    )];
    if ranked.is_empty() {
        let msg = match scale {
            Scale::Count => "none".to_string(),
            Scale::Rate => format!(
                "none yet (a key needs {}+ attempts to rank)",
                keyboard::MIN_ATTEMPTS
            ),
        };
        spans.push(Span::styled(msg, Style::new().fg(GREEN)));
    }
    for (c, k) in ranked.iter().take(8) {
        spans.push(Span::styled(
            c.to_string(),
            Style::new().fg(RED).add_modifier(Modifier::BOLD),
        ));
        let detail = match scale {
            Scale::Count => format!("×{}  ", k.misses),
            Scale::Rate => format!(" {:.0}%  ", k.rate() * 100.0),
        };
        spans.push(Span::styled(detail, Style::new().fg(DIM)));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn heading(title: &str) -> Paragraph<'_> {
    Paragraph::new(Line::from(Span::styled(title, Style::new().fg(DIM))))
        .alignment(Alignment::Center)
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
