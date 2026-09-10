//! Streaming downloads with byte-accurate progress and atomic file replacement.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};

const COPY_BUFFER_BYTES: usize = 1024 * 1024;
const DEFAULT_ATTEMPTS: u8 = 3;
const DEFAULT_RETRY_DELAY: Duration = Duration::from_secs(1);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(9 * 60);

const CANCELLATION_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// A download request whose result replaces `destination` only after it is complete.
pub struct DownloadRequest<'a> {
    pub url: &'a str,
    pub destination: &'a Path,
    /// A trusted expected size. When present, it drives progress and is validated.
    pub expected_bytes: Option<u64>,
    pub cancellation: &'a DownloadCancellation,
}

/// Cloneable cooperative cancellation handle for one or more related downloads.
#[derive(Clone, Debug, Default)]
pub struct DownloadCancellation {
    cancelled: Arc<AtomicBool>,
}

impl DownloadCancellation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub fn check(&self) -> Result<(), DownloadCancelled> {
        if self.is_cancelled() {
            Err(DownloadCancelled)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DownloadCancelled;

impl std::fmt::Display for DownloadCancelled {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("download cancelled")
    }
}

impl std::error::Error for DownloadCancelled {}

pub fn is_download_cancelled(error: &anyhow::Error) -> bool {
    error.downcast_ref::<DownloadCancelled>().is_some()
}

/// Progress for a single download.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DownloadProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
}

impl DownloadProgress {
    /// Returns a determinate percentage when the total size is known.
    pub fn percent(self) -> Option<u8> {
        let total = self.total_bytes?;
        if total == 0 {
            return Some(100);
        }
        Some(((self.downloaded_bytes.min(total) * 100) / total) as u8)
    }
}

/// Reusable downloader. Clones share reqwest's connection pool and can be sent
/// to separate background tasks for parallel downloads.
#[derive(Clone)]
pub struct DownloadManager {
    client: reqwest::blocking::Client,
    attempts: u8,
    retry_delay: Duration,
}

impl DownloadManager {
    pub fn new() -> anyhow::Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(concat!("padu/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .context("could not initialize the download client")?;
        Ok(Self {
            client,
            attempts: DEFAULT_ATTEMPTS,
            retry_delay: DEFAULT_RETRY_DELAY,
        })
    }

    /// Streams a URL to disk and reports at most one update per percentage
    /// point (plus the initial update), avoiding noisy cross-thread UI events.
    pub fn download(
        &self,
        request: DownloadRequest<'_>,
        mut on_progress: impl FnMut(DownloadProgress),
    ) -> anyhow::Result<()> {
        let temporary = temporary_path(request.destination);
        let mut last_error = None;

        for attempt in 1..=self.attempts {
            request.cancellation.check()?;
            let _ = fs::remove_file(&temporary);
            match self.download_once(&request, &temporary, &mut on_progress) {
                Ok(()) => {
                    request.cancellation.check()?;
                    replace_file(&temporary, request.destination)?;
                    return Ok(());
                }
                Err(error) => {
                    if request.cancellation.is_cancelled() {
                        let _ = fs::remove_file(&temporary);
                        return Err(DownloadCancelled.into());
                    }
                    last_error = Some(error);
                    if attempt < self.attempts {
                        sleep_with_cancellation(request.cancellation, self.retry_delay)?;
                    }
                }
            }
        }

        let _ = fs::remove_file(&temporary);
        Err(last_error.expect("a download manager always performs at least one attempt"))
    }

    fn download_once(
        &self,
        request: &DownloadRequest<'_>,
        temporary: &Path,
        on_progress: &mut impl FnMut(DownloadProgress),
    ) -> anyhow::Result<()> {
        request.cancellation.check()?;
        let mut response = self
            .client
            .get(request.url)
            .send()
            .with_context(|| format!("could not download {}", request.url))?
            .error_for_status()
            .with_context(|| format!("download server rejected {}", request.url))?;

        let response_bytes = response.content_length();
        if let (Some(expected), Some(actual)) = (request.expected_bytes, response_bytes)
            && expected != actual
        {
            bail!("download size mismatch: expected {expected} bytes, server announced {actual}");
        }
        let total_bytes = request.expected_bytes.or(response_bytes);
        let mut last_percent = None;
        report_progress(on_progress, 0, total_bytes, &mut last_percent);

        let mut file = File::create(temporary)
            .with_context(|| format!("could not create download file {}", temporary.display()))?;
        let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
        let mut downloaded_bytes = 0_u64;

        loop {
            request.cancellation.check()?;
            let count = response
                .read(&mut buffer)
                .context("could not read the download response")?;
            if count == 0 {
                break;
            }
            file.write_all(&buffer[..count])
                .context("could not write the downloaded data")?;
            downloaded_bytes += count as u64;
            report_progress(
                on_progress,
                downloaded_bytes,
                total_bytes,
                &mut last_percent,
            );
        }
        file.flush().context("could not flush the download file")?;
        request.cancellation.check()?;

        if let Some(expected) = request.expected_bytes
            && downloaded_bytes != expected
        {
            bail!("download size mismatch: expected {expected} bytes, got {downloaded_bytes}");
        }
        Ok(())
    }
}

fn sleep_with_cancellation(
    cancellation: &DownloadCancellation,
    duration: Duration,
) -> Result<(), DownloadCancelled> {
    let deadline = Instant::now() + duration;
    loop {
        cancellation.check()?;
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(());
        }
        thread::sleep(remaining.min(CANCELLATION_POLL_INTERVAL));
    }
}

fn report_progress(
    on_progress: &mut impl FnMut(DownloadProgress),
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    last_percent: &mut Option<u8>,
) {
    let progress = DownloadProgress {
        downloaded_bytes,
        total_bytes,
    };
    let percent = progress.percent();
    if percent.is_none() || percent != *last_percent {
        *last_percent = percent;
        on_progress(progress);
    }
}

fn temporary_path(destination: &Path) -> PathBuf {
    let mut name = destination.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    destination.with_file_name(name)
}

fn replace_file(source: &Path, destination: &Path) -> anyhow::Result<()> {
    // Unix rename replaces atomically. Windows rename does not replace an
    // existing file, so remove it immediately before the same-directory move.
    #[cfg(windows)]
    if destination.exists() {
        fs::remove_file(destination).with_context(|| {
            format!(
                "could not replace existing download {}",
                destination.display()
            )
        })?;
    }
    fs::rename(source, destination).with_context(|| {
        format!(
            "could not move completed download to {}",
            destination.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_percentage_is_bounded() {
        assert_eq!(
            DownloadProgress {
                downloaded_bytes: 25,
                total_bytes: Some(100),
            }
            .percent(),
            Some(25)
        );
        assert_eq!(
            DownloadProgress {
                downloaded_bytes: 101,
                total_bytes: Some(100),
            }
            .percent(),
            Some(100)
        );
        assert_eq!(
            DownloadProgress {
                downloaded_bytes: 1,
                total_bytes: None,
            }
            .percent(),
            None
        );
    }

    #[test]
    fn cancelled_download_does_not_start_a_request() {
        let cancellation = DownloadCancellation::new();
        cancellation.cancel();
        let manager = DownloadManager::new().unwrap();
        let destination =
            std::env::temp_dir().join(format!("padu-cancel-{}", uuid::Uuid::new_v4()));
        let result = manager.download(
            DownloadRequest {
                url: "https://example.invalid/download",
                destination: &destination,
                expected_bytes: None,
                cancellation: &cancellation,
            },
            |_| {},
        );
        assert!(is_download_cancelled(&result.unwrap_err()));
        assert!(!destination.exists());
    }

    #[test]
    fn temporary_file_stays_next_to_destination() {
        assert_eq!(
            temporary_path(Path::new("/tmp/archive.zip.download")),
            Path::new("/tmp/archive.zip.download.part")
        );
    }
}
