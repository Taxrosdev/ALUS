use console::{Style, style};
use indicatif::{MultiProgress, ProgressDrawTarget, ProgressStyle};
use std::{
    env::var_os,
    ffi::OsString,
    process::{Output, exit},
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

const STYLE_LOG: Style = Style::new();
const STYLE_WARN: Style = Style::new().yellow();
const STYLE_DEBUG: Style = Style::new().color256(248).force_styling(true);
const STYLE_FATAL: Style = Style::new().red();

pub fn log(message: impl AsRef<str>) {
    eprintln!("{}", STYLE_LOG.apply_to(message.as_ref()));
}

pub fn debug(message: impl AsRef<str>) {
    eprintln!("{}", STYLE_DEBUG.apply_to(message.as_ref()));
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
pub enum Progress {
    Bar {
        mp: MultiProgress,
        resolve_bar: indicatif::ProgressBar,
        download_bar: indicatif::ProgressBar,
    },
    Json {
        download_total: Arc<RwLock<u64>>,
    },
}

#[derive(serde::Serialize, serde::Deserialize)]
struct JsonOutput {
    // 'resolve', 'download', 'finish'
    state: String,
    total: u64,
    downloaded: u64,
}

const PROGRESSBAR_TICK_INTERVAL: Duration = Duration::from_millis(150);

impl Progress {
    pub fn new() -> Progress {
        match var_os("ALUS_STATUS_OUTPUT")
            .unwrap_or(OsString::from("BAR"))
            .into_string()
            .expect("ALUS_STATUS_OUTPUT should be a UTF8 String.")
            .as_str()
        {
            "BAR" => {
                let mp = MultiProgress::new();
                let resolve_bar = indicatif::ProgressBar::new(0)
                    .with_message("Resolving...")
                    .with_style(ProgressStyle::with_template("{spinner:.green} {msg}").unwrap());
                let download_bar = indicatif::ProgressBar::hidden().with_message("Downloading...")
                    .with_style(ProgressStyle::with_template("{spinner:.cyan} {msg} {bytes}/{total_bytes} {bar:30.cyan/blue} [{bytes_per_sec}] {eta}").unwrap());
                resolve_bar.enable_steady_tick(PROGRESSBAR_TICK_INTERVAL);
                download_bar.enable_steady_tick(PROGRESSBAR_TICK_INTERVAL);

                Progress::Bar {
                    mp,
                    download_bar,
                    resolve_bar,
                }
            }
            "JSON" => Progress::Json {
                download_total: Arc::new(RwLock::new(0)),
            },
            _ => die("Invalid ALUS_OUTPUT"),
        }
    }

    pub fn set_resolving(&self, resolved: &AtomicU64, total: &AtomicU64) {
        match self {
            Progress::Json { download_total: _ } => {
                println!(
                    "{}",
                    serde_json::to_string(&JsonOutput {
                        state: String::from("resolve"),
                        total: total.load(Ordering::Acquire),
                        downloaded: resolved.load(Ordering::Acquire)
                    })
                    .unwrap()
                )
            }
            Progress::Bar {
                mp: _,
                resolve_bar,
                download_bar: _,
            } => {
                resolve_bar.set_length(total.load(Ordering::Acquire));
                resolve_bar.set_position(resolved.load(Ordering::Acquire));
            }
        }
    }

    pub fn set_downloading(&self, downloaded: &AtomicU64) {
        match self {
            Progress::Json { download_total } => {
                println!(
                    "{}",
                    serde_json::to_string(&JsonOutput {
                        state: String::from("download"),
                        total: *download_total.read().unwrap(),
                        downloaded: downloaded.load(Ordering::Acquire)
                    })
                    .unwrap()
                )
            }
            Progress::Bar {
                mp: _,
                resolve_bar: _,
                download_bar,
            } => {
                download_bar.set_position(downloaded.load(Ordering::Acquire));
            }
        }
    }

    pub fn finish_resolving(&self, total_download: u64) {
        match self {
            Progress::Json { download_total } => {
                *download_total.write().unwrap() = total_download;
            }
            Progress::Bar {
                mp: _,
                resolve_bar,
                download_bar,
            } => {
                resolve_bar.finish_with_message("Finished resolving metadata");
                download_bar.set_draw_target(ProgressDrawTarget::stderr());
                download_bar.set_length(total_download);
            }
        }
    }

    pub fn finish(&self) {
        match self {
            Progress::Json { download_total: _ } => {
                println!(
                    "{}",
                    serde_json::to_string(&JsonOutput {
                        state: String::from("finish"),
                        total: 0,
                        downloaded: 0
                    })
                    .unwrap()
                )
            }
            Progress::Bar {
                mp: _,
                resolve_bar: _,
                download_bar,
            } => {
                download_bar.finish_with_message("Downloaded");
            }
        }
    }
}

impl Default for Progress {
    fn default() -> Self {
        Self::new()
    }
}
