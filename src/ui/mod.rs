mod keyboard;
pub mod motion;
mod results;
mod typing;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, Screen};
use crate::test::MODES;

// One Dark
const FG: Color = Color::Rgb(0xab, 0xb2, 0xbf);
const DIM: Color = Color::Rgb(0x5c, 0x63, 0x70);
const FAINT: Color = Color::Rgb(0x3e, 0x44, 0x51);
const RED: Color = Color::Rgb(0xe0, 0x6c, 0x75);
const DARK_RED: Color = Color::Rgb(0x9a, 0x3f, 0x47);
pub(crate) const ACCENT: Color = Color::Rgb(0xe5, 0xc0, 0x7b);
const GREEN: Color = Color::Rgb(0x98, 0xc3, 0x79);

pub fn draw(frame: &mut Frame, app: &App) {
    let [header, _, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(frame.area().inner(Margin::new(2, 1)));

    draw_header(frame, header, app);
    match &app.screen {
        Screen::Typing => typing::draw(frame, body, app),
        Screen::Results(results) => results::draw(frame, body, results),
    }
    draw_footer(frame, footer, app);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let mut spans = vec![
        Span::styled(
            "ttype",
            Style::new().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
    ];
    for (i, m) in MODES.iter().enumerate() {
        if i == 4 {
            spans.push(Span::styled(" │ ", Style::new().fg(FAINT)));
        }
        let color = if *m == app.test.mode { ACCENT } else { DIM };
        spans.push(Span::styled(
            format!(" {} ", m.label()),
            Style::new().fg(color),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let keys: &[(&str, &str)] = match app.screen {
        Screen::Typing => &[("tab", "restart"), ("←→", "mode"), ("esc", "quit")],
        Screen::Results(_) => &[
            ("tab/enter", "next test"),
            ("h", "chart/heatmap"),
            ("←→", "mode"),
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
