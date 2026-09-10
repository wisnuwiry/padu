//! Installation of Google's official Antigravity ACP server distribution.

use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use crate::download_manager::{DownloadCancellation, DownloadManager, DownloadRequest};
use anyhow::{Context, bail};
use sha2::{Digest, Sha256};

const VERSION: &str = "1.1.1";

#[derive(Clone, Copy)]
struct ReleaseAsset {
    url: &'static str,
    archive_sha256: &'static str,
    archive_bytes: u64,
    executable_name: &'static str,
    executable_bytes: u64,
}

pub fn remove() -> anyhow::Result<()> {
    let root = dirs::home_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(".padu")
        .join("providers")
        .join("antigravity")
        .join(VERSION);
    if root.exists() {
        fs::remove_dir_all(&root)
            .context("could not remove the downloaded Antigravity ACP server")?;
    }
    Ok(())
}

pub fn install(
    cancellation: &DownloadCancellation,
    mut progress: impl FnMut(u8, &'static str),
) -> anyhow::Result<PathBuf> {
    let asset = distribution()?;
    progress(0, "Downloading");
    let root = dirs::home_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(".padu")
        .join("providers")
        .join("antigravity")
        .join(VERSION);
    fs::create_dir_all(&root).context("could not create the Antigravity provider directory")?;

    let archive = root.join("agy-acp-server.zip.download");
    let extract = root.join("extract");
    let _ = fs::remove_file(&archive);
    let _ = fs::remove_dir_all(&extract);
    fs::create_dir_all(&extract)?;
    let _cleanup = InstallCleanup {
        archive: archive.clone(),
        extract: extract.clone(),
    };

    DownloadManager::new()?.download(
        DownloadRequest {
            url: asset.url,
            destination: &archive,
            expected_bytes: Some(asset.archive_bytes),
            cancellation,
        },
        |download| {
            if let Some(percent) = download.percent() {
                progress(((u16::from(percent) * 90) / 100) as u8, "Downloading");
            }
        },
    )?;
    progress(90, "Verifying");
    verify_archive(&archive, &asset, cancellation)?;

    progress(92, "Extracting");
    let harness_name = if cfg!(target_os = "windows") {
        "localharness_external.exe"
    } else {
        "localharness_external"
    };
    extract_binaries(
        &archive,
        &extract,
        asset.executable_name,
        harness_name,
        cancellation,
    )?;

    progress(97, "Installing");
    cancellation.check()?;
    let source = find_file(&extract, asset.executable_name)?;
    let executable_size = fs::metadata(&source)
        .context("could not inspect the downloaded Antigravity executable")?
        .len();
    if executable_size != asset.executable_bytes {
        bail!(
            "Antigravity executable size mismatch: expected {} bytes, got {executable_size}",
            asset.executable_bytes
        );
    }
    let destination = root.join(asset.executable_name);
    replace_installed_file(&source, &destination)
        .context("could not install agy_acp_server.par")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&destination)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&destination, permissions)?;
    }

    if let Ok(harness_source) = find_file(&extract, harness_name) {
        let harness_dest = root.join(harness_name);
        cancellation.check()?;
        replace_installed_file(&harness_source, &harness_dest)
            .context("could not install localharness_external")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&harness_dest)?.permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&harness_dest, permissions)?;
        }
    }

    cancellation.check()?;
    progress(100, "Complete");
    Ok(destination)
}

fn distribution() -> anyhow::Result<ReleaseAsset> {
    #[cfg(target_os = "macos")]
    {
        // The official registry currently publishes the Agy ACP macOS ARM64
        // archive. Intel macOS is not listed by the registry.
        #[cfg(target_arch = "aarch64")]
        return Ok(ReleaseAsset {
            url: "https://dl.google.com/agy-extensions/releases/macos/agy-acp-server-agy_acp_server_1.1.1-darwin-arm64.zip",
            archive_sha256: "fdfa915652cdb7ba8085cc8fffed072cbe009251aa2c951aabdda07a8c28a189",
            archive_bytes: 316_014_828,
            executable_name: "agy_acp_server.par",
            executable_bytes: 802_163_856,
        });
        #[cfg(not(target_arch = "aarch64"))]
        bail!("Antigravity ACP is not distributed for Intel macOS");
    }
    #[cfg(target_os = "linux")]
    {
        #[cfg(target_arch = "aarch64")]
        return Ok(ReleaseAsset {
            url: "https://dl.google.com/agy-extensions/releases/linux/agy-acp-server-agy_acp_server_1.1.1-linux-arm64.zip",
            archive_sha256: "ed69e64b308fcb123ab54bf3277bf9cb0d651064f885ea5aab0ff520c7175398",
            archive_bytes: 656_572_786,
            executable_name: "agy_acp_server.par",
            executable_bytes: 1_862_073_131,
        });
        #[cfg(target_arch = "x86_64")]
        return Ok(ReleaseAsset {
            url: "https://dl.google.com/agy-extensions/releases/linux/agy-acp-server-agy_acp_server_1.1.1-linux-x86_64.zip",
            archive_sha256: "38f62d01b32deb0907b3d39a71ec301fd36369f6ffd1cf262d4af385177f79df",
            archive_bytes: 681_969_407,
            executable_name: "agy_acp_server.par",
            executable_bytes: 1_880_360_328,
        });
        #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
        bail!("Antigravity ACP is not distributed for this Linux architecture");
    }
    #[cfg(target_os = "windows")]
    {
        #[cfg(target_arch = "aarch64")]
        return Ok(ReleaseAsset {
            url: "https://dl.google.com/agy-extensions/releases/windows/agy-acp-server-agy_acp_server_1.1.1-windows-arm64.zip",
            archive_sha256: "35f4b1f47ba6a3fea7b0a3e30010df5ea73a64b4f0e7cf991cddc673ddfbcafc",
            archive_bytes: 468_521_191,
            executable_name: "agy_acp_server.exe",
            executable_bytes: 435_075_816,
        });
        #[cfg(target_arch = "x86_64")]
        return Ok(ReleaseAsset {
            url: "https://dl.google.com/agy-extensions/releases/windows/agy-acp-server-agy_acp_server_1.1.1-windows-x86_64.zip",
            archive_sha256: "47cb50eef14f0a4655d78cfcfda869bcea7aaee5f9787e936bc2935ea612c3b8",
            archive_bytes: 468_238_392,
            executable_name: "agy_acp_server.exe",
            executable_bytes: 430_801_616,
        });
        #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
        bail!("Antigravity ACP is not distributed for this Windows architecture");
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    bail!("Antigravity ACP is not distributed for this platform")
}

fn extract_binaries(
    archive_path: &Path,
    destination: &Path,
    executable_name: &str,
    harness_name: &str,
    cancellation: &DownloadCancellation,
) -> anyhow::Result<()> {
    let archive_file =
        File::open(archive_path).context("could not open the Antigravity archive")?;
    let mut archive =
        zip::ZipArchive::new(archive_file).context("could not read the Antigravity ZIP archive")?;
    let mut found_executable = false;
    let mut found_harness = false;

    for index in 0..archive.len() {
        cancellation.check()?;
        let mut entry = archive
            .by_index(index)
            .context("could not read an Antigravity ZIP entry")?;
        let enclosed_path = entry
            .enclosed_name()
            .ok_or_else(|| anyhow::anyhow!("Antigravity ZIP contains an unsafe path"))?;
        let Some(name) = enclosed_path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let is_executable = name == executable_name;
        let is_harness = name == harness_name;
        if (!is_executable && !is_harness) || entry.is_dir() {
            continue;
        }
        if entry.is_symlink() {
            bail!("Antigravity ZIP contains a symbolic link for {name}");
        }
        if (is_executable && found_executable) || (is_harness && found_harness) {
            bail!("Antigravity ZIP contains duplicate entries for {name}");
        }

        let output_path = destination.join(name);
        let mut output = File::create(&output_path)
            .with_context(|| format!("could not create {}", output_path.display()))?;
        let mut buffer = vec![0_u8; 1024 * 1024];
        loop {
            cancellation.check()?;
            let count = entry
                .read(&mut buffer)
                .with_context(|| format!("could not extract {name}"))?;
            if count == 0 {
                break;
            }
            std::io::Write::write_all(&mut output, &buffer[..count])
                .with_context(|| format!("could not write extracted {name}"))?;
        }
        if is_executable {
            found_executable = true;
        } else {
            found_harness = true;
        }
    }

    if !found_executable {
        bail!("the downloaded archive did not contain {executable_name}");
    }
    Ok(())
}

fn verify_archive(
    path: &Path,
    asset: &ReleaseAsset,
    cancellation: &DownloadCancellation,
) -> anyhow::Result<()> {
    let metadata =
        fs::metadata(path).context("could not inspect the downloaded Antigravity archive")?;
    if metadata.len() != asset.archive_bytes {
        bail!(
            "Antigravity archive size mismatch: expected {} bytes, got {}",
            asset.archive_bytes,
            metadata.len()
        );
    }

    let file = File::open(path).context("could not open the downloaded Antigravity archive")?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        cancellation.check()?;
        let read = reader
            .read(&mut buffer)
            .context("could not hash the downloaded Antigravity archive")?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual = format!("{:x}", hasher.finalize());
    if actual != asset.archive_sha256 {
        bail!(
            "Antigravity archive checksum mismatch: expected {}, got {actual}",
            asset.archive_sha256
        );
    }
    Ok(())
}

fn replace_installed_file(source: &Path, destination: &Path) -> anyhow::Result<()> {
    #[cfg(windows)]
    if destination.exists() {
        fs::remove_file(destination)?;
    }
    fs::rename(source, destination)?;
    Ok(())
}

struct InstallCleanup {
    archive: PathBuf,
    extract: PathBuf,
}

impl Drop for InstallCleanup {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.archive);
        let _ = fs::remove_dir_all(&self.extract);
    }
}

fn find_file(root: &Path, name: &str) -> anyhow::Result<PathBuf> {
    let mut pending = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.file_name().and_then(|name| name.to_str()) == Some(name) {
                return Ok(path);
            }
        }
    }
    bail!("the downloaded archive did not contain {name}")
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use super::*;

    #[test]
    fn distribution_asset_is_valid() {
        #[cfg(any(
            all(target_os = "macos", target_arch = "aarch64"),
            all(
                any(target_os = "linux", target_os = "windows"),
                any(target_arch = "aarch64", target_arch = "x86_64")
            )
        ))]
        {
            let asset = distribution().expect("supported platform should have distribution");
            assert!(
                asset
                    .url
                    .starts_with("https://dl.google.com/agy-extensions/releases/")
            );
            assert_eq!(asset.archive_sha256.len(), 64);
            assert!(asset.archive_bytes > 0);
            assert!(asset.executable_bytes > 0);
            #[cfg(target_os = "windows")]
            assert_eq!(asset.executable_name, "agy_acp_server.exe");
            #[cfg(not(target_os = "windows"))]
            assert_eq!(asset.executable_name, "agy_acp_server.par");
        }
    }

    #[test]
    fn verify_archive_checks_length_and_checksum() {
        let temp_dir =
            std::env::temp_dir().join(format!("agy-test-verify-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();
        let file_path = temp_dir.join("test_archive.bin");

        let content = b"antigravity test archive content payload 1234567890";
        fs::write(&file_path, content).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(content);
        let valid_sha256 = format!("{:x}", hasher.finalize());

        let valid_asset = ReleaseAsset {
            url: "https://example.com/archive.zip",
            archive_sha256: Box::leak(valid_sha256.into_boxed_str()),
            archive_bytes: content.len() as u64,
            executable_name: "test_binary",
            executable_bytes: 100,
        };

        let cancellation = DownloadCancellation::new();
        // Verification succeeds with valid size and sha256
        assert!(verify_archive(&file_path, &valid_asset, &cancellation).is_ok());

        // Verification fails when length mismatches
        let wrong_size_asset = ReleaseAsset {
            archive_bytes: (content.len() + 1) as u64,
            ..valid_asset
        };
        let err = verify_archive(&file_path, &wrong_size_asset, &cancellation).unwrap_err();
        assert!(err.to_string().contains("size mismatch"));

        // Verification fails when sha256 mismatches
        let wrong_hash_asset = ReleaseAsset {
            archive_sha256: "0000000000000000000000000000000000000000000000000000000000000000",
            ..valid_asset
        };
        let err = verify_archive(&file_path, &wrong_hash_asset, &cancellation).unwrap_err();
        assert!(err.to_string().contains("checksum mismatch"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn extracts_required_binaries_without_a_system_unzip() {
        let temp_dir =
            std::env::temp_dir().join(format!("agy-test-extract-{}", uuid::Uuid::new_v4()));
        let archive_path = temp_dir.join("archive.zip");
        let extract_dir = temp_dir.join("extract");
        fs::create_dir_all(&extract_dir).unwrap();

        let archive_file = File::create(&archive_path).unwrap();
        let mut archive = zip::ZipWriter::new(archive_file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        archive
            .start_file("nested/agy_acp_server.par", options)
            .unwrap();
        archive.write_all(b"server").unwrap();
        archive
            .start_file("nested/localharness_external", options)
            .unwrap();
        archive.write_all(b"harness").unwrap();
        archive.start_file("nested/unneeded.bin", options).unwrap();
        archive.write_all(b"unneeded").unwrap();
        archive.finish().unwrap();

        extract_binaries(
            &archive_path,
            &extract_dir,
            "agy_acp_server.par",
            "localharness_external",
            &DownloadCancellation::new(),
        )
        .unwrap();

        assert_eq!(
            fs::read(extract_dir.join("agy_acp_server.par")).unwrap(),
            b"server"
        );
        assert_eq!(
            fs::read(extract_dir.join("localharness_external")).unwrap(),
            b"harness"
        );
        assert!(!extract_dir.join("unneeded.bin").exists());
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn find_file_locates_nested_file() {
        let temp_dir = std::env::temp_dir().join(format!("agy-test-find-{}", uuid::Uuid::new_v4()));
        let nested_dir = temp_dir.join("a").join("b").join("c");
        fs::create_dir_all(&nested_dir).unwrap();

        let target_file = nested_dir.join("agy_acp_server.par");
        fs::write(&target_file, b"binary").unwrap();

        let found = find_file(&temp_dir, "agy_acp_server.par").expect("file should be found");
        assert_eq!(found, target_file);

        let missing = find_file(&temp_dir, "nonexistent.exe");
        assert!(missing.is_err());

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
