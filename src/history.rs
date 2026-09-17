use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::test::Summary;

/// one line of ~/.local/share/ttype/history.jsonl
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Record {
    pub timestamp: u64,
    pub mode: String,
    pub lang: String,
    pub wpm: f64,
    pub raw: f64,
    pub accuracy: f64,
    pub consistency: f64,
}

pub struct History {
    path: Option<PathBuf>,
    records: Vec<Record>,
}

impl History {
    pub fn load() -> History {
        let path = dirs::data_dir().map(|d| d.join("ttype").join("history.jsonl"));
        let records = path
            .as_ref()
            .and_then(|p| fs::File::open(p).ok())
            .map(|f| {
                BufReader::new(f)
                    .lines()
                    .map_while(Result::ok)
                    // skip lines a newer/older version can't parse instead of failing
                    .filter_map(|l| serde_json::from_str(&l).ok())
                    .collect()
            })
            .unwrap_or_default();
        History { path, records }
    }

    pub fn best(&self, mode: &str, lang: &str) -> Option<f64> {
        self.records
            .iter()
            .filter(|r| r.mode == mode && r.lang == lang)
            .map(|r| r.wpm)
            .max_by(f64::total_cmp)
    }

    /// appends to disk; a write failure only loses history, never the session
    pub fn add(&mut self, s: &Summary) -> std::io::Result<()> {
        let record = Record {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            mode: s.mode.key(),
            lang: s.lang.code().to_string(),
            wpm: round2(s.wpm),
            raw: round2(s.raw),
            accuracy: round2(s.accuracy),
            consistency: round2(s.consistency),
        };
        let line = serde_json::to_string(&record).map_err(std::io::Error::other)?;
        self.records.push(record);

        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut f = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(f, "{line}")
    }
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}
