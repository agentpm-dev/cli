use std::time::{Duration, Instant};

/// Human-friendly step logger that prints to STDERR.
/// - Auto-suppressed with `quiet` (e.g., --quiet or non-TTY)
/// - Methods borrow `&mut self` so you can call from closures if needed.
pub struct Step {
    label: String,
    started: Instant,
    finished: bool,
    quiet: bool,
}

impl Step {
    pub fn new(label: impl Into<String>, quiet: bool) -> Self {
        let label = label.into();
        if !quiet {
            eprintln!("• {}…", label);
        }
        Self {
            label,
            started: Instant::now(),
            finished: false,
            quiet,
        }
    }

    pub fn ok(&mut self, details: impl Into<String>) {
        if self.finished || self.quiet {
            return;
        }
        let d = details.into();
        let dur = self.started.elapsed();
        if d.is_empty() {
            eprintln!("✓ {} ({})", self.label, fmt_dur(dur));
        } else {
            eprintln!("✓ {} ({}) — {}", self.label, fmt_dur(dur), d);
        }
        self.finished = true;
    }

    pub fn err(&mut self, details: impl Into<String>) {
        if self.finished || self.quiet {
            return;
        }
        let dur = self.started.elapsed();
        eprintln!("✗ {} ({}) — {}", self.label, fmt_dur(dur), details.into());
        self.finished = true;
    }

    pub fn final_msg(msg: impl AsRef<str>, quiet: bool) {
        if quiet {
            return;
        }
        eprintln!("{}", msg.as_ref());
    }
}

fn fmt_dur(d: Duration) -> String {
    if d.as_secs() >= 1 {
        format!("{:.1}s", d.as_secs_f32())
    } else {
        format!("{}ms", d.as_millis())
    }
}
