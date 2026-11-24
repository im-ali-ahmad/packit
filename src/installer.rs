use crate::{installed_manifest::Package, installer_manifest::*, utils::*};
use chrono::{Datelike, Local};
use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use sha2::{Digest, Sha512};
use std::{
    collections::HashSet,
    fs::{self, File, Permissions},
    io::{self, Read, Write},
    os::unix::fs::{PermissionsExt, symlink},
    path::{Component, Path, PathBuf},
    time::Duration,
};
use tar::Archive;
use tempfile::Builder;
use walkdir::WalkDir;
const MAX_ARCHIVE_SIZE: u64 = 2 * 1024 * 1024 * 1024;
const MAX_EXTRACTED_SIZE: u64 = 8 * 1024 * 1024 * 1024;
const INSTALL_DIR: &str = ".local/packit.opt";
const BIN_DIR: &str = ".local/bin";
const INSTALL_MANIFEST: &str = "packit-manifest.toml";
const DOWNLOAD_TIMEOUT: u64 = 300;
const PROGRESS_INTERVAL: u64 = 1024 * 1024;
fn valid_component(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}
fn validate_relative_path(value: &str) -> Result<PathBuf, String> {
    if value.is_empty() || value.contains('\\') {
        return Err(format!("Invalid relative path '{}'.", value));
    }
    let path = Path::new(value);
    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            _ => return Err(format!("Unsafe path '{}'.", value)),
        }
    }
    Ok(path.to_path_buf())
}
fn hash_directory(root: &Path) -> Result<String, String> {
    let mut entries: Vec<_> = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Cannot walk installation directory: {}", e))?;
    entries.sort_by(|a, b| {
        a.path()
            .strip_prefix(root)
            .unwrap()
            .cmp(b.path().strip_prefix(root).unwrap())
    });
    let mut hasher = Sha512::new();
    for entry in entries {
        let relative = entry.path().strip_prefix(root).map_err(|e| e.to_string())?;
        if relative == Path::new(INSTALL_MANIFEST) {
            continue;
        }
        let path = relative.to_string_lossy();
        hasher.update((path.len() as u64).to_le_bytes());
        hasher.update(path.as_bytes());
        let file_type = entry.file_type();
        if file_type.is_dir() {
            hasher.update(b"D");
        } else if file_type.is_symlink() {
            return Err(format!(
                "Symbolic links are not supported: '{}'.",
                entry.path().display()
            ));
        } else if file_type.is_file() {
            hasher.update(b"F");
            let mut file = File::open(entry.path())
                .map_err(|e| format!("Cannot open '{}': {}", entry.path().display(), e))?;
            let mut buffer = [0u8; 8192];
            loop {
                let count = file
                    .read(&mut buffer)
                    .map_err(|e| format!("Cannot read '{}': {}", entry.path().display(), e))?;
                if count == 0 {
                    break;
                }
                hasher.update(&buffer[..count]);
            }
        } else {
            return Err(format!(
                "Unsupported file type '{}'.",
                entry.path().display()
            ));
        }
    }
    let digest = hasher.finalize();
    Ok(digest.iter().map(|b| format!("{:02x}", b)).collect())
}
fn validate_archive(archive_path: &Path) -> Result<(), String> {
    let file = File::open(archive_path).map_err(|e| format!("Cannot open package: {}", e))?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|e| format!("Invalid TAR archive: {}", e))?;
    let mut paths = HashSet::new();
    let mut total_size = 0u64;
    let mut count = 0usize;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Invalid archive entry: {}", e))?;
        let path = entry
            .path()
            .map_err(|e| format!("Cannot read archive path: {}", e))?
            .into_owned();
        if path.to_string_lossy().contains('\\') {
            return Err(format!("Unsafe archive path '{}'.", path.display()));
        }
        for component in path.components() {
            match component {
                Component::Normal(_) | Component::CurDir => {}
                _ => return Err(format!("Unsafe archive path '{}'.", path.display())),
            }
        }
        if !paths.insert(path.clone()) {
            return Err(format!("Duplicate archive entry '{}'.", path.display()));
        }
        let kind = entry.header().entry_type();
        if !kind.is_file() && !kind.is_dir() {
            return Err(format!("Unsupported archive entry '{}'.", path.display()));
        }
        if kind.is_file() {
            let size = entry
                .header()
                .size()
                .map_err(|e| format!("Cannot read archive entry size: {}", e))?;
            total_size = total_size
                .checked_add(size)
                .ok_or_else(|| "Archive size overflow.".to_string())?;
            if total_size > MAX_EXTRACTED_SIZE {
                return Err("Package exceeds the maximum extracted size.".into());
            }
        }
        count += 1;
    }
    log_log(format!("Validated {} archive entries.", count));
    log_log(format!("Extracted size: {} bytes.", total_size));
    Ok(())
}
fn extract_tar(archive_path: &Path, output_dir: &Path) -> Result<(), String> {
    validate_archive(archive_path)?;
    let file = File::open(archive_path).map_err(|e| format!("Cannot reopen package: {}", e))?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);
    archive
        .unpack(output_dir)
        .map_err(|e| format!("Cannot extract package: {}", e))?;
    Ok(())
}
fn read_manifest(root: &Path) -> Result<Manifest, String> {
    let path = root.join(INSTALL_MANIFEST);
    let metadata = fs::symlink_metadata(&path).map_err(|_| {
        format!(
            "Required installer manifest '{}' was not found.",
            INSTALL_MANIFEST
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("Invalid installer manifest.".into());
    }
    let content =
        fs::read_to_string(&path).map_err(|e| format!("Cannot read installer manifest: {}", e))?;
    let manifest: Manifest =
        toml::from_str(&content).map_err(|e| format!("Invalid installer manifest: {}", e))?;
    if !valid_component(&manifest.project.uid) {
        return Err("Invalid package UID in installer manifest.".into());
    }
    if manifest.project.name.trim().is_empty() || manifest.project.version.trim().is_empty() {
        return Err("Invalid project metadata in installer manifest.".into());
    }
    Ok(manifest)
}
fn verify_hash(root: &Path, expected: &str) -> Result<(), String> {
    log_step("Verifying package SHA-512.");
    let actual = hash_directory(root)?;
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(format!(
            "Package SHA-512 mismatch.\nExpected: {}\nActual:   {}",
            expected, actual
        ));
    }
    log_success("SHA-512 verification successful.");
    Ok(())
}
fn validate_commands(root: &Path, manifest: &Manifest) -> Result<(), String> {
    let canonical_root = root
        .canonicalize()
        .map_err(|e| format!("Cannot resolve extracted directory: {}", e))?;
    let mut names = HashSet::new();
    for command in &manifest.commands {
        if !valid_component(&command.name) {
            return Err(format!("Invalid command name '{}'.", command.name));
        }
        if !names.insert(command.name.clone()) {
            return Err(format!("Duplicate command name '{}'.", command.name));
        }
        let relative = validate_relative_path(&command.file)?;
        let path = canonical_root.join(relative);
        let canonical_path = path
            .canonicalize()
            .map_err(|e| format!("Cannot resolve command '{}': {}", command.name, e))?;
        if !canonical_path.starts_with(&canonical_root) {
            return Err(format!(
                "Command '{}' escapes the package directory.",
                command.name
            ));
        }
        let metadata = fs::symlink_metadata(&canonical_path)
            .map_err(|e| format!("Cannot inspect command '{}': {}", command.name, e))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(format!("Command '{}' is not a regular file.", command.name));
        }
    }
    Ok(())
}
fn confirm_replacement(name: &str) -> Result<bool, String> {
    let stdin = io::stdin();
    log_warning(format!("Package '{}' is already installed.", name));
    print!("Confirm replacement [yes/no]: ");
    use std::io::Write;
    io::stdout()
        .flush()
        .map_err(|e| format!("Cannot flush terminal output: {}", e))?;
    let mut input = String::new();
    stdin
        .read_line(&mut input)
        .map_err(|e| format!("Cannot read confirmation: {}", e))?;
    if input.trim().eq_ignore_ascii_case("yes") {
        Ok(true)
    } else {
        log_warning("Installation cancelled.");
        Ok(false)
    }
}
fn ensure_real_directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(format!("'{}' is not a regular directory.", path.display()));
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            fs::create_dir_all(path)
                .map_err(|e| format!("Cannot create '{}': {}", path.display(), e))?;
        }
        Err(e) => {
            return Err(format!("Cannot inspect '{}': {}", path.display(), e));
        }
    }
    Ok(())
}
fn prepare_links(
    bin_dir: &Path,
    install_path: &Path,
    manifest: &Manifest,
    old_install: bool,
) -> Result<Vec<(PathBuf, Option<PathBuf>)>, String> {
    let mut links = Vec::new();
    for command in &manifest.commands {
        let link = bin_dir.join(&command.name);
        let previous = match fs::symlink_metadata(&link) {
            Ok(metadata) => {
                if !metadata.file_type().is_symlink() {
                    return Err(format!(
                        "Cannot replace existing non-symlink command '{}'.",
                        link.display()
                    ));
                }
                let old_target = fs::read_link(&link)
                    .map_err(|e| format!("Cannot inspect existing link: {}", e))?;
                if !old_install {
                    return Err(format!("Command link '{}' already exists.", link.display()));
                }
                let resolved = if old_target.is_absolute() {
                    old_target.clone()
                } else {
                    link.parent().unwrap().join(&old_target)
                };
                let old_root = install_path
                    .canonicalize()
                    .map_err(|e| format!("Cannot resolve previous installation: {}", e))?;
                let resolved = resolved
                    .canonicalize()
                    .map_err(|e| format!("Cannot resolve existing command link: {}", e))?;
                if !resolved.starts_with(old_root) {
                    return Err(format!(
                        "Command link '{}' does not belong to this package.",
                        link.display()
                    ));
                }
                Some(old_target)
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => {
                return Err(format!("Cannot inspect command link: {}", e));
            }
        };
        links.push((link, previous));
    }
    Ok(links)
}
fn write_installed_manifest(install_path: &Path, manifest: &Manifest) -> Result<(), String> {
    let now = Local::now();
    let mut package = Package {
        uid: manifest.project.uid.clone(),
        name: manifest.project.name.clone(),
        description: manifest.project.description.clone(),
        authors: manifest.project.author.clone(),
        version: manifest.project.version.clone(),
        license: manifest.project.license.clone(),
        files: Vec::new(),
        install_date: (now.day() as u8, now.month() as u8, now.year() as u16),
    };
    for entry in WalkDir::new(install_path).follow_links(false) {
        let entry = entry.map_err(|e| format!("Cannot walk installation directory: {}", e))?;
        if entry.file_type().is_file() {
            package
                .files
                .push(entry.path().to_string_lossy().to_string());
        }
    }
    let content = toml::to_string_pretty(&package)
        .map_err(|e| format!("Cannot serialize installed manifest: {}", e))?;
    fs::write(install_path.join(INSTALL_MANIFEST), content)
        .map_err(|e| format!("Cannot write installed manifest: {}", e))?;
    Ok(())
}
fn install_package(archive_path: &Path, install_root: &Path, bin_dir: &Path) -> Result<(), String> {
    let archive_size = fs::metadata(archive_path)
        .map_err(|e| format!("Cannot inspect package: {}", e))?
        .len();
    if archive_size > MAX_ARCHIVE_SIZE {
        return Err("Package exceeds the maximum archive size.".into());
    }
    ensure_real_directory(install_root)?;
    ensure_real_directory(bin_dir)?;
    let temp_dir = Builder::new()
        .prefix("packit-install-")
        .tempdir_in(install_root)
        .map_err(|e| format!("Cannot create temporary directory: {}", e))?;
    log_step("Extracting package.");
    extract_tar(archive_path, temp_dir.path())?;
    log_step("Reading installer manifest.");
    let manifest = read_manifest(temp_dir.path())?;
    log_log(format!("Package: {}", manifest.project.name));
    log_log(format!("Version: {}", manifest.project.version));
    log_log(format!("UID: {}", manifest.project.uid));
    verify_hash(temp_dir.path(), &manifest.hash)?;
    validate_commands(temp_dir.path(), &manifest)?;
    let install_path = install_root.join(&manifest.project.uid);
    let old_install = match fs::symlink_metadata(&install_path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(format!(
                    "Existing installation '{}' is not a regular directory.",
                    install_path.display()
                ));
            }
            true
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => false,
        Err(e) => {
            return Err(format!("Cannot inspect installation: {}", e));
        }
    };
    if old_install && !confirm_replacement(&manifest.project.name)? {
        return Err("Installation cancelled.".into());
    }
    let links = prepare_links(bin_dir, &install_path, &manifest, old_install)?;
    let staging_path = temp_dir.keep();
    let backup_dir = if old_install {
        let backup = Builder::new()
            .prefix("packit-backup-")
            .tempdir_in(install_root)
            .map_err(|e| {
                let _ = fs::remove_dir_all(&staging_path);
                format!("Cannot create backup directory: {}", e)
            })?
            .keep();
        if let Err(e) = fs::remove_dir_all(&backup) {
            let _ = fs::remove_dir_all(&staging_path);
            return Err(format!("Cannot prepare backup directory: {}", e));
        }
        if let Err(e) = fs::rename(&install_path, &backup) {
            let _ = fs::remove_dir_all(&backup);
            let _ = fs::remove_dir_all(&staging_path);
            return Err(format!("Cannot back up existing installation: {}", e));
        }
        Some(backup)
    } else {
        None
    };
    let mut created_links = Vec::new();
    let mut removed_links = Vec::new();
    let result = (|| -> Result<(), String> {
        fs::rename(&staging_path, &install_path)
            .map_err(|e| format!("Cannot activate installation: {}", e))?;
        for (link, previous) in &links {
            if previous.is_some() {
                fs::remove_file(link).map_err(|e| format!("Cannot replace command link: {}", e))?;
                removed_links.push((link.clone(), previous.clone()));
            }
            let command = manifest
                .commands
                .iter()
                .find(|command| bin_dir.join(&command.name) == *link)
                .ok_or_else(|| "Cannot resolve command link.".to_string())?;
            let target = install_path.join(validate_relative_path(&command.file)?);
            symlink(&target, link)
                .map_err(|e| format!("Cannot create command link '{}': {}", link.display(), e))?;
            created_links.push(link.clone());
            fs::set_permissions(&target, Permissions::from_mode(0o755)).map_err(|e| {
                format!(
                    "Cannot set executable permissions on '{}': {}",
                    target.display(),
                    e
                )
            })?;
        }
        log_step("Installing manifest.");
        write_installed_manifest(&install_path, &manifest)?;
        Ok(())
    })();
    if let Err(error) = result {
        for link in created_links {
            let _ = fs::remove_file(link);
        }
        let _ = fs::remove_dir_all(&install_path);
        if let Some(backup) = &backup_dir {
            let _ = fs::rename(backup, &install_path);
        }
        for (link, previous) in removed_links {
            if let Some(target) = previous {
                let _ = symlink(target, link);
            }
        }
        let _ = fs::remove_dir_all(&staging_path);
        return Err(format!("Installation failed: {}", error));
    }
    if let Some(backup) = backup_dir {
        if let Err(e) = fs::remove_dir_all(&backup) {
            log_warning(format!(
                "Installation succeeded, but backup '{}' could not be removed: {}",
                backup.display(),
                e
            ));
        }
    }
    log_success("Installation completed successfully.");
    log_log(format!("Installed at: {}", install_path.display()));
    log_log(format!("Commands linked in: {}", bin_dir.display()));
    Ok(())
}
pub fn install(path: String) -> i32 {
    log_step("Starting package installation.");
    let result = (|| -> Result<(), String> {
        let archive_path = Path::new(&path)
            .canonicalize()
            .map_err(|e| format!("Cannot resolve package '{}': {}", path, e))?;
        if !archive_path.is_file() {
            return Err("The provided package path is not a regular file.".into());
        }
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| "Cannot determine HOME directory.".to_string())?;
        let install_root = home.join(INSTALL_DIR);
        let bin_dir = home.join(BIN_DIR);
        install_package(&archive_path, &install_root, &bin_dir)
    })();
    match result {
        Ok(()) => {
            log_success("Package installation completed.");
            0
        }
        Err(error) => {
            log_error(format!("Installation failed: {}", error));
            1
        }
    }
}
fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    format!("{:.2} {}", size, UNITS[unit])
}
fn download_archive(client: &Client, url: &str, destination: &Path) -> Result<u64, String> {
    log_step(format!("Downloading '{}'.", url));
    let mut response = client
        .get(url)
        .send()
        .map_err(|e| format!("Cannot download remote archive: {}", e))?;
    if !response.status().is_success() {
        return Err(format!(
            "Remote server returned HTTP status {}.",
            response.status()
        ));
    }
    let content_length = response.content_length();
    if let Some(length) = content_length {
        if length > MAX_ARCHIVE_SIZE {
            return Err("Remote archive exceeds the maximum allowed size.".into());
        }
        log_log(format!("Remote file size: {}", format_size(length)));
    } else {
        log_warning("Remote file size is unknown.");
    }
    let mut file = File::create(destination)
        .map_err(|e| format!("Cannot create downloaded archive: {}", e))?;
    let mut buffer = [0u8; 64 * 1024];
    let mut downloaded = 0u64;
    let mut last_percent = 0u64;
    let mut last_reported = 0u64;
    loop {
        let count = response
            .read(&mut buffer)
            .map_err(|e| format!("Download interrupted: {}", e))?;
        if count == 0 {
            break;
        }
        downloaded = downloaded
            .checked_add(count as u64)
            .ok_or_else(|| "Download size overflow.".to_string())?;
        if downloaded > MAX_ARCHIVE_SIZE {
            return Err("Downloaded archive exceeds the maximum allowed size.".into());
        }
        file.write_all(&buffer[..count])
            .map_err(|e| format!("Cannot write downloaded archive: {}", e))?;
        match content_length {
            Some(total) if total > 0 => {
                let percent = (downloaded.saturating_mul(100) / total).min(100);
                while last_percent < percent {
                    last_percent += 1;
                    progress(&"Downloading...".to_string(), last_percent as u8);
                }
            }
            _ if downloaded.saturating_sub(last_reported) >= PROGRESS_INTERVAL => {
                log_log(format!("Downloaded: {}", format_size(downloaded)));
                last_reported = downloaded;
            }
            _ => {}
        }
    }
    file.flush()
        .map_err(|e| format!("Cannot flush downloaded archive: {}", e))?;
    if let Some(total) = content_length {
        if downloaded != total {
            return Err(format!(
                "Incomplete download: expected {}, received {}.",
                format_size(total),
                format_size(downloaded)
            ));
        }
        while last_percent < 100 {
            last_percent += 1;
            progress(&"Downloading...".to_string(), last_percent as u8);
        }
        println!();
    }
    log_success(format!("Download completed: {}.", format_size(downloaded)));
    Ok(downloaded)
}
fn install_remote_download(url: String) -> Result<(tempfile::TempDir, PathBuf), String> {
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(DOWNLOAD_TIMEOUT))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| format!("Cannot create HTTP client: {e}"))?;
    log_success("HTTP client ready.");
    log_step("Creating temporary download directory.");
    let temp_dir =
        tempfile::tempdir().map_err(|e| format!("Cannot create temporary directory: {e}"))?;
    let archive_path = temp_dir.path().join("remote.tar.gz");
    download_archive(&client, &url, &archive_path)?;
    Ok((temp_dir, archive_path))
}
pub fn install_remote(url: String) -> i32 {
    match install_remote_download(url) {
        Ok((_temp_dir, archive_path)) => install(archive_path.to_string_lossy().to_string()),
        Err(e) => {
            log_error(e);
            return 1;
        }
    }
}
