use std::collections::BTreeMap;
use std::time::Duration;

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::history::History;
use crate::test::{KeyStat, MODES, Mode, Summary, Test};
use crate::ui;
use crate::words::Lang;

const FRAME: Duration = Duration::from_millis(50);

/// what fills the middle of the results screen; `h` cycles through them
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ResultView {
    Chart,
    Heatmap,
    AllTime,
}

impl ResultView {
    fn next(self) -> ResultView {
        match self {
            ResultView::Chart => ResultView::Heatmap,
            ResultView::Heatmap => ResultView::AllTime,
            ResultView::AllTime => ResultView::Chart,
        }
    }
}

pub enum Screen {
    Typing,
    Results {
        summary: Summary,
        previous_best: Option<f64>,
        view: ResultView,
        /// key stats over every saved run in this language, this one included
        all_time: BTreeMap<char, KeyStat>,
        all_time_runs: usize,
    },
}

pub struct App {
    pub test: Test,
    pub screen: Screen,
    /// kept across tests so the heatmap stays up if that's what you were looking at
    view: ResultView,
    /// personal best for the current mode + language, shown before a test starts
    pub best: Option<f64>,
    history: History,
    quit: bool,
}

impl App {
    pub fn new(mode: Mode, lang: Lang) -> App {
        let history = History::load();
        let best = history.best(&mode.key(), lang.code());
        App {
            test: Test::new(mode, lang),
            screen: Screen::Typing,
            view: ResultView::Chart,
            best,
            history,
            quit: false,
        }
    }

    pub fn run(mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        while !self.quit {
            terminal.draw(|f| ui::draw(f, &self))?;
            if event::poll(FRAME)?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                self.on_key(key);
            }
            self.test.tick();
            if matches!(self.screen, Screen::Typing) && self.test.is_finished() {
                self.show_results();
            }
        }
        Ok(())
    }

    fn restart(&mut self, mode: Mode, lang: Lang) {
        self.test = Test::new(mode, lang);
        self.best = self.history.best(&mode.key(), lang.code());
        self.screen = Screen::Typing;
    }

    fn show_results(&mut self) {
        let summary = self.test.summary();
        let previous_best = self.best;
        // a write failure (read-only home, full disk) just means this run isn't saved
        let _ = self.history.add(&summary);
        let (all_time, all_time_runs) = self.history.key_totals(summary.lang.code());
        self.screen = Screen::Results {
            summary,
            previous_best,
            view: self.view,
            all_time,
            all_time_runs,
        };
    }

    fn cycle_mode(&mut self, step: isize) {
        let i = MODES.iter().position(|m| *m == self.test.mode).unwrap_or(0) as isize;
        let next = MODES[(i + step).rem_euclid(MODES.len() as isize) as usize];
        self.restart(next, self.test.lang);
    }

    fn on_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let (mode, lang) = (self.test.mode, self.test.lang);
        match key.code {
            KeyCode::Esc => self.quit = true,
            KeyCode::Char('c') if ctrl => self.quit = true,
            KeyCode::Tab => self.restart(mode, lang),
            KeyCode::Left => self.cycle_mode(-1),
            KeyCode::Right => self.cycle_mode(1),
            KeyCode::Up | KeyCode::Down => self.restart(mode, lang.toggle()),
            _ if matches!(self.screen, Screen::Results { .. }) => match key.code {
                KeyCode::Enter => self.restart(mode, lang),
                KeyCode::Char('h') => {
                    if let Screen::Results { view, .. } = &mut self.screen {
                        *view = view.next();
                        self.view = *view;
                    }
                }
                _ => {}
            },
            // terminals send ctrl+backspace as ctrl+h, ctrl+w or backspace+ctrl/alt
            KeyCode::Backspace if ctrl || key.modifiers.contains(KeyModifiers::ALT) => {
                self.test.delete_word()
            }
            KeyCode::Char('h' | 'w') if ctrl => self.test.delete_word(),
            KeyCode::Backspace => self.test.backspace(),
            KeyCode::Char(c) if !ctrl => self.test.type_char(c),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::time::Instant;

    fn press(app: &mut App, s: &str) {
        for c in s.chars() {
            app.on_key(KeyEvent::from(KeyCode::Char(c)));
        }
    }

    fn render(app: &App) -> String {
        let mut term = Terminal::new(TestBackend::new(100, 28)).unwrap();
        term.draw(|f| ui::draw(f, app)).unwrap();
        let buf = term.backend().buffer();
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// cargo test preview -- --ignored --nocapture
    #[test]
    #[ignore]
    fn preview() {
        for lang in [Lang::En, Lang::Mn] {
            let mut app = App::new(Mode::Time(30), lang);
            app.best = Some(98.0);
            println!("{}\n", render(&app));

            let words: Vec<String> = app
                .test
                .words
                .iter()
                .take(12)
                .map(|w| w.iter().collect())
                .collect();
            for (i, w) in words.iter().enumerate() {
                // make a few mistakes: drop a letter, add extras
                let typed = match i {
                    2 => w[..w.len() - w.chars().last().unwrap().len_utf8()].to_string(),
                    5 => format!("{w}xx"),
                    _ => w.clone(),
                };
                press(&mut app, &format!("{typed} "));
            }
            press(&mut app, "q");
            println!("{}\n", render(&app));

            app.test.start = Some(Instant::now() - Duration::from_secs(30));
            app.test.tick();
            app.show_results();
            for _ in 0..3 {
                println!("{}\n", render(&app));
                press(&mut app, "h");
            }
        }
    }
}

#[cfg(test)]
mod html_preview {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;

    fn css(c: Color, default: &str) -> String {
        match c {
            Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
            _ => default.to_string(),
        }
    }

    /// TTYPE_PREVIEW=out.html cargo test html_preview -- --ignored
    #[test]
    #[ignore]
    fn html_preview() {
        let out = std::env::var("TTYPE_PREVIEW").expect("set TTYPE_PREVIEW");
        let mut html = String::from(
            "<body style='background:#282c34;margin:0'><pre style='font:15px/19px monospace;margin:12px'>",
        );
        for lang in [Lang::En, Lang::Mn] {
            let mut app = App::new(Mode::Words(25), lang);
            let sample: &[(char, u32, u32)] = match lang {
                Lang::En => &[
                    ('e', 9, 4),
                    ('r', 5, 3),
                    ('t', 7, 1),
                    ('a', 6, 0),
                    ('o', 6, 2),
                    ('n', 4, 0),
                    ('s', 3, 1),
                    ('i', 5, 0),
                    ('h', 3, 0),
                    ('l', 3, 0),
                ],
                Lang::Mn => &[
                    ('ө', 6, 4),
                    ('ү', 5, 2),
                    ('а', 9, 1),
                    ('н', 6, 0),
                    ('х', 4, 3),
                    ('р', 5, 0),
                    ('ж', 2, 1),
                    ('л', 4, 0),
                    ('г', 3, 0),
                ],
            };
            let keys: BTreeMap<char, KeyStat> = sample
                .iter()
                .map(|&(c, attempts, misses)| (c, KeyStat { attempts, misses }))
                .collect();
            let mut summary = app.test.summary();
            summary.keys = keys.clone();
            let all_time: BTreeMap<char, KeyStat> = keys
                .iter()
                .map(|(&c, k)| {
                    (
                        c,
                        KeyStat {
                            attempts: k.attempts * 12,
                            misses: k.misses * 7 + 3,
                        },
                    )
                })
                .collect();
            for view in [ResultView::Heatmap, ResultView::AllTime] {
                app.screen = Screen::Results {
                    summary: summary.clone(),
                    previous_best: Some(90.0),
                    view,
                    all_time: all_time.clone(),
                    all_time_runs: 14,
                };
                let mut term = Terminal::new(TestBackend::new(96, 26)).unwrap();
                term.draw(|f| ui::draw(f, &app)).unwrap();
                let buf = term.backend().buffer();
                for y in 0..buf.area.height {
                    for x in 0..buf.area.width {
                        let cell = &buf[(x, y)];
                        let sym = match cell.symbol() {
                            "<" => "&lt;",
                            ">" => "&gt;",
                            "&" => "&amp;",
                            s => s,
                        };
                        html.push_str(&format!(
                            "<span style='color:{};background:{}'>{}</span>",
                            css(cell.fg, "#abb2bf"),
                            css(cell.bg, "transparent"),
                            sym
                        ));
                    }
                    html.push('\n');
                }
                html.push_str("<hr style='border-color:#3e4451'>");
            }
        }
        std::fs::write(out, html + "</pre></body>").unwrap();
    }
}
