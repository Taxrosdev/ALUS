use std::{process::exit, time::Duration};

use console::style;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

pub fn log(message: impl AsRef<str>) {
    println!("{}", style(message.as_ref()));
}

pub fn debug(message: impl AsRef<str>) {
    println!(
        "{}",
        style(message.as_ref()).color256(248).force_styling(true)
    );
}

pub fn warn(message: impl AsRef<str>) {
    eprintln!("Warning: {}", style(message.as_ref()).yellow());
}

pub fn die(message: impl AsRef<str>) {
    eprintln!("Fatal Error: {}", style(message.as_ref()).red());
    exit(1)
}

#[derive(Clone)]
pub struct Progress {
    _multi_progress: MultiProgress,
    pub resolve: ProgressBar,
    pub download: ProgressBar,
}

impl Progress {
    pub fn new() -> Self {
        let mp = MultiProgress::new();
        let interval = Duration::from_millis(150);

        // Resolve
        let resolve = ProgressBar::new(1).with_message("Resolving...").with_style(
            ProgressStyle::with_template("{spinner:.green} {msg} {pos}/{len}").unwrap(),
        );
        resolve.enable_steady_tick(interval);
        mp.add(resolve.clone());

        // Download
        let download = ProgressBar::new(1)
                    .with_message("Downloading...")
                    .with_style(
                        ProgressStyle::with_template(
                            "{spinner:.cyan} {msg} {bytes}/{total_bytes} {bar:30.cyan/blue} [{bytes_per_sec}] {eta}",
                        )
                        .expect("Progress bar error")
                        .progress_chars("█▉▊▋▌▍▎▏ "),
                    );
        download.enable_steady_tick(interval);
        mp.add(download.clone());

        Self {
            _multi_progress: mp,
            resolve,
            download,
        }
    }
}

impl Drop for Progress {
    fn drop(&mut self) {
        self.resolve.finish();
        self.download.finish();
    }
}
