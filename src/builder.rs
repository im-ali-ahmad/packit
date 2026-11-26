use crate::{installer_manifest, utils::*};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use rayon::prelude::*;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    time::{Duration, Instant},
};
use tar::{Archive, Builder};
use tempfile::NamedTempFile;
use walkdir::WalkDir;
const MAX_ARCHIVE_SIZE: u64 = 2 * 1024 * 1024 * 1024;
const MAX_EXTRACTED_SIZE: u64 = 8 * 1024 * 1024 * 1024;
const DOWNLOAD_TIMEOUT: u64 = 300;
const PROGRESS_INTERVAL: u64 = 1024 * 1024;
#[derive(Debug, Deserialize, Clone)]
struct Manifest {
    r#type: String,
    path: String,
    prebuild: Option<String>,
    files: Option<Files>,
    commands: Vec<Command>,
    project: Project,
}
#[derive(Debug, Deserialize, Clone)]
struct Files {
    base: String,
    extra: Vec<ExtraFile>,
}
#[derive(Debug, Deserialize, Clone)]
struct ExtraFile {
    source: String,
    dest: String,
}
#[derive(Debug, Deserialize, Clone)]
struct Command {
    file: String,
    name: String,
}
#[derive(Debug, Deserialize, Clone)]
struct Project {
    uid: String,
    name: String,
    description: String,
    author: Vec<String>,
    version: String,
    license: String,
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
fn format_duration(duration: Duration) -> String {
    if duration.as_secs() >= 60 {
        format!(
            "{}m {:.2}s",
            duration.as_secs() / 60,
            duration.as_secs_f64() % 60.0
        )
    } else {
        format!("{:.2}s", duration.as_secs_f64())
    }
}
fn parse_version(manifest: &mut Manifest, parent: &Path) -> Result<Manifest, String> {
    let version = &manifest.project.version;
    if let Some(raw_cmd) = version
        .strip_prefix("bash(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let parent = parent
            .canonicalize()
            .map_err(|e| format!("Cannot resolve parent directory: {}", e))?
            .to_string_lossy()
            .into_owned();
        let cmd = raw_cmd.replace("$PARENT", &parent);
        for attempt in 1..=2 {
            print!(
                "Confirmation ({}/2): Execute bash command `{}`? [yes]: ",
                attempt, cmd
            );
            io::stdout()
                .flush()
                .map_err(|e| format!("Cannot flush confirmation prompt: {}", e))?;
            let mut input = String::new();
            io::stdin()
                .read_line(&mut input)
                .map_err(|e| format!("Cannot read confirmation: {}", e))?;
            if input.trim().to_lowercase() != "yes" {
                return Err("Command execution aborted by user.".into());
            }
        }
        let output = std::process::Command::new("bash")
            .arg("-c")
            .arg(&cmd)
            .output()
            .map_err(|e| format!("Failed to execute bash: {}", e))?;
        if !output.status.success() {
            return Err(format!(
                "Bash command failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.is_empty() {
            return Err("Version command returned an empty value.".into());
        }
        manifest.project.version = stdout;
    }
    Ok(manifest.clone())
}
fn parse_manifest(path: &str) -> Result<(Manifest, PathBuf), String> {
    log_step(format!("Loading manifest '{}'.", path));
    let content =
        fs::read_to_string(path).map_err(|e| format!("Cannot read manifest '{}': {}", path, e))?;
    let manifest: Manifest =
        toml::from_str(&content).map_err(|e| format!("Invalid manifest: {}", e))?;
    let manifest_path =
        fs::canonicalize(path).map_err(|e| format!("Cannot resolve manifest path: {}", e))?;
    log_success("Manifest loaded successfully.");
    log_log(format!("Package type: {}", manifest.r#type));
    log_log(format!("Project: {}", manifest.project.name));
    log_log(format!("Version: {}", manifest.project.version));
    log_log(format!("UID: {}", manifest.project.uid));
    Ok((manifest, manifest_path))
}
fn validate_relative_path(path: &str) -> Result<PathBuf, String> {
    let path = Path::new(path);
    if path.as_os_str().is_empty() {
        return Err("Path cannot be empty.".into());
    }
    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            _ => {
                return Err(format!("Invalid relative path '{}'.", path.display()));
            }
        }
    }
    Ok(path.to_path_buf())
}
fn resolve_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let relative = validate_relative_path(relative)?;
    let canonical_root = root
        .canonicalize()
        .map_err(|e| format!("Cannot resolve root '{}': {}", root.display(), e))?;
    let path = canonical_root.join(relative);
    let canonical_path = path
        .canonicalize()
        .map_err(|e| format!("Cannot resolve '{}': {}", path.display(), e))?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(format!(
            "Path '{}' escapes its project directory.",
            path.display()
        ));
    }
    Ok(canonical_path)
}
fn cleanup_output_dir(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|e| format!("Cannot inspect output '{}': {}", path.display(), e))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "Refusing to remove unexpected output '{}'.",
            path.display()
        ));
    }
    fs::remove_dir_all(path)
        .map_err(|e| format!("Cannot remove output '{}': {}", path.display(), e))?;
    log_success(format!("Removed temporary output '{}'.", path.display()));
    Ok(())
}
fn copy_dir_content(src: &Path, dest: &Path) -> Result<(), String> {
    log_step(format!(
        "Copying directory '{}' -> '{}'.",
        src.display(),
        dest.display()
    ));
    if !src.is_dir() {
        return Err(format!("Source '{}' is not a directory.", src.display()));
    }
    let src = src
        .canonicalize()
        .map_err(|e| format!("Cannot resolve source directory: {}", e))?;
    let dest_absolute = if dest.is_absolute() {
        dest.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(dest)
    };
    if dest_absolute.starts_with(&src) {
        return Err("Destination cannot be inside the source directory.".into());
    }
    let mut directories = Vec::new();
    let mut files = Vec::new();
    for entry in WalkDir::new(&src).follow_links(false) {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path == src {
            continue;
        }
        let relative = path.strip_prefix(&src).map_err(|e| e.to_string())?;
        let destination = dest_absolute.join(relative);
        let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        if metadata.is_dir() {
            directories.push(destination);
        } else if metadata.is_file() || metadata.file_type().is_symlink() {
            files.push((path.to_path_buf(), destination, metadata.len()));
        } else {
            return Err(format!("Unsupported file type: '{}'.", path.display()));
        }
    }
    fs::create_dir_all(&dest_absolute).map_err(|e| format!("Cannot create destination: {}", e))?;
    for directory in directories {
        fs::create_dir_all(&directory)
            .map_err(|e| format!("Cannot create directory '{}': {}", directory.display(), e))?;
    }
    let total_files = files.len();
    let total_size: u64 = files.iter().map(|(_, _, size)| *size).sum();
    log_step(format!(
        "Copying {} files ({}).",
        total_files,
        format_size(total_size)
    ));
    let copied = std::sync::atomic::AtomicUsize::new(0);
    let results: Vec<Result<(), String>> = files
        .par_iter()
        .map(|(source, destination, _)| {
            fs::copy(source, destination)
                .map(|_| {
                    copied.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                })
                .map_err(|e| {
                    format!(
                        "Cannot copy '{}' to '{}': {}",
                        source.display(),
                        destination.display(),
                        e
                    )
                })
        })
        .collect();
    for result in results {
        result?;
    }
    log_success(format!(
        "Copied {} of {} files ({}).",
        copied.load(std::sync::atomic::Ordering::Relaxed),
        total_files,
        format_size(total_size)
    ));
    Ok(())
}
fn valid_filename_component(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}
fn append_directory(archive: &mut Builder<GzEncoder<File>>, src: &Path) -> Result<(), String> {
    let mut entries = WalkDir::new(src)
        .follow_links(false)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    entries.sort_by(|a, b| a.path().cmp(b.path()));
    let mut files = Vec::new();
    let mut directories = Vec::new();
    let mut total_size = 0u64;
    for entry in entries {
        let path = entry.path();
        if path == src {
            continue;
        }
        let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "Symbolic links are not supported: '{}'.",
                path.display()
            ));
        }
        if metadata.is_dir() {
            directories.push(path.to_path_buf());
        } else if metadata.is_file() {
            total_size = total_size
                .checked_add(metadata.len())
                .ok_or_else(|| "Archive size overflow.".to_string())?;
            files.push((path.to_path_buf(), metadata.len()));
        } else {
            return Err(format!("Unsupported file type: '{}'.", path.display()));
        }
    }
    log_step(format!(
        "Archiving {} files and {} directories ({}).",
        files.len(),
        directories.len(),
        format_size(total_size)
    ));
    for directory in directories {
        let relative = directory.strip_prefix(src).map_err(|e| e.to_string())?;
        archive
            .append_dir(relative, &directory)
            .map_err(|e| format!("Cannot archive directory: {}", e))?;
    }
    let mut processed = 0u64;
    let mut last_percent = 0u64;
    for (path, size) in files {
        let relative = path.strip_prefix(src).map_err(|e| e.to_string())?;
        archive
            .append_path_with_name(&path, relative)
            .map_err(|e| format!("Cannot archive '{}': {}", path.display(), e))?;
        processed = processed.saturating_add(size);
        let percent = if total_size == 0 {
            100
        } else {
            ((processed as f64 / total_size as f64) * 100.0) as u64
        };
        while last_percent < percent {
            last_percent += 1;
            progress(&"Archiving...".to_string(), last_percent as u8);
        }
    }
    if last_percent == 0 {
        log_success("Archive completed.");
    } else {
        println!();
    }
    log_success("Archive entries written successfully.");
    Ok(())
}
fn pack_folder(src: &Path, manifest: &Manifest) -> Result<PathBuf, String> {
    let started = Instant::now();
    log_step("Creating package.");
    let src = src
        .canonicalize()
        .map_err(|e| format!("Cannot resolve source directory: {}", e))?;
    if !src.is_dir() {
        return Err("Source path must be a directory.".into());
    }
    if !valid_filename_component(&manifest.project.uid)
        || !valid_filename_component(&manifest.project.version)
    {
        return Err("Invalid project UID or version.".into());
    }
    let output_dir = src
        .parent()
        .ok_or_else(|| "Cannot resolve package output directory.".to_string())?
        .to_path_buf();
    let manifest_file = src.join("packit-manifest.toml");
    if manifest_file.exists() {
        log_warning("Removing previous installer manifest.");
        fs::remove_file(&manifest_file)
            .map_err(|e| format!("Cannot remove previous manifest: {}", e))?;
    }
    log_step("Validating command files.");
    let mut commands = Vec::with_capacity(manifest.commands.len());
    for command in &manifest.commands {
        log_step(format!(
            "Checking command '{}': {}",
            command.name, command.file
        ));
        let command_path = resolve_path(&src, &command.file)?;
        if !command_path.is_file() {
            return Err(format!(
                "Command file '{}' is not a regular file.",
                command.file
            ));
        }
        commands.push(installer_manifest::Command {
            file: command.file.clone(),
            name: command.name.clone(),
        });
    }
    log_step("Calculating project hash.");
    let hash = hash_directory(&src).map_err(|e| format!("Cannot hash project directory: {}", e))?;
    log_log(format!("Project hash: {}", hash));
    let install_manifest = installer_manifest::Manifest {
        commands,
        project: installer_manifest::Project {
            uid: manifest.project.uid.clone(),
            name: manifest.project.name.clone(),
            description: manifest.project.description.clone(),
            author: manifest.project.author.clone(),
            version: manifest.project.version.clone(),
            license: manifest.project.license.clone(),
        },
        hash,
    };
    log_step("Generating installer manifest.");
    let serialized = toml::to_string(&install_manifest)
        .map_err(|e| format!("Cannot serialize manifest: {}", e))?;
    fs::write(&manifest_file, serialized).map_err(|e| format!("Cannot write manifest: {}", e))?;
    log_success(format!(
        "Installer manifest created: '{}'.",
        manifest_file.display()
    ));
    let output_path = output_dir.join(format!(
        "{}-{}.pack",
        manifest.project.uid, manifest.project.version
    ));
    log_step("Preparing temporary package.");
    let temporary_file = NamedTempFile::new_in(&output_dir)
        .map_err(|e| format!("Cannot create temporary archive: {}", e))?;
    let file = temporary_file
        .reopen()
        .map_err(|e| format!("Cannot open temporary archive: {}", e))?;
    log_step("Compressing and archiving project.");
    let encoder = GzEncoder::new(file, Compression::default());
    let mut archive = Builder::new(encoder);
    append_directory(&mut archive, &src)?;
    log_step("Finalizing archive.");
    let encoder = archive
        .into_inner()
        .map_err(|e| format!("Cannot finalize TAR archive: {}", e))?;
    encoder
        .finish()
        .map_err(|e| format!("Cannot finish compressed archive: {}", e))?;
    log_step("Saving final package.");
    temporary_file
        .persist(&output_path)
        .map_err(|e| format!("Cannot save package: {}", e.error))?;
    let size = fs::metadata(&output_path)
        .map_err(|e| format!("Cannot read package metadata: {}", e))?
        .len();
    log_log(format!("Package: {}", output_path.display()));
    log_log(format!("Package size: {}", format_size(size)));
    log_success(format!(
        "Packaging completed in {}.",
        format_duration(started.elapsed())
    ));
    Ok(output_path)
}
fn prepare_output_dir(path: &Path) -> Result<(), String> {
    log_step(format!("Preparing output directory '{}'.", path.display()));
    if path.exists() {
        let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(format!(
                "Output path '{}' is not a regular directory.",
                path.display()
            ));
        }
        log_warning("Removing previous output directory.");
        fs::remove_dir_all(path)
            .map_err(|e| format!("Cannot remove previous output directory: {}", e))?;
    }
    fs::create_dir_all(path).map_err(|e| format!("Cannot create output directory: {}", e))?;
    log_success("Output directory ready.");
    Ok(())
}
fn build_project_contents(
    manifest: &Manifest,
    project_path: &Path,
    out_path: &Path,
) -> Result<(), String> {
    let started = Instant::now();
    log_step("Preparing project files.");
    let files = manifest
        .files
        .as_ref()
        .ok_or_else(|| "No files specified in the manifest.".to_string())?;
    let project_path = project_path
        .canonicalize()
        .map_err(|e| format!("Cannot resolve project directory: {}", e))?;
    if !project_path.is_dir() {
        return Err("Project path is not a directory.".into());
    }
    log_log(format!("Project base: {}", project_path.display()));
    log_step("Resolving base files.");
    let base_path = resolve_path(&project_path, &files.base)?;
    if !base_path.is_dir() {
        return Err(format!(
            "Base '{}' is not a directory.",
            base_path.display()
        ));
    }
    log_log(format!("Base directory: {}", base_path.display()));
    prepare_output_dir(out_path)?;
    log_step("Copying base files.");
    copy_dir_content(&base_path, out_path)?;
    log_step("Copying extra files.");
    for (index, extra) in files.extra.iter().enumerate() {
        log_log(format!(
            "Extra file {}/{}: {} -> {}",
            index + 1,
            files.extra.len(),
            extra.source,
            extra.dest
        ));
        let source = resolve_path(&project_path, &extra.source)?;
        let relative_dest = validate_relative_path(&extra.dest)?;
        let destination = out_path.join(relative_dest);
        if source.is_dir() {
            if destination.exists() && !destination.is_dir() {
                return Err(format!(
                    "Destination '{}' is not a directory.",
                    destination.display()
                ));
            }
            copy_dir_content(&source, &destination)?;
        } else if source.is_file() {
            let parent = destination
                .parent()
                .ok_or_else(|| "Invalid destination path.".to_string())?;
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            let size = fs::copy(&source, &destination).map_err(|e| {
                format!(
                    "Cannot copy '{}' to '{}': {}",
                    source.display(),
                    destination.display(),
                    e
                )
            })?;
            log_success(format!("Copied {}.", format_size(size)));
        } else {
            return Err(format!("Unsupported extra file '{}'.", source.display()));
        }
    }
    log_success(format!(
        "Project files prepared in {}.",
        format_duration(started.elapsed())
    ));
    Ok(())
}
fn build_project_local(manifest: &Manifest, manifest_file_path: &Path) -> (i32, Option<PathBuf>) {
    let started = Instant::now();
    log_step("Starting local project build.");
    let project_path = match manifest_file_path
        .parent()
        .ok_or_else(|| "Cannot resolve manifest parent directory.".to_string())
        .and_then(|parent| {
            parent
                .join(&manifest.path)
                .canonicalize()
                .map_err(|e| format!("Cannot resolve project path: {}", e))
        }) {
        Ok(path) => path,
        Err(e) => {
            log_error(e);
            return (1, None);
        }
    };
    let out_path = project_path.join("packit.out");
    let result = (|| -> Result<PathBuf, String> {
        if !project_path.is_dir() {
            return Err(format!(
                "Project '{}' is not a directory.",
                project_path.display()
            ));
        }
        log_log(format!("Project directory: {}", project_path.display()));
        let mut local_manifest = manifest.clone();
        let local_manifest = parse_version(&mut local_manifest, &project_path)?;
        build_project_contents(&local_manifest, &project_path, &out_path)?;
        pack_folder(&out_path, &local_manifest)
    })();
    let cleanup_result = cleanup_output_dir(&out_path);
    match (result, cleanup_result) {
        (Ok(path), Ok(())) => {
            log_success("Build completed successfully.");
            log_log(format!("Package created: {}", path.display()));
            log_log(format!(
                "Total build time: {}.",
                format_duration(started.elapsed())
            ));
            (0, path.clone().into())
        }
        (Ok(path), Err(e)) => {
            log_warning("Build completed with cleanup error.");
            log_log(format!("Package created: {}", path.display()));
            log_error(e);
            (1, None)
        }
        (Err(e), cleanup) => {
            log_error(format!("Build failed: {}", e));
            if let Err(cleanup_error) = cleanup {
                log_error(format!("Cleanup failed: {}", cleanup_error));
            }
            log_log(format!(
                "Elapsed time: {}.",
                format_duration(started.elapsed())
            ));
            (1, None)
        }
    }
}
fn validate_archive(archive: &mut Archive<GzDecoder<File>>) -> Result<u64, String> {
    log_step("Validating archive entries.");
    let entries = archive
        .entries()
        .map_err(|e| format!("Cannot read archive entries: {}", e))?;
    let mut total_size = 0u64;
    let mut count = 0usize;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Invalid archive entry: {}", e))?;
        let path = entry
            .path()
            .map_err(|e| format!("Cannot read archive path: {}", e))?;
        if path.to_string_lossy().contains('\\') {
            return Err(format!("Unsafe archive path '{}'.", path.display()));
        }
        for component in path.components() {
            match component {
                Component::Normal(_) | Component::CurDir => {}
                _ => {
                    return Err(format!("Unsafe archive path '{}'.", path.display()));
                }
            }
        }
        let kind = entry.header().entry_type();
        if kind.is_file() {
            let size = entry
                .header()
                .size()
                .map_err(|e| format!("Cannot read entry size: {}", e))?;
            total_size = total_size
                .checked_add(size)
                .ok_or_else(|| "Archive extracted size overflow.".to_string())?;
            if total_size > MAX_EXTRACTED_SIZE {
                return Err("Archive exceeds the maximum extracted size.".into());
            }
        }
        count += 1;
    }
    log_success(format!("Validated {} archive entries.", count));
    log_log(format!(
        "Total extracted size: {}.",
        format_size(total_size)
    ));
    Ok(total_size)
}
fn build_from_archive(
    manifest: &Manifest,
    archive_path: &Path,
    out_path: &Path,
) -> Result<PathBuf, String> {
    log_step("Preparing archive build.");
    if !archive_path.is_file() {
        return Err(format!(
            "Archive '{}' does not exist.",
            archive_path.display()
        ));
    }
    let archive_size = fs::metadata(archive_path).map_err(|e| e.to_string())?.len();
    if archive_size > MAX_ARCHIVE_SIZE {
        return Err("Archive exceeds the maximum allowed size.".into());
    }
    log_log(format!("Archive: {}", archive_path.display()));
    log_log(format!("Archive size: {}", format_size(archive_size)));
    log_step("Checking archive integrity and safety.");
    let archive_file =
        File::open(archive_path).map_err(|e| format!("Cannot open archive: {}", e))?;
    let decoder = GzDecoder::new(archive_file);
    let mut archive = Archive::new(decoder);
    validate_archive(&mut archive)?;
    log_step("Creating temporary extraction directory.");
    let temp_dir =
        tempfile::tempdir().map_err(|e| format!("Cannot create temporary directory: {}", e))?;
    let extracted_path = temp_dir.path().to_path_buf();
    log_log(format!(
        "Extraction directory: {}",
        extracted_path.display()
    ));
    log_step("Extracting archive.");
    let archive_file =
        File::open(archive_path).map_err(|e| format!("Cannot reopen archive: {}", e))?;
    let decoder = GzDecoder::new(archive_file);
    let mut archive = Archive::new(decoder);
    archive
        .unpack(&extracted_path)
        .map_err(|e| format!("Cannot extract archive: {}", e))?;
    log_success("Archive extracted successfully.");
    let mut local_manifest = manifest.clone();
    let local_manifest = parse_version(&mut local_manifest, &extracted_path)?;
    let build_result = (|| -> Result<PathBuf, String> {
        let mut build_manifest = local_manifest.clone();
        build_manifest.files = Some(Files {
            base: ".".to_string(),
            extra: Vec::new(),
        });
        log_step("Preparing extracted project files.");
        build_project_contents(&build_manifest, &extracted_path, out_path)?;
        log_step("Creating package from extracted project.");
        pack_folder(out_path, &build_manifest)
    })();
    let cleanup_result = cleanup_output_dir(out_path);
    let package = match (build_result, cleanup_result) {
        (Ok(package), Ok(())) => package,
        (Ok(_), Err(e)) => return Err(e),
        (Err(e), Ok(())) => return Err(e),
        (Err(e), Err(cleanup_error)) => {
            return Err(format!("{} Cleanup failed: {}", e, cleanup_error));
        }
    };
    log_success("Archive build completed successfully.");
    Ok(package)
}
fn build_tar_local(manifest: &Manifest) -> (i32, Option<PathBuf>) {
    let started = Instant::now();
    log_step("Starting local TAR build.");
    let result = (|| -> Result<PathBuf, String> {
        let archive_path = Path::new(&manifest.path);
        let archive_path = if archive_path.is_absolute() {
            archive_path.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|e| e.to_string())?
                .join(archive_path)
        };
        let archive_path = archive_path
            .canonicalize()
            .map_err(|e| format!("Cannot resolve archive path: {}", e))?;
        let parent = archive_path
            .parent()
            .ok_or_else(|| "Cannot resolve archive parent directory.".to_string())?;
        let out_path = parent.join("packit.out");
        log_log(format!("Archive parent: {}", parent.display()));
        log_log(format!("Output directory: {}", out_path.display()));
        log_log(format!(
            "Final package will be saved in: {}",
            parent.display()
        ));
        build_from_archive(manifest, &archive_path, &out_path)
    })();
    match result {
        Ok(path) => {
            log_success("Local TAR build completed successfully.");
            log_log(format!("Package created: {}", path.display()));
            log_log(format!(
                "Total build time: {}.",
                format_duration(started.elapsed())
            ));
            (0, path.clone().into())
        }
        Err(e) => {
            log_error(format!("Local TAR build failed: {}", e));
            log_log(format!(
                "Elapsed time: {}.",
                format_duration(started.elapsed())
            ));
            (1, None)
        }
    }
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
fn build_tar_remote(manifest: &Manifest) -> (i32, Option<PathBuf>) {
    let started = Instant::now();
    log_step("Starting remote TAR build.");
    let result = (|| -> Result<PathBuf, String> {
        log_step("Configuring HTTP client.");
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(DOWNLOAD_TIMEOUT))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .map_err(|e| format!("Cannot create HTTP client: {}", e))?;
        log_success("HTTP client ready.");
        log_step("Creating temporary download directory.");
        let temp_dir =
            tempfile::tempdir().map_err(|e| format!("Cannot create temporary directory: {}", e))?;
        let archive_path = temp_dir.path().join("remote.tar.gz");
        download_archive(&client, &manifest.path, &archive_path)?;
        let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
        let out_path = cwd.join("packit.out");
        log_log(format!("Output directory: {}", out_path.display()));
        log_log(format!("Final package will be saved in: {}", cwd.display()));
        build_from_archive(manifest, &archive_path, &out_path)
    })();
    match result {
        Ok(path) => {
            log_success("Remote TAR build completed successfully.");
            log_log(format!("Package created: {}", path.display()));
            log_log(format!(
                "Total build time: {}.",
                format_duration(started.elapsed())
            ));
            (0, path.clone().into())
        }
        Err(e) => {
            log_error(format!("Remote TAR build failed: {}", e));
            log_log(format!(
                "Elapsed time: {}.",
                format_duration(started.elapsed())
            ));
            (1, None)
        }
    }
}
fn run_prebuild_command(cmd: &String, manifest_file_path: &Path) -> Result<(), String> {
    let command = cmd.replace(
        "$PARENT",
        &match manifest_file_path.parent() {
            Some(p) => p,
            None => return Err("Cannot determine parent directory.".to_string()),
        }
        .to_string_lossy(),
    );
    for attempt in 1..=2 {
        print!(
            "Confirmation ({}/2): Execute bash command `{}`? [yes]: ",
            attempt, command
        );
        io::stdout()
            .flush()
            .map_err(|e| format!("Cannot flush confirmation prompt: {}", e))?;
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| format!("Cannot read confirmation: {}", e))?;
        if input.trim().to_lowercase() != "yes" {
            return Err("Command execution aborted by user.".into());
        }
    }
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .status()
        .expect("Failed to execute shell command");
    if !status.success() {
        return Err(format!("Command exited with error: {}", status));
    }
    Ok(())
}
pub fn build(path: String) -> (i32, Option<PathBuf>) {
    let started = Instant::now();
    log_step("Packit build started.");
    let result = (|| -> Result<(i32, Option<PathBuf>), String> {
        let (manifest, manifest_file_path) = parse_manifest(&path)?;
        if let Some(prebuild) = &manifest.prebuild {
            match run_prebuild_command(prebuild, &manifest_file_path) {
                Ok(_) => log_success("Ran prebuild command successfully."),
                Err(e) => {
                    return Err(e);
                }
            };
        }
        match manifest.r#type.as_str() {
            "PROJECTLOCAL" => Ok(build_project_local(&manifest, &manifest_file_path)),
            "TARLOCAL" => Ok(build_tar_local(&manifest)),
            "TARREMOTE" => Ok(build_tar_remote(&manifest)),
            _ => Err(format!("Invalid package type '{}'.", manifest.r#type)),
        }
    })();
    let code = match result {
        Ok(code) => code,
        Err(e) => {
            log_error(format!("Build failed: {}", e));
            (1, None)
        }
    };
    if code.0 == 0 {
        log_success(format!(
            "Packit finished successfully in {}.",
            format_duration(started.elapsed())
        ));
    } else {
        log_error(format!(
            "Packit finished with exit code {} in {}.",
            code.0,
            format_duration(started.elapsed())
        ));
    }
    code
}
