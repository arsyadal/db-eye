use std::path::PathBuf;

pub const GITHUB_REPO: &str = "arsyadal/db-eye";
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn current_target() -> Option<&'static str> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return Some("x86_64-unknown-linux-gnu");

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return Some("aarch64-apple-darwin");

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return Some("x86_64-apple-darwin");

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return Some("x86_64-pc-windows-msvc");

    #[allow(unreachable_code)]
    None
}

pub fn is_newer_version(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.trim_start_matches('v')
            .split('.')
            .filter_map(|s| s.parse::<u64>().ok())
            .collect()
    };
    let latest_parts = parse(latest);
    let current_parts = parse(current);
    latest_parts > current_parts
}

pub fn update_cache_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let base = std::env::var("APPDATA").ok().map(PathBuf::from);
    #[cfg(not(target_os = "windows"))]
    let base = std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".config"));

    base.map(|b| b.join("db-eye").join("update_check.json"))
}

#[derive(serde::Serialize, serde::Deserialize, Default, Clone, Debug)]
pub struct UpdateCache {
    pub last_checked_unix_secs: u64,
    pub latest_version: String,
}

pub fn load_update_cache() -> Option<UpdateCache> {
    let path = update_cache_path()?;
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn save_update_cache(cache: &UpdateCache) {
    if let Some(path) = update_cache_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_json::to_string(cache) {
            let _ = std::fs::write(path, content);
        }
    }
}

#[derive(serde::Deserialize)]
struct GitHubRelease {
    tag_name: String,
}

pub fn fetch_latest_release_tag() -> Result<String, String> {
    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    let resp = ureq::get(&url)
        .set("User-Agent", &format!("db-eye/{CURRENT_VERSION}"))
        .timeout(std::time::Duration::from_secs(5))
        .call()
        .map_err(|e| format!("Failed to connect to GitHub Releases: {e}"))?;

    let release: GitHubRelease = resp
        .into_json()
        .map_err(|e| format!("Invalid release response format: {e}"))?;

    Ok(release.tag_name)
}

pub fn spawn_background_update_check(sender: tokio::sync::mpsc::UnboundedSender<String>) {
    let handle = match tokio::runtime::Handle::try_current() {
        Ok(h) => h,
        Err(_) => return,
    };

    handle.spawn_blocking(move || {
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Check if cache is fresh within 24 hours (86,400 seconds)
        if let Some(cache) = load_update_cache()
            && now_secs.saturating_sub(cache.last_checked_unix_secs) < 86_400
        {
            if is_newer_version(&cache.latest_version, CURRENT_VERSION) {
                let _ = sender.send(cache.latest_version);
            }
            return;
        }

        // Cache missing or expired -> query GitHub API
        if let Ok(latest_tag) = fetch_latest_release_tag() {
            let cache = UpdateCache {
                last_checked_unix_secs: now_secs,
                latest_version: latest_tag.clone(),
            };
            save_update_cache(&cache);

            if is_newer_version(&latest_tag, CURRENT_VERSION) {
                let _ = sender.send(latest_tag);
            }
        }
    });
}

pub fn handle_update_command() -> Result<(), Box<dyn std::error::Error>> {
    println!("Checking for updates...");
    let latest_tag = match fetch_latest_release_tag() {
        Ok(tag) => tag,
        Err(e) => {
            eprintln!("{e}");
            eprintln!("You can also update manually via: cargo install --force db-eye");
            return Ok(());
        }
    };

    if !is_newer_version(&latest_tag, CURRENT_VERSION) {
        println!("db-eye is already up to date (v{CURRENT_VERSION}).");
        return Ok(());
    }

    let target = match current_target() {
        Some(t) => t,
        None => {
            println!("Found new version: {latest_tag} (current: v{CURRENT_VERSION})");
            println!("Direct binary replacement is not supported on this platform.");
            println!("Please update using: cargo install --force db-eye");
            return Ok(());
        }
    };

    println!("Found new version: {latest_tag} (current: v{CURRENT_VERSION})");
    println!("Downloading binary for {target}...");

    let bin_name = if cfg!(windows) {
        format!("db-eye-{target}.exe")
    } else {
        format!("db-eye-{target}")
    };

    let download_url =
        format!("https://github.com/{GITHUB_REPO}/releases/download/{latest_tag}/{bin_name}");

    let resp = match ureq::get(&download_url)
        .set("User-Agent", &format!("db-eye/{CURRENT_VERSION}"))
        .timeout(std::time::Duration::from_secs(60))
        .call()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to download update binary from {download_url}: {e}");
            eprintln!("Fallback to update via cargo: cargo install --force db-eye");
            return Ok(());
        }
    };
    let temp_path = std::env::temp_dir().join(format!("db-eye-update-{}.bin", std::process::id()));
    let mut reader = resp.into_reader();
    let mut writer = std::fs::File::create(&temp_path)?;
    std::io::copy(&mut reader, &mut writer)?;
    drop(writer);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        let _ = std::fs::set_permissions(&temp_path, perms);
    }

    println!("Applying update...");
    let replace_res = self_replace::self_replace(&temp_path);
    let _ = std::fs::remove_file(&temp_path);
    replace_res?;
    println!("Successfully updated db-eye to {latest_tag}!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("v0.4.0", "0.3.0"));
        assert!(is_newer_version("0.3.1", "0.3.0"));
        assert!(is_newer_version("v1.0.0", "0.3.0"));
        assert!(!is_newer_version("v0.3.0", "0.3.0"));
        assert!(!is_newer_version("v0.2.0", "0.3.0"));
        assert!(!is_newer_version("v0.2.9", "0.3.0"));
    }

    #[test]
    fn test_update_cache_serialization() {
        let cache = UpdateCache {
            last_checked_unix_secs: 1700000000,
            latest_version: "v0.4.0".to_string(),
        };
        let serialized = serde_json::to_string(&cache).unwrap();
        let loaded: UpdateCache = serde_json::from_str(&serialized).unwrap();
        assert_eq!(loaded.latest_version, "v0.4.0");
        assert_eq!(loaded.last_checked_unix_secs, 1700000000);
    }
}
