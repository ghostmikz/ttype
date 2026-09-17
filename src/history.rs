use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::test::{KeyStat, Summary};

/// one line of ~/.local/share/ttype/history.jsonl
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Record {
    pub timestamp: u64,
    pub mode: String,
    /// every run is english now; older mongolian runs are kept on disk but ignored
    #[serde(default = "english")]
    pub lang: String,
    pub wpm: f64,
    pub raw: f64,
    pub accuracy: f64,
    pub consistency: f64,
    /// key -> [attempts, misses]; absent in records from before the heatmap
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub keys: BTreeMap<char, [u32; 2]>,
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

    /// in-memory only, for tests that shouldn't touch ~/.local/share
    #[cfg(test)]
    pub fn empty() -> History {
        History {
            path: None,
            records: Vec::new(),
        }
    }

    pub fn best(&self, mode: &str) -> Option<f64> {
        self.records
            .iter()
            .filter(|r| r.mode == mode && r.lang == LANG)
            .map(|r| r.wpm)
            .max_by(f64::total_cmp)
    }

    /// key stats summed over every saved run, plus how many runs had any
    pub fn key_totals(&self) -> (BTreeMap<char, KeyStat>, usize) {
        let mut totals: BTreeMap<char, KeyStat> = BTreeMap::new();
        let mut runs = 0;
        for r in self
            .records
            .iter()
            .filter(|r| r.lang == LANG && !r.keys.is_empty())
        {
            runs += 1;
            for (&c, &[attempts, misses]) in &r.keys {
                totals
                    .entry(c)
                    .or_default()
                    .add(KeyStat { attempts, misses });
            }
        }
        (totals, runs)
    }

    /// appends to disk; a write failure only loses history, never the session
    pub fn add(&mut self, s: &Summary) -> std::io::Result<()> {
        let record = Record {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            mode: s.mode.key(),
            lang: LANG.to_string(),
            wpm: round2(s.wpm),
            raw: round2(s.raw),
            accuracy: round2(s.accuracy),
            consistency: round2(s.consistency),
            keys: s
                .keys
                .iter()
                .map(|(&c, k)| (c, [k.attempts, k.misses]))
                .collect(),
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

const LANG: &str = "en";

fn english() -> String {
    LANG.to_string()
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(lang: &str, wpm: f64, keys: &[(char, [u32; 2])]) -> Record {
        Record {
            timestamp: 1,
            mode: "words:25".into(),
            lang: lang.into(),
            wpm,
            raw: wpm,
            accuracy: 100.0,
            consistency: 100.0,
            keys: keys.iter().copied().collect(),
        }
    }

    #[test]
    fn old_records_without_keys_or_lang_still_load() {
        let old = r#"{"timestamp":1,"mode":"time:30","wpm":90.0,"raw":95.0,"accuracy":97.0,"consistency":80.0}"#;
        let r: Record = serde_json::from_str(old).unwrap();
        assert!(r.keys.is_empty());
        assert_eq!(r.lang, "en");
    }

    #[test]
    fn keys_round_trip() {
        let r = record("en", 1.0, &[('e', [40, 1])]);
        let json = serde_json::to_string(&r).unwrap();
        let back: Record = serde_json::from_str(&json).unwrap();
        assert_eq!(back.keys[&'e'], [40, 1]);
    }

    #[test]
    fn old_mongolian_runs_are_ignored() {
        let h = History {
            path: None,
            records: vec![
                record("en", 80.0, &[('e', [10, 2])]),
                record("en", 95.0, &[('e', [10, 1])]),
                record("mn", 120.0, &[('ө', [10, 5])]),
            ],
        };
        assert_eq!(h.best("words:25"), Some(95.0));
        let (totals, runs) = h.key_totals();
        assert_eq!(runs, 2);
        assert_eq!(
            totals[&'e'],
            KeyStat {
                attempts: 20,
                misses: 3
            }
        );
        assert!(!totals.contains_key(&'ө'));
    }
}
