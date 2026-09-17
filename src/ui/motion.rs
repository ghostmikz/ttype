//! Eased positions for the caret and line scrolling, so moving between letters,
//! words and lines glides instead of teleporting.

use std::time::{Duration, Instant};

const CARET_X: Duration = Duration::from_millis(90);
/// vertical caret movement and scrolling share one duration, otherwise the
/// caret would bob while the text scrolls under it
const VERTICAL: Duration = Duration::from_millis(140);

struct Tween {
    from: f64,
    to: f64,
    start: Option<Instant>,
    duration: Duration,
}

impl Tween {
    const fn new(duration: Duration) -> Tween {
        Tween {
            from: 0.0,
            to: 0.0,
            start: None,
            duration,
        }
    }

    fn value(&self, now: Instant) -> f64 {
        let Some(start) = self.start else {
            return self.to;
        };
        let t = now.saturating_duration_since(start).as_secs_f64() / self.duration.as_secs_f64();
        if t >= 1.0 {
            return self.to;
        }
        // ease-out cubic: fast start, gentle landing
        let eased = 1.0 - (1.0 - t).powi(3);
        self.from + (self.to - self.from) * eased
    }

    fn retarget(&mut self, to: f64, now: Instant) {
        if to != self.to {
            self.from = self.value(now);
            self.to = to;
            self.start = Some(now);
        }
    }

    fn snap(&mut self, to: f64) {
        self.from = to;
        self.to = to;
        self.start = None;
    }

    fn active(&self, now: Instant) -> bool {
        self.start
            .is_some_and(|s| now.saturating_duration_since(s) < self.duration)
    }
}

/// where things are drawn this frame, in character/line units
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub caret_col: f64,
    pub caret_line: f64,
    pub scroll: f64,
}

pub struct Motion {
    caret_col: Tween,
    caret_line: Tween,
    scroll: Tween,
    primed: bool,
}

impl Default for Motion {
    fn default() -> Motion {
        Motion {
            caret_col: Tween::new(CARET_X),
            caret_line: Tween::new(VERTICAL),
            scroll: Tween::new(VERTICAL),
            primed: false,
        }
    }
}

impl Motion {
    /// move towards the new targets and return this frame's positions.
    /// the first call after a reset jumps straight there.
    pub fn update(&mut self, target: Frame, now: Instant) -> Frame {
        if self.primed {
            self.caret_col.retarget(target.caret_col, now);
            self.caret_line.retarget(target.caret_line, now);
            self.scroll.retarget(target.scroll, now);
        } else {
            self.caret_col.snap(target.caret_col);
            self.caret_line.snap(target.caret_line);
            self.scroll.snap(target.scroll);
            self.primed = true;
        }
        Frame {
            caret_col: self.caret_col.value(now),
            caret_line: self.caret_line.value(now),
            scroll: self.scroll.value(now),
        }
    }

    /// a new test: don't animate from wherever the old caret was
    pub fn reset(&mut self) {
        self.primed = false;
    }

    /// while true the app redraws at a high frame rate
    pub fn animating(&self, now: Instant) -> bool {
        self.caret_col.active(now) || self.caret_line.active(now) || self.scroll.active(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(col: f64, line: f64) -> Frame {
        Frame {
            caret_col: col,
            caret_line: line,
            scroll: 0.0,
        }
    }

    #[test]
    fn first_update_snaps_then_glides() {
        let mut m = Motion::default();
        let t0 = Instant::now();
        assert_eq!(m.update(at(10.0, 0.0), t0), at(10.0, 0.0));
        assert!(!m.animating(t0));

        let mid = m.update(at(20.0, 0.0), t0);
        assert_eq!(mid.caret_col, 10.0, "starts where it was");
        assert!(m.animating(t0));

        let part = m.update(at(20.0, 0.0), t0 + CARET_X / 2);
        assert!(
            part.caret_col > 15.0 && part.caret_col < 20.0,
            "eases out: {part:?}"
        );

        let done = m.update(at(20.0, 0.0), t0 + CARET_X);
        assert_eq!(done.caret_col, 20.0);
        assert!(!m.animating(t0 + CARET_X));
    }

    #[test]
    fn retargeting_mid_flight_continues_from_current_spot() {
        let mut m = Motion::default();
        let t0 = Instant::now();
        m.update(at(0.0, 0.0), t0);
        m.update(at(10.0, 0.0), t0);
        let halfway = m.update(at(10.0, 0.0), t0 + CARET_X / 2).caret_col;
        let turned = m.update(at(0.0, 0.0), t0 + CARET_X / 2).caret_col;
        assert!((turned - halfway).abs() < 1e-9, "no jump when reversing");
    }

    #[test]
    fn reset_snaps_on_next_update() {
        let mut m = Motion::default();
        let t0 = Instant::now();
        m.update(at(30.0, 2.0), t0);
        m.reset();
        assert_eq!(m.update(at(0.0, 0.0), t0), at(0.0, 0.0));
    }
}
