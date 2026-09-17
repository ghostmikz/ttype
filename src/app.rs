use std::time::Duration;

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::history::History;
use crate::test::{MODES, Mode, Summary, Test};
use crate::ui;
use crate::words::Lang;

const FRAME: Duration = Duration::from_millis(50);

pub enum Screen {
    Typing,
    Results {
        summary: Summary,
        previous_best: Option<f64>,
    },
}

pub struct App {
    pub test: Test,
    pub screen: Screen,
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
        self.screen = Screen::Results {
            summary,
            previous_best,
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
            _ if matches!(self.screen, Screen::Results { .. }) => {
                if key.code == KeyCode::Enter {
                    self.restart(mode, lang);
                }
            }
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
            println!("{}\n", render(&app));
        }
    }
}
