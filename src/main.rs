mod app;
mod history;
mod test;
mod ui;
mod words;

use std::process::ExitCode;

use test::Mode;

const USAGE: &str = "\
ttype - typing test in your terminal

usage: ttype [-t SECONDS | -w WORDS]

  -t, --time SECONDS   timed test (default: 30)
  -w, --words WORDS    fixed word count
  -h, --help           show this help

keys: tab restart · ←→ mode · esc quit
results: h chart/heatmap
history: ~/.local/share/ttype/history.jsonl";

fn parse_args() -> Result<Mode, String> {
    let mut mode = Mode::Time(30);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "-h" | "--help" => return Err(String::new()),
            "-t" | "--time" => {
                let v = value(&arg)?;
                let secs = v
                    .parse()
                    .ok()
                    .filter(|&s| s > 0)
                    .ok_or(format!("bad time: {v}"))?;
                mode = Mode::Time(secs);
            }
            "-w" | "--words" => {
                let v = value(&arg)?;
                let n = v
                    .parse()
                    .ok()
                    .filter(|&n| n > 0)
                    .ok_or(format!("bad word count: {v}"))?;
                mode = Mode::Words(n);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(mode)
}

fn main() -> ExitCode {
    let mode = match parse_args() {
        Ok(v) => v,
        Err(msg) if msg.is_empty() => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(msg) => {
            eprintln!("ttype: {msg}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match ratatui::run(|terminal| app::App::new(mode).run(terminal)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ttype: {e}");
            ExitCode::FAILURE
        }
    }
}
