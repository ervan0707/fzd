//! Check GitHub Releases for a newer version, and optionally self-update.
//!
//! `check()` queries the repository's `releases/latest` endpoint and compares
//! the returned tag against the version compiled into this binary. `update()`
//! goes further: it downloads the release asset for the current platform,
//! extracts the binary, and swaps it in over the running executable.
//!
//! Self-update only makes sense for a raw-binary install (the `install.sh`
//! path, or a manual download). When the binary lives under a package
//! manager's tree we refuse and point at that manager instead, unless the
//! caller forces it.

use std::io::Read;
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

const CURRENT: &str = env!("CARGO_PKG_VERSION");
const USER_AGENT: &str = concat!("fzd/", env!("CARGO_PKG_VERSION"));

#[derive(Deserialize)]
struct Release {
    tag_name: String,
}

pub struct UpdateCheck {
    pub current: String,
    /// Latest version with any leading `v` stripped, for display/compare.
    pub latest: String,
    /// The raw release tag (e.g. `v0.2.0`), used to build the download URL.
    pub tag: String,
    pub newer: bool,
}

/// `owner/repo` pulled from the crate's repository URL, e.g.
/// `https://github.com/ervan0707/fzd` -> `ervan0707/fzd`.
fn repo_slug() -> &'static str {
    env!("CARGO_PKG_REPOSITORY")
        .trim_end_matches('/')
        .rsplit_once("github.com/")
        .map(|(_, slug)| slug)
        .unwrap_or("ervan0707/fzd")
}

fn agent(read_timeout: Duration) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(read_timeout)
        .build()
}

/// Fetch the latest release tag from GitHub and compare it to this build.
pub fn check() -> Result<UpdateCheck> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo_slug());

    let resp = agent(Duration::from_secs(10))
        .get(&url)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .call();

    let body = match resp {
        Ok(r) => r.into_string().context("reading GitHub response body")?,
        // `releases/latest` 404s when the repo has no published release yet.
        Err(ureq::Error::Status(404, _)) => {
            return Err(anyhow!("no releases published for {} yet", repo_slug()))
        }
        Err(e) => return Err(e).context("requesting latest release from GitHub"),
    };

    let release: Release =
        serde_json::from_str(&body).context("parsing GitHub release JSON")?;
    let tag = release.tag_name;
    let latest = tag.trim_start_matches('v').to_string();

    let newer = is_newer(&latest, CURRENT)
        .ok_or_else(|| anyhow!("cannot compare versions {CURRENT} and {latest}"))?;

    Ok(UpdateCheck {
        current: CURRENT.to_string(),
        latest,
        tag,
        newer,
    })
}

/// Run the check and print a one-line result to stdout.
pub fn check_and_report() -> Result<()> {
    let u = check()?;
    if u.newer {
        println!("fzd {} is available (you have {}).", u.latest, u.current);
        println!("Run `fzd --update`, or update via your installer.");
    } else {
        println!("fzd {} is the latest release.", u.current);
    }
    Ok(())
}

/// Download the latest release and replace the running binary in place.
pub fn update(force: bool) -> Result<()> {
    let c = check()?;
    if !c.newer {
        println!("fzd {} is already the latest release.", c.current);
        return Ok(());
    }

    let exe = std::env::current_exe().context("locating the running executable")?;
    if let Some(hint) = managed_install_hint(&exe) {
        if !force {
            return Err(anyhow!(
                "fzd looks installed via {hint}.\n\
                 Update it there, or re-run with --force to self-replace {} anyway.",
                exe.display()
            ));
        }
    }

    let (asset, bin_in_archive) = asset_for(std::env::consts::OS, std::env::consts::ARCH)
        .ok_or_else(|| {
            anyhow!(
                "no prebuilt binary published for {}-{}",
                std::env::consts::OS,
                std::env::consts::ARCH
            )
        })?;

    let url = format!(
        "https://github.com/{}/releases/download/{}/{}",
        repo_slug(),
        c.tag,
        asset
    );
    println!("Downloading fzd {} ({asset})...", c.latest);
    let bytes = download(&url)?;

    // Extract next to the target so the final swap stays on one filesystem.
    let scratch = exe
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!(".fzd-update-{}", c.latest));
    extract(&bytes, &bin_in_archive, &scratch)
        .with_context(|| format!("extracting {bin_in_archive} from {asset}"))?;

    let swap = self_replace::self_replace(&scratch)
        .with_context(|| format!("replacing {}", exe.display()));
    let _ = std::fs::remove_file(&scratch);
    swap?;

    println!("Updated fzd {} -> {}.", c.current, c.latest);
    Ok(())
}

fn download(url: &str) -> Result<Vec<u8>> {
    let resp = agent(Duration::from_secs(120))
        .get(url)
        .set("User-Agent", USER_AGENT)
        .call()
        .context("downloading release asset")?;
    let mut buf = Vec::new();
    resp.into_reader()
        .read_to_end(&mut buf)
        .context("reading release asset body")?;
    Ok(buf)
}

/// If the executable lives under a known package-manager tree, return a short
/// hint naming that manager. Self-update should defer to it by default.
fn managed_install_hint(exe: &Path) -> Option<&'static str> {
    let p = exe.to_string_lossy();
    if p.contains("/nix/store/") {
        Some("Nix (`nix profile upgrade fzd`)")
    } else if p.contains("/.cargo/") {
        Some("Cargo (`cargo install fzd --force`)")
    } else if p.contains("node_modules") {
        Some("npm (`npm update -g fzd`)")
    } else if p.contains("site-packages") || p.contains("/pipx/") {
        Some("pip (`pip install --upgrade fzd`)")
    } else if p.contains("/Cellar/") || p.contains("/homebrew/") {
        Some("Homebrew (`brew upgrade fzd`)")
    } else {
        None
    }
}

/// Map a Rust `(os, arch)` pair to the release asset name and the binary file
/// packed inside it. `None` for platforms we don't publish.
fn asset_for(os: &str, arch: &str) -> Option<(String, String)> {
    let rel_os = match os {
        "linux" => "linux",
        "macos" => "darwin",
        "windows" => "windows",
        _ => return None,
    };
    let rel_arch = match arch {
        "x86_64" => "x86_64",
        "aarch64" => "arm64",
        _ => return None,
    };
    let base = format!("fzd-{rel_os}-{rel_arch}");
    if os == "windows" {
        Some((format!("{base}.zip"), format!("{base}.exe")))
    } else {
        Some((format!("{base}.tar.gz"), base))
    }
}

#[cfg(unix)]
fn extract(archive: &[u8], bin_in_archive: &str, dest: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    use flate2::read::GzDecoder;
    use tar::Archive;

    let mut ar = Archive::new(GzDecoder::new(archive));
    for entry in ar.entries().context("reading tar entries")? {
        let mut entry = entry?;
        let is_match = entry
            .path()?
            .file_name()
            .and_then(|s| s.to_str())
            .map(|n| n == bin_in_archive)
            .unwrap_or(false);
        if is_match {
            let mut out = std::fs::File::create(dest)?;
            std::io::copy(&mut entry, &mut out)?;
            std::fs::set_permissions(dest, std::fs::Permissions::from_mode(0o755))?;
            return Ok(());
        }
    }
    Err(anyhow!("{bin_in_archive} not found in archive"))
}

#[cfg(windows)]
fn extract(archive: &[u8], bin_in_archive: &str, dest: &Path) -> Result<()> {
    use std::io::Cursor;

    let mut zip = zip::ZipArchive::new(Cursor::new(archive)).context("opening zip")?;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i)?;
        let name = f
            .enclosed_name()
            .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()));
        if name.as_deref() == Some(bin_in_archive) {
            let mut out = std::fs::File::create(dest)?;
            std::io::copy(&mut f, &mut out)?;
            return Ok(());
        }
    }
    Err(anyhow!("{bin_in_archive} not found in archive"))
}

/// True if `latest` is a higher semver than `current`. `None` if either string
/// isn't a plain `major.minor.patch`.
fn is_newer(latest: &str, current: &str) -> Option<bool> {
    Some(parse(latest)? > parse(current)?)
}

fn parse(v: &str) -> Option<(u64, u64, u64)> {
    // Drop any pre-release / build suffix (e.g. 1.2.3-rc1) before splitting.
    let core = v.split(['-', '+']).next()?;
    let mut it = core.split('.');
    let major = it.next()?.parse().ok()?;
    let minor = it.next()?.parse().ok()?;
    let patch = it.next()?.parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::{asset_for, is_newer, parse};

    #[test]
    fn compares_versions() {
        assert_eq!(is_newer("0.2.0", "0.1.0"), Some(true));
        assert_eq!(is_newer("0.1.0", "0.1.0"), Some(false));
        assert_eq!(is_newer("0.1.0", "0.2.0"), Some(false));
        assert_eq!(is_newer("1.0.0", "0.9.9"), Some(true));
        assert_eq!(is_newer("0.1.10", "0.1.2"), Some(true));
    }

    #[test]
    fn strips_prerelease_suffix() {
        assert_eq!(parse("1.2.3-rc1"), Some((1, 2, 3)));
        assert_eq!(parse("1.2.3+build.5"), Some((1, 2, 3)));
    }

    #[test]
    fn rejects_malformed() {
        assert_eq!(parse("1.2"), None);
        assert_eq!(parse("1.2.3.4"), None);
        assert_eq!(parse("v1.2.3"), None); // caller strips the leading v
    }

    #[test]
    fn asset_names_match_release_convention() {
        assert_eq!(
            asset_for("linux", "x86_64"),
            Some(("fzd-linux-x86_64.tar.gz".into(), "fzd-linux-x86_64".into()))
        );
        assert_eq!(
            asset_for("macos", "aarch64"),
            Some(("fzd-darwin-arm64.tar.gz".into(), "fzd-darwin-arm64".into()))
        );
        assert_eq!(
            asset_for("windows", "x86_64"),
            Some(("fzd-windows-x86_64.zip".into(), "fzd-windows-x86_64.exe".into()))
        );
        assert_eq!(asset_for("freebsd", "x86_64"), None);
        assert_eq!(asset_for("linux", "riscv64"), None);
    }

    #[cfg(unix)]
    #[test]
    fn extracts_binary_from_tar_gz() {
        use std::io::Read;
        use std::os::unix::fs::PermissionsExt;

        use flate2::{write::GzEncoder, Compression};

        // Build a gzipped tar holding a couple of files, one of them our binary.
        let payload = b"#!/bin/sh\necho hi\n";
        let mut gz = GzEncoder::new(Vec::new(), Compression::default());
        {
            let mut builder = tar::Builder::new(&mut gz);
            for (name, body) in [("README", &b"noise"[..]), ("fzd-linux-x86_64", payload)] {
                let mut header = tar::Header::new_gnu();
                header.set_size(body.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder.append_data(&mut header, name, body).unwrap();
            }
            builder.finish().unwrap();
        }
        let archive = gz.finish().unwrap();

        let dest = std::env::temp_dir().join("fzd-extract-test-bin");
        let _ = std::fs::remove_file(&dest);
        super::extract(&archive, "fzd-linux-x86_64", &dest).unwrap();

        let mut got = Vec::new();
        std::fs::File::open(&dest)
            .unwrap()
            .read_to_end(&mut got)
            .unwrap();
        assert_eq!(got, payload);
        // Extracted binary must be executable.
        let mode = std::fs::metadata(&dest).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111);
        std::fs::remove_file(&dest).unwrap();

        // A missing binary name is a clean error, not a panic.
        assert!(super::extract(&archive, "nope", &dest).is_err());
    }
}
