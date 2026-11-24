use colored::Colorize;
use sha2::{
    Digest, Sha512, digest::array::Array, digest::consts::B0, digest::consts::B1,
    digest::typenum::UInt, digest::typenum::UTerm,
};
use std::{
    borrow::Cow,
    fs::{File, FileType},
    io::{Error as IoError, Read, Result, Write, stdout},
    path::{Path, PathBuf},
    result::Result as StdResult,
};
use terminal_size::{Height, Width, terminal_size};
use walkdir::{DirEntry, Error, WalkDir};
pub fn hash_directory(root: &Path) -> Result<String> {
    let mut entries: Vec<DirEntry> = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .collect::<StdResult<Vec<DirEntry>, Error>>()
        .map_err(IoError::other)?;
    entries.sort_by(|a: &DirEntry, b: &DirEntry| {
        a.path()
            .strip_prefix(root)
            .unwrap()
            .cmp(b.path().strip_prefix(root).unwrap())
    });
    let mut hasher: Sha512 = Sha512::new();
    for entry in entries {
        let path: &Path = entry.path().strip_prefix(root).unwrap();
        let path: Cow<'_, str> = path.to_string_lossy();
        hasher.update((path.len() as u64).to_le_bytes());
        hasher.update(path.as_bytes());
        let file_type: FileType = entry.file_type();
        if file_type.is_dir() {
            hasher.update(b"D");
        } else if file_type.is_symlink() {
            hasher.update(b"L");
            let target: PathBuf = std::fs::read_link(entry.path())?;
            let target: Cow<'_, str> = target.to_string_lossy();
            hasher.update((target.len() as u64).to_le_bytes());
            hasher.update(target.as_bytes());
        } else if file_type.is_file() {
            hasher.update(b"F");
            let mut file: File = File::open(entry.path())?;
            let mut buffer: [u8; 8192] = [0u8; 8192];
            loop {
                let count: usize = file.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                hasher.update(&buffer[..count]);
            }
        }
    }
    let digest: Array<
        u8,
        UInt<UInt<UInt<UInt<UInt<UInt<UInt<UTerm, B1>, B0>, B0>, B0>, B0>, B0>, B0>,
    > = hasher.finalize();
    Ok(digest.iter().map(|b| format!("{:02x}", b)).collect())
}
pub fn log_success(message: impl std::fmt::Display) {
    println!("{} {}", "[SUCCESS]".green(), message);
}
pub fn log_error(message: impl std::fmt::Display) {
    println!("{} {}", "[ ERROR ]".red(), message);
}
pub fn log_warning(message: impl std::fmt::Display) {
    println!("{} {}", "[WARNING]".yellow(), message);
}
pub fn log_step(message: impl std::fmt::Display) {
    println!("{} {}", "[ STEP  ]".purple(), message);
}
pub fn log_log(message: impl std::fmt::Display) {
    println!("{} {}", "[ INFO  ]".blue(), message);
}
pub fn progress(what: &str, percent: u8) {
    let Some((Width(w), Height(_))) = terminal_size() else {
        return;
    };
    let percent = percent.min(100);
    let label_len = what.chars().count();
    let overhead = label_len + 8;
    let w = w as usize;
    if w <= overhead {
        print!("\r\x1b[2K{} {:>3}%", what, percent);
        let _ = stdout().flush();
        return;
    }
    let bar_width = w - overhead;
    let filled = (bar_width * percent as usize) / 100;
    let empty = bar_width - filled;
    print!(
        "\r\x1b[2K{} [{}{}] {:>3}%",
        what,
        "▮".repeat(filled),
        "▯".repeat(empty),
        percent
    );
    let _ = stdout().flush();
}
