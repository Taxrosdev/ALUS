use console::{Style, style};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::{
    process::{Output, exit},
    time::Duration,
};

const STYLE_LOG: Style = Style::new();
const STYLE_WARN: Style = Style::new().yellow();
const STYLE_DEBUG: Style = Style::new().color256(248).force_styling(true);
const STYLE_FATAL: Style = Style::new().red();

pub fn log(message: impl AsRef<str>) {
    println!("{}", STYLE_LOG.apply_to(message.as_ref()));
}

pub fn debug(message: impl AsRef<str>) {
    println!("{}", STYLE_DEBUG.apply_to(message.as_ref()));
}

pub fn warn(message: impl AsRef<str>) {
    eprintln!("Warning: {}", STYLE_WARN.apply_to(message.as_ref()));
}

pub fn die(message: impl AsRef<str>) -> ! {
    eprintln!("Fatal Error: {}", STYLE_FATAL.apply_to(message.as_ref()));
    exit(1)
}

pub fn hook(output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    for line in stderr.lines() {
        eprintln!("[Hook]: {}", style(line).red());
    }
    for line in stdout.lines() {
        eprintln!("[Hook]: {}", STYLE_DEBUG.apply_to(line));
    }
}

#[derive(Clone)]
pub struct Progress {
    pub multi_progress: MultiProgress,
    resolve: ProgressBar,
}

impl Progress {
    pub fn new() -> Self {
        let mp = MultiProgress::new();
        let interval = Duration::from_millis(150);

        // Resolve
        let resolve = ProgressBar::new(1)
            .with_message("Resolving metadata")
            .with_style(ProgressStyle::with_template("{spinner:.green} {msg}").unwrap());
        resolve.enable_steady_tick(interval);
        mp.add(resolve.clone());

        Self {
            multi_progress: mp,
            resolve,
        }
    }

    pub fn finish_resolving(&self) {
        self.resolve.finish_with_message("Resolved Metadata");
        self.resolve.disable_steady_tick();
    }
}

impl Default for Progress {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Progress {
    fn drop(&mut self) {
        self.resolve.finish();
    }
}
