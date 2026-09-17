//! Platform build metadata.
//!
//! Every native updater verifies the public release key exported here. On
//! Windows, Explorer, the taskbar, and the Programs list also read the icon
//! and version block out of the PE image itself.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    export_sparkle_public_key();
    embed_whats_new_notes();

    #[cfg(target_os = "windows")]
    {
        // GPUI's Taffy layout and text shaping recurse deeply enough to
        // overflow the 1 MiB the MSVC linker defaults to.
        println!("cargo:rustc-link-arg-bins=/stack:{}", 8 * 1024 * 1024);
        embed_windows_resources();
    }
}

/// Republish `SUPublicEDKey` from the macOS Info.plist as a compile-time
/// constant.
///
/// The Linux and Windows updaters verify the same EdDSA signatures
/// `generate_appcast` writes, against the same key. Reading the plist here
/// rather than repeating the key in Rust means the platforms cannot drift
/// into a feed the app rejects.
fn export_sparkle_public_key() {
    const PLIST: &str = "../../resources/Info.plist";
    const KEY: &str = "<key>SUPublicEDKey</key>";

    println!("cargo:rerun-if-changed={PLIST}");

    let plist = std::fs::read_to_string(PLIST).expect("read the app Info.plist");
    let value = plist
        .split_once(KEY)
        .and_then(|(_, rest)| rest.split_once("<string>"))
        .and_then(|(_, rest)| rest.split_once("</string>"))
        .map(|(value, _)| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| panic!("{PLIST} has no SUPublicEDKey"));

    println!("cargo:rustc-env=PADU_SPARKLE_PUBLIC_ED_KEY={value}");
}

/// Bundle only the current version's release notes for the in-app
/// "what's new" dialog.
///
/// The full `CHANGELOG.md` stays out of the binary (it also carries the
/// `[unreleased]` draft section): this extracts the section whose level-2
/// heading matches `CARGO_PKG_VERSION` — the same rule
/// `scripts/changelog.ts` uses for the Sparkle feed — and writes it to
/// `$OUT_DIR/whats-new.md` for `include_str!`. A missing section bundles an
/// empty file with a build warning instead of failing the build, so a
/// forgotten changelog entry can never break a release build.
fn embed_whats_new_notes() {
    const CHANGELOG: &str = "../../CHANGELOG.md";

    println!("cargo:rerun-if-changed={CHANGELOG}");
    // A version bump edits the manifest, not the changelog.
    println!("cargo:rerun-if-changed=Cargo.toml");

    let manifest_dir = std::path::PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"),
    );
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let out = out_dir.join("whats-new.md");
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();

    let notes = std::fs::read_to_string(manifest_dir.join(CHANGELOG))
        .ok()
        .and_then(|changelog| extract_release_notes(&changelog, &version));
    match notes {
        Some(notes) => std::fs::write(&out, notes).expect("write whats-new.md"),
        None => {
            println!(
                "cargo:warning=CHANGELOG.md has no section for v{version}; bundling empty release notes"
            );
            std::fs::write(&out, "").expect("write empty whats-new.md");
        }
    }
}

/// The version token from a level-2 changelog heading, or `None` if the line
/// isn't one. Handles `## [0.2.0] - 2026-08-08`, `## 0.2.0`, `## v0.2.0`.
/// Mirrors `headingVersion` in `scripts/changelog.ts`.
fn heading_version(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("##")?;
    // Level 2 only — `### …` must not match.
    if rest.starts_with('#') {
        return None;
    }
    let token = rest.split_whitespace().next()?;
    let token = token.strip_prefix('[').unwrap_or(token);
    let token = token.strip_suffix(']').unwrap_or(token);
    let token = token
        .strip_prefix('v')
        .or_else(|| token.strip_prefix('V'))
        .unwrap_or(token);
    if token.is_empty() { None } else { Some(token) }
}

/// The notes body for `version` (without its heading), or `None` when the
/// changelog has no section for it. Mirrors `extractReleaseNotes` in
/// `scripts/changelog.ts`.
fn extract_release_notes(changelog: &str, version: &str) -> Option<String> {
    let lines: Vec<&str> = changelog.lines().collect();
    let mut start = None;
    for (index, line) in lines.iter().enumerate() {
        if heading_version(line) == Some(version) {
            start = Some(index + 1);
            break;
        }
    }
    let start = start?;

    let mut end = lines.len();
    for (index, line) in lines.iter().enumerate().skip(start) {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("##")
            && !rest.starts_with('#')
            && (rest.is_empty() || rest.starts_with(char::is_whitespace))
        {
            end = index;
            break;
        }
    }

    let body = lines[start..end].join("\n").trim().to_string();
    if body.is_empty() { None } else { Some(body) }
}

#[cfg(target_os = "windows")]
fn embed_windows_resources() {
    const ICON: &str = "../../resources/windows/AppIcon.ico";

    println!("cargo:rerun-if-changed={ICON}");

    let icon = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(ICON);
    // The resource compiler reads `.rc` as C source, so a Windows path
    // separator has to survive as a literal backslash.
    let icon = icon.to_string_lossy().replace('\\', "\\\\");

    let package_version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    // VERSIONINFO wants four numeric fields; Padu's version has three.
    let mut fields = package_version
        .split(['.', '-', '+'])
        .map(|field| field.parse::<u16>().unwrap_or(0))
        .chain(std::iter::repeat(0));
    let file_version = format!(
        "{},{},{},{}",
        fields.next().unwrap_or(0),
        fields.next().unwrap_or(0),
        fields.next().unwrap_or(0),
        fields.next().unwrap_or(0),
    );
    let description = std::env::var("CARGO_PKG_DESCRIPTION").unwrap_or_default();

    let resources = format!(
        r#"1 ICON "{icon}"

1 VERSIONINFO
FILEVERSION {file_version}
PRODUCTVERSION {file_version}
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904b0"
        BEGIN
            VALUE "CompanyName", "Padu\0"
            VALUE "FileDescription", "{description}\0"
            VALUE "FileVersion", "{package_version}\0"
            VALUE "InternalName", "padu\0"
            VALUE "OriginalFilename", "padu.exe\0"
            VALUE "ProductName", "Padu\0"
            VALUE "ProductVersion", "{package_version}\0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x0409, 1200
    END
END
"#
    );

    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let script = out_dir.join("padu.rc");
    std::fs::write(&script, resources).expect("write the resource script");

    // GPUI embeds the application manifest through its own resource script,
    // so this one only claims the icon and version block.
    embed_resource::compile(&script, embed_resource::NONE)
        .manifest_optional()
        .expect("compile Windows resources");
}
