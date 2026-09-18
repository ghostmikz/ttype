use std::cell::RefCell;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use ratatui::DefaultTerminal;
use ratatui::crossterm::cursor::SetCursorStyle;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::crossterm::execute;

use crate::history::History;
use crate::test::{KeyStat, MODES, Mode, Summary, Test};
use crate::ui::{self, motion::Motion};

/// redraw interval while idle (the timer still needs to tick)
const IDLE_FRAME: Duration = Duration::from_millis(50);
/// redraw interval while the caret or text is gliding
const ANIMATION_FRAME: Duration = Duration::from_millis(8);

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

pub struct Results {
    pub summary: Summary,
    pub previous_best: Option<f64>,
    pub view: ResultView,
    /// key stats over every saved run, this one included
    pub all_time: BTreeMap<char, KeyStat>,
    pub all_time_runs: usize,
}

pub enum Screen {
    Typing,
    Results(Results),
}

pub struct App {
    pub test: Test,
    pub screen: Screen,
    /// personal best for the current mode, shown before a test starts
    pub best: Option<f64>,
    /// eased caret/scroll positions; drawing advances them, hence the RefCell
    pub motion: RefCell<Motion>,
    /// kept across tests so the heatmap stays up if that's what you were looking at
    view: ResultView,
    history: History,
    quit: bool,
}

impl App {
    pub fn new(mode: Mode) -> App {
        App::with_history(mode, History::load())
    }

    fn with_history(mode: Mode, history: History) -> App {
        App {
            test: Test::new(mode),
            screen: Screen::Typing,
            best: history.best(&mode.key()),
            motion: RefCell::default(),
            view: ResultView::Chart,
            history,
            quit: false,
        }
    }

    pub fn run(mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        // a bar caret sitting before the next letter, monkeytype style. not every
        // terminal honours it, so a failure here isn't worth ending the run over.
        let _ = execute!(std::io::stdout(), SetCursorStyle::BlinkingBar);
        let result = self.event_loop(terminal);
        let _ = execute!(std::io::stdout(), SetCursorStyle::DefaultUserShape);
        result
    }

    fn event_loop(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        while !self.quit {
            terminal.draw(|f| ui::draw(f, self))?;
            let wait = if self.motion.borrow().animating(Instant::now()) {
                ANIMATION_FRAME
            } else {
                IDLE_FRAME
            };
            // handle everything already queued before drawing again, so fast
            // typing never falls a frame per key behind
            if event::poll(wait)? {
                loop {
                    if let Event::Key(key) = event::read()?
                        && key.kind == KeyEventKind::Press
                    {
                        self.on_key(key);
                    }
                    if self.quit || !event::poll(Duration::ZERO)? {
                        break;
                    }
                }
            }
            self.test.tick();
            if matches!(self.screen, Screen::Typing) && self.test.is_finished() {
                self.show_results();
            }
        }
        Ok(())
    }

    fn restart(&mut self, mode: Mode) {
        self.test = Test::new(mode);
        self.best = self.history.best(&mode.key());
        self.screen = Screen::Typing;
        self.motion.borrow_mut().reset();
    }

    fn show_results(&mut self) {
        let summary = self.test.summary();
        let previous_best = self.best;
        // a write failure (read-only home, full disk) just means this run isn't saved
        let _ = self.history.add(&summary);
        let (all_time, all_time_runs) = self.history.key_totals();
        self.screen = Screen::Results(Results {
            summary,
            previous_best,
            view: self.view,
            all_time,
            all_time_runs,
        });
    }

    fn cycle_mode(&mut self, step: isize) {
        let i = MODES.iter().position(|m| *m == self.test.mode).unwrap_or(0) as isize;
        let next = MODES[(i + step).rem_euclid(MODES.len() as isize) as usize];
        self.restart(next);
    }

    fn on_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let mode = self.test.mode;
        match key.code {
            KeyCode::Esc => self.quit = true,
            KeyCode::Char('c') if ctrl => self.quit = true,
            KeyCode::Tab => self.restart(mode),
            KeyCode::Left => self.cycle_mode(-1),
            KeyCode::Right => self.cycle_mode(1),
            _ if matches!(self.screen, Screen::Results(_)) => match key.code {
                KeyCode::Enter => self.restart(mode),
                KeyCode::Char('h') => {
                    if let Screen::Results(r) = &mut self.screen {
                        r.view = r.view.next();
                        self.view = r.view;
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
    use ratatui::backend::{Backend, TestBackend};
    use ratatui::buffer::Buffer;
    use ratatui::style::{Color, Modifier};

    fn app(mode: Mode) -> App {
        App::with_history(mode, History::empty())
    }

    fn press(app: &mut App, s: &str) {
        for c in s.chars() {
            app.on_key(KeyEvent::from(KeyCode::Char(c)));
        }
    }

    fn render(app: &App, w: u16, h: u16) -> Buffer {
        draw(app, w, h).0
    }

    /// the drawn screen plus where the caret (the terminal's cursor) ended up
    fn draw(app: &App, w: u16, h: u16) -> (Buffer, (u16, u16)) {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| ui::draw(f, app)).unwrap();
        let caret = term.backend_mut().get_cursor_position().unwrap();
        (term.backend().buffer().clone(), (caret.x, caret.y))
    }

    /// type the first `n` words, fumbling a couple of them
    fn type_some(app: &mut App, n: usize) {
        let words: Vec<String> = app
            .test
            .words
            .iter()
            .take(n)
            .map(|w| w.iter().collect())
            .collect();
        for (i, w) in words.iter().enumerate() {
            let typed = match i {
                2 => w[..w.len() - 1].to_string(),
                5 => format!("{w}xx"),
                _ => w.clone(),
            };
            press(app, &format!("{typed} "));
        }
    }

    fn finished() -> App {
        let mut app = app(Mode::Time(30));
        type_some(&mut app, 14);
        app.test.start = Some(Instant::now() - Duration::from_secs(30));
        app.test.tick();
        app.show_results();
        app
    }

    #[test]
    fn every_screen_renders_at_every_size() {
        for (w, h) in [(20, 8), (40, 12), (80, 24), (120, 36), (211, 53), (300, 80)] {
            let mut a = app(Mode::Words(25));
            render(&a, w, h);
            type_some(&mut a, 8);
            render(&a, w, h);

            let mut a = finished();
            for _ in 0..3 {
                render(&a, w, h);
                press(&mut a, "h");
            }
        }
    }

    #[test]
    fn normal_text_widens_with_the_terminal() {
        let a = app(Mode::Words(100));
        let longest_line = |buf: &Buffer| {
            (0..buf.area.height)
                .map(|y| {
                    let row: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
                    row.trim().len()
                })
                .max()
                .unwrap()
        };
        let narrow = longest_line(&render(&a, 80, 24));
        let wide = longest_line(&render(&a, 160, 40));
        assert!(wide > narrow + 20, "{narrow} -> {wide}");
    }

    #[test]
    fn caret_glides_instead_of_jumping() {
        let mut a = app(Mode::Words(25));
        let (start, row) = draw(&a, 100, 30).1;
        let word: String = a.test.words[0].iter().collect();
        press(&mut a, &format!("{word} "));
        let target = start + word.len() as u16 + 1;

        // right after the keypress it has only started moving
        let (moving, _) = draw(&a, 100, 30).1;
        assert!(moving < target, "caret teleported to {moving}");

        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(draw(&a, 100, 30).1, (target, row));
        assert!(!a.motion.borrow().animating(Instant::now()));
    }

    #[test]
    fn restart_puts_caret_straight_back() {
        let mut a = app(Mode::Words(25));
        let home = draw(&a, 100, 30).1;
        type_some(&mut a, 3);
        render(&a, 100, 30);
        a.on_key(KeyEvent::from(KeyCode::Tab));
        assert_eq!(draw(&a, 100, 30).1, home);
    }

    #[test]
    fn live_numbers_only_change_once_a_second() {
        let mut a = app(Mode::Time(30));
        let line = |a: &App| {
            let buf = render(a, 100, 30);
            (0..buf.area.height)
                .map(|y| {
                    (0..buf.area.width)
                        .map(|x| buf[(x, y)].symbol())
                        .collect::<String>()
                })
                .find(|row| row.contains("wpm"))
                .expect("status line with the live numbers")
        };
        type_some(&mut a, 2);
        let first = line(&a);
        assert!(
            first.contains("0 wpm"),
            "no numbers before the first second: {first}"
        );
        type_some(&mut a, 2);
        assert_eq!(line(&a), first, "numbers must hold until the second ticks");

        a.test.start = Some(Instant::now() - Duration::from_secs(3));
        a.test.tick();
        assert_ne!(line(&a), first, "a completed second refreshes them");
    }

    fn css(c: Color, default: &str) -> String {
        match c {
            Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
            _ => default.to_string(),
        }
    }

    fn to_html(buf: &Buffer) -> String {
        let mut html = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                let cell = &buf[(x, y)];
                let sym = match cell.symbol() {
                    "<" => "&lt;",
                    ">" => "&gt;",
                    "&" => "&amp;",
                    s => s,
                };
                let underline = if cell.modifier.contains(Modifier::UNDERLINED) {
                    format!(
                        ";text-decoration:underline {}",
                        css(cell.underline_color, "currentColor")
                    )
                } else {
                    String::new()
                };
                html.push_str(&format!(
                    "<span style='color:{};background:{}{underline}'>{sym}</span>",
                    css(cell.fg, "#abb2bf"),
                    css(cell.bg, "transparent"),
                ));
            }
            html.push('\n');
        }
        html
    }

    /// TTYPE_PREVIEW=out.html [TTYPE_PREVIEW_SIZE=211x53] cargo test html_preview -- --ignored
    #[test]
    #[ignore]
    fn html_preview() {
        let out = std::env::var("TTYPE_PREVIEW").expect("set TTYPE_PREVIEW");
        let (w, h): (u16, u16) = std::env::var("TTYPE_PREVIEW_SIZE")
            .ok()
            .and_then(|s| {
                let (a, b) = s.split_once('x')?;
                Some((a.parse().ok()?, b.parse().ok()?))
            })
            .unwrap_or((211, 53));
        let mut html = String::from(
            "<body style='background:#282c34;margin:0'><pre style='font:10px/12px 'DejaVu Sans Mono',monospace;margin:8px'>",
        );
        let hr = "<hr style='border-color:#3e4451'>";

        let mut a = app(Mode::Words(50));
        type_some(&mut a, 16);
        press(&mut a, "wor");
        render(&a, w, h);
        std::thread::sleep(Duration::from_millis(200));
        html += &to_html(&render(&a, w, h));
        html += hr;

        let mut a = finished();
        if let Screen::Results(r) = &mut a.screen {
            let sample = [
                ('e', 9, 4),
                ('r', 5, 3),
                ('t', 7, 1),
                ('a', 6, 0),
                ('o', 6, 2),
                ('s', 3, 1),
                ('i', 5, 0),
            ];
            r.summary.keys = sample
                .iter()
                .map(|&(c, attempts, misses)| (c, KeyStat { attempts, misses }))
                .collect();
        }
        for _ in 0..2 {
            html += &to_html(&render(&a, w, h));
            html += hr;
            press(&mut a, "h");
        }
        std::fs::write(out, html + "</pre></body>").unwrap();
    }
}
