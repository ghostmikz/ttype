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

    pub fn best(&self, mode: &str, lang: &str) -> Option<f64> {
        self.records
            .iter()
            .filter(|r| r.mode == mode && r.lang == lang)
            .map(|r| r.wpm)
            .max_by(f64::total_cmp)
    }

    /// key stats summed over every saved run in `lang`, plus how many runs had any
    pub fn key_totals(&self, lang: &str) -> (BTreeMap<char, KeyStat>, usize) {
        let mut totals: BTreeMap<char, KeyStat> = BTreeMap::new();
        let mut runs = 0;
        for r in self
            .records
            .iter()
            .filter(|r| r.lang == lang && !r.keys.is_empty())
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
            lang: s.lang.code().to_string(),
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

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_records_without_keys_still_load() {
        let old = r#"{"timestamp":1,"mode":"time:30","lang":"en","wpm":90.0,"raw":95.0,"accuracy":97.0,"consistency":80.0}"#;
        let r: Record = serde_json::from_str(old).unwrap();
        assert!(r.keys.is_empty());
    }

    #[test]
    fn keys_round_trip_including_cyrillic() {
        let mut keys = BTreeMap::new();
        keys.insert('ө', [12, 3]);
        keys.insert('e', [40, 1]);
        let r = Record {
            timestamp: 1,
            mode: "words:25".into(),
            lang: "mn".into(),
            wpm: 1.0,
            raw: 1.0,
            accuracy: 1.0,
            consistency: 1.0,
            keys,
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: Record = serde_json::from_str(&json).unwrap();
        assert_eq!(back.keys[&'ө'], [12, 3]);

        let h = History {
            path: None,
            records: vec![back.clone(), back],
        };
        let (totals, runs) = h.key_totals("mn");
        assert_eq!(runs, 2);
        assert_eq!(
            totals[&'ө'],
            KeyStat {
                attempts: 24,
                misses: 6
            }
        );
        assert_eq!(h.key_totals("en").1, 0);
    }
}
