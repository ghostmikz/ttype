use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crate::words::{self, Rng};

/// extra characters allowed past the end of a word before input is ignored
const MAX_OVERFLOW: usize = 10;
/// time mode generates words in batches and tops up as you near the end
const TIME_BATCH: usize = 120;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Time(u64),
    Words(usize),
}

pub const MODES: [Mode; 8] = [
    Mode::Time(15),
    Mode::Time(30),
    Mode::Time(60),
    Mode::Time(120),
    Mode::Words(10),
    Mode::Words(25),
    Mode::Words(50),
    Mode::Words(100),
];

impl Mode {
    pub fn label(self) -> String {
        match self {
            Mode::Time(s) => format!("{s}s"),
            Mode::Words(n) => format!("{n}w"),
        }
    }

    pub fn key(self) -> String {
        match self {
            Mode::Time(s) => format!("time:{s}"),
            Mode::Words(n) => format!("words:{n}"),
        }
    }
}

/// one per elapsed second, for the results chart
#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub second: u64,
    pub wpm: f64,
    pub raw: f64,
    pub errors: u32,
}

#[derive(Clone, Debug, Default)]
pub struct CharCounts {
    pub correct: u32,
    pub incorrect: u32,
    pub extra: u32,
    pub missed: u32,
}

/// per-key tallies, keyed by the character that should have been typed
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct KeyStat {
    pub attempts: u32,
    pub misses: u32,
}

impl KeyStat {
    pub fn rate(&self) -> f64 {
        if self.attempts == 0 {
            return 0.0;
        }
        self.misses as f64 / self.attempts as f64
    }

    pub fn add(&mut self, other: KeyStat) {
        self.attempts += other.attempts;
        self.misses += other.misses;
    }
}

#[derive(Clone, Debug)]
pub struct Summary {
    pub mode: Mode,
    pub wpm: f64,
    pub raw: f64,
    pub accuracy: f64,
    pub consistency: f64,
    pub seconds: f64,
    pub chars: CharCounts,
    pub samples: Vec<Sample>,
    pub keys: BTreeMap<char, KeyStat>,
}

pub struct Test {
    pub mode: Mode,
    pub words: Vec<Vec<char>>,
    /// what was typed for each word reached so far; the last entry is the current word
    pub typed: Vec<Vec<char>>,
    pub start: Option<Instant>,
    finished: Option<Instant>,
    keystrokes: u32,
    errors: u32,
    sec_chars: u32,
    sec_errors: u32,
    /// wpm/accuracy as of the last completed second; recomputing them every
    /// frame makes the numbers flicker while you type
    shown: (f64, f64),
    samples: Vec<Sample>,
    keys: BTreeMap<char, KeyStat>,
    rng: Rng,
}

impl Test {
    pub fn new(mode: Mode) -> Test {
        let mut rng = Rng::seeded();
        let count = match mode {
            Mode::Time(_) => TIME_BATCH,
            Mode::Words(n) => n,
        };
        Test {
            mode,
            words: words::generate(count, &mut rng),
            typed: vec![Vec::new()],
            start: None,
            finished: None,
            keystrokes: 0,
            errors: 0,
            sec_chars: 0,
            sec_errors: 0,
            shown: (0.0, 100.0),
            samples: Vec::new(),
            keys: BTreeMap::new(),
            rng,
        }
    }

    pub fn current(&self) -> usize {
        self.typed.len() - 1
    }

    pub fn is_finished(&self) -> bool {
        self.finished.is_some()
    }

    pub fn elapsed(&self) -> Duration {
        match self.start {
            None => Duration::ZERO,
            Some(s) => self.finished.unwrap_or_else(Instant::now) - s,
        }
    }

    pub fn remaining_secs(&self) -> Option<u64> {
        match self.mode {
            Mode::Time(limit) => Some(limit.saturating_sub(self.elapsed().as_secs())),
            Mode::Words(_) => None,
        }
    }

    pub fn type_char(&mut self, c: char) {
        if self.is_finished() {
            return;
        }
        if c == ' ' {
            return self.space();
        }
        let i = self.current();
        let word = &self.words[i];
        let typed = &self.typed[i];
        if typed.len() >= word.len() + MAX_OVERFLOW {
            return;
        }
        self.start.get_or_insert_with(Instant::now);

        let pos = typed.len();
        let expected = word.get(pos).copied();
        let wrong = expected != Some(c);
        // extra characters past the word's end aren't a miss on any particular key
        if let Some(e) = expected {
            let stat = self.keys.entry(e).or_default();
            stat.attempts += 1;
            stat.misses += wrong as u32;
        }
        self.record(wrong);
        self.typed[i].push(c);

        if let Mode::Words(_) = self.mode
            && i + 1 == self.words.len()
            && self.typed[i] == self.words[i]
        {
            self.finish();
        }
    }

    fn space(&mut self) {
        let i = self.current();
        if self.typed[i].is_empty() {
            return;
        }
        let wrong = self.typed[i] != self.words[i];
        self.record(wrong);
        if wrong {
            for &c in self.words[i].iter().skip(self.typed[i].len()) {
                let stat = self.keys.entry(c).or_default();
                stat.attempts += 1;
                stat.misses += 1;
            }
        }
        if i + 1 == self.words.len()
            && let Mode::Words(_) = self.mode
        {
            return self.finish();
        }
        self.typed.push(Vec::new());
        if let Mode::Time(_) = self.mode
            && self.words.len() - self.typed.len() < TIME_BATCH / 2
        {
            let more = words::generate(TIME_BATCH, &mut self.rng);
            self.words.extend(more);
        }
    }

    pub fn backspace(&mut self) {
        if self.is_finished() {
            return;
        }
        let i = self.current();
        if self.typed[i].pop().is_none() && self.can_go_back() {
            self.typed.pop();
        }
    }

    /// ctrl+backspace: clear the current word, or step back into a wrong previous one
    pub fn delete_word(&mut self) {
        if self.is_finished() {
            return;
        }
        let i = self.current();
        if self.typed[i].is_empty() && self.can_go_back() {
            self.typed.pop();
        }
        let i = self.current();
        self.typed[i].clear();
    }

    /// only a word that was got wrong can be re-entered
    fn can_go_back(&self) -> bool {
        let i = self.current();
        i > 0 && self.typed[i - 1] != self.words[i - 1]
    }

    fn record(&mut self, wrong: bool) {
        self.keystrokes += 1;
        self.sec_chars += 1;
        if wrong {
            self.errors += 1;
            self.sec_errors += 1;
        }
    }

    /// call often; takes per-second samples and ends time-mode tests
    pub fn tick(&mut self) {
        if self.start.is_none() || self.is_finished() {
            return;
        }
        let mut elapsed = self.elapsed();
        if let Mode::Time(limit) = self.mode {
            // a stalled loop (suspend, sleep) mustn't sample past the limit
            elapsed = elapsed.min(Duration::from_secs(limit));
        }
        while (self.samples.len() as u64) < elapsed.as_secs() {
            self.push_sample();
        }
        if let Mode::Time(limit) = self.mode
            && elapsed.as_secs() >= limit
        {
            self.finish();
        }
    }

    fn push_sample(&mut self) {
        let second = self.samples.len() as u64 + 1;
        let wpm = self.correct_chars() as f64 / 5.0 / (second as f64 / 60.0);
        self.samples.push(Sample {
            second,
            wpm,
            raw: self.sec_chars as f64 / 5.0 * 60.0,
            errors: self.sec_errors,
        });
        self.shown = (wpm, self.live_accuracy());
        self.sec_chars = 0;
        self.sec_errors = 0;
    }

    fn finish(&mut self) {
        let now = Instant::now();
        if let (Some(start), Mode::Time(limit)) = (self.start, self.mode) {
            // clamp so a late tick doesn't stretch a 30s test to 30.04s
            self.finished = Some(now.min(start + Duration::from_secs(limit)));
        } else {
            self.finished = Some(now);
        }
        // a partial trailing second still counts if anything was typed in it
        if (self.samples.len() as f64) < self.elapsed().as_secs_f64()
            && (self.sec_chars > 0 || self.samples.is_empty())
        {
            self.push_sample();
        }
    }

    /// characters of correctly typed words plus the spaces after them,
    /// plus the correct prefix of the word in progress
    pub fn correct_chars(&self) -> usize {
        let cur = self.current();
        let mut n = 0;
        for (i, t) in self.typed.iter().enumerate() {
            let w = &self.words[i];
            if i < cur {
                if t == w {
                    n += w.len() + 1;
                }
            } else if w.starts_with(t) {
                n += t.len();
            }
        }
        n
    }

    /// the numbers shown while typing: wpm and accuracy, one update per second
    pub fn shown_stats(&self) -> (f64, f64) {
        self.shown
    }

    pub fn live_accuracy(&self) -> f64 {
        if self.keystrokes == 0 {
            return 100.0;
        }
        (self.keystrokes - self.errors) as f64 / self.keystrokes as f64 * 100.0
    }

    pub fn summary(&self) -> Summary {
        let seconds = self.elapsed().as_secs_f64().max(0.001);
        let mins = seconds / 60.0;
        let cur = self.current();

        let mut chars = CharCounts::default();
        for (i, t) in self.typed.iter().enumerate() {
            let w = &self.words[i];
            for j in 0..w.len().max(t.len()) {
                match (t.get(j), w.get(j)) {
                    (Some(a), Some(b)) if a == b => chars.correct += 1,
                    (Some(_), Some(_)) => chars.incorrect += 1,
                    (Some(_), None) => chars.extra += 1,
                    (None, Some(_)) if i < cur => chars.missed += 1,
                    _ => {}
                }
            }
        }

        let typed_chars: usize = self.typed.iter().map(Vec::len).sum::<usize>() + cur;
        let raws: Vec<f64> = self.samples.iter().map(|s| s.raw).collect();

        Summary {
            mode: self.mode,
            wpm: self.correct_chars() as f64 / 5.0 / mins,
            raw: typed_chars as f64 / 5.0 / mins,
            accuracy: self.live_accuracy(),
            consistency: consistency(&raws),
            seconds,
            chars,
            samples: self.samples.clone(),
            keys: self.keys.clone(),
        }
    }
}

/// 100 = perfectly even pace; drops with the spread of per-second raw speed
fn consistency(raws: &[f64]) -> f64 {
    if raws.len() < 2 {
        return 100.0;
    }
    let mean = raws.iter().sum::<f64>() / raws.len() as f64;
    if mean <= 0.0 {
        return 0.0;
    }
    let var = raws.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / raws.len() as f64;
    (100.0 * (1.0 - var.sqrt() / mean)).clamp(0.0, 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed(words: &[&str], mode: Mode) -> Test {
        let mut t = Test::new(mode);
        t.words = words.iter().map(|w| w.chars().collect()).collect();
        t
    }

    fn type_str(t: &mut Test, s: &str) {
        for c in s.chars() {
            t.type_char(c);
        }
    }

    #[test]
    fn words_mode_finishes_on_last_correct_char() {
        let mut t = fixed(&["ab", "cd"], Mode::Words(2));
        type_str(&mut t, "ab cd");
        assert!(t.is_finished());
        let s = t.summary();
        assert_eq!(s.chars.correct, 4);
        assert_eq!(s.accuracy, 100.0);
    }

    #[test]
    fn wrong_word_counts_missed_and_blocks_nothing() {
        let mut t = fixed(&["hello", "world"], Mode::Words(2));
        type_str(&mut t, "hel world");
        assert!(t.is_finished());
        let s = t.summary();
        assert_eq!(s.chars.missed, 2);
        assert_eq!(t.correct_chars(), 5);
        let miss = |c| s.keys.get(&c).copied().unwrap_or_default();
        // "hel" then space: the skipped l and o are misses, h e l were clean
        assert_eq!(
            miss('l'),
            KeyStat {
                attempts: 3,
                misses: 1
            }
        );
        assert_eq!(
            miss('o'),
            KeyStat {
                attempts: 2,
                misses: 1
            }
        );
        assert_eq!(miss('h').misses, 0);
    }

    #[test]
    fn backspace_only_reenters_wrong_words() {
        let mut t = fixed(&["ab", "cd", "ef"], Mode::Words(3));
        type_str(&mut t, "ab ");
        t.backspace();
        assert_eq!(t.current(), 1, "correct word must stay locked");

        type_str(&mut t, "cx ");
        t.backspace();
        assert_eq!(t.current(), 1);
        assert_eq!(t.typed[1], vec!['c', 'x']);
    }

    #[test]
    fn delete_word_clears_current() {
        let mut t = fixed(&["abc", "def"], Mode::Words(2));
        type_str(&mut t, "ab");
        t.delete_word();
        assert!(t.typed[0].is_empty());
        assert_eq!(t.current(), 0);
    }

    #[test]
    fn typo_is_charged_to_the_expected_key() {
        let mut t = fixed(&["cat", "dog"], Mode::Words(2));
        type_str(&mut t, "cst");
        assert_eq!(
            t.keys[&'a'],
            KeyStat {
                attempts: 1,
                misses: 1
            }
        );
        assert!(!t.keys.contains_key(&'s'));
        // overflow characters count as extras, not key misses
        type_str(&mut t, "zz");
        assert_eq!(t.keys.values().map(|k| k.attempts).sum::<u32>(), 3);
    }

    #[test]
    fn overflow_is_capped() {
        let mut t = fixed(&["a", "b"], Mode::Words(2));
        type_str(&mut t, &"x".repeat(50));
        assert_eq!(t.typed[0].len(), 1 + MAX_OVERFLOW);
    }

    #[test]
    fn time_mode_has_one_sample_per_second() {
        let mut t = fixed(&["ab", "cd", "ef"], Mode::Time(15));
        type_str(&mut t, "ab c");
        t.start = Some(Instant::now() - Duration::from_secs(20));
        t.tick();
        assert!(t.is_finished());
        let s = t.summary();
        assert_eq!(s.samples.len(), 15);
        assert!((s.seconds - 15.0).abs() < 1e-9);
    }

    #[test]
    fn consistency_bounds() {
        assert_eq!(consistency(&[60.0, 60.0, 60.0]), 100.0);
        assert!(consistency(&[10.0, 120.0, 5.0]) < 50.0);
    }
}
