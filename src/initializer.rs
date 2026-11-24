use crate::utils::{log_error, log_log, log_step, log_success};
use std::{
    borrow::Cow,
    ffi::OsStr,
    fs::File,
    io::{Error, ErrorKind, Write},
    path::{Path, PathBuf},
};
pub enum InitializeType {
    PROJECTLOCAL,
    TARLOCAL,
    TARREMOTE,
}
fn create_file(name: &str, content: &str) -> Result<String, String> {
    let path = Path::new(name);
    let parent = path.parent().unwrap_or(Path::new(""));
    let stem: Cow<'_, str> = path
        .file_stem()
        .ok_or_else(|| "Invalid filename.".to_string())?
        .to_string_lossy();
    let ext: Option<Cow<'_, str>> = path.extension().map(|e: &OsStr| e.to_string_lossy());
    let mut counter = 0;
    loop {
        let filename = if counter == 0 {
            name.to_string()
        } else {
            match &ext {
                Some(ext) => format!("{stem}-{counter}.{ext}"),
                None => format!("{stem}-{counter}"),
            }
        };
        let filepath: PathBuf = parent.join(&filename);
        match File::options().write(true).create_new(true).open(&filepath) {
            Ok(mut file) => {
                file.write_all(content.as_bytes()).map_err(|e: Error| {
                    let _ = std::fs::remove_file(&filepath);
                    e.to_string()
                })?;
                return Ok(filepath.to_string_lossy().to_string());
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                counter += 1;
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}
pub fn initialize(init_type: InitializeType, url: String) -> i32 {
    log_step("Initializing Packit configuration.");
    let (package_type, content) = match init_type {
        InitializeType::PROJECTLOCAL => (
            "PROJECTLOCAL",
            format!(
                "\
type = \"PROJECTLOCAL\"
path = \"{}\"

[files]
base = \"path/to/base/distributable/dir\" # Relative to project path

[[files.extra]]
source = \"path/to/extra/file/or/folder/source\" # Relative to project path
dest = \"path/to/extra/file/or/folder/destination\" # Relative to base dir

[[commands]]
file = \"path/to/executable/app\" # Relative to base dir
name = \"name\" # Simple name, like 'git', 'code', etc...

[project]
uid = \"example.app\"
name = \"Package Name\"
description = \"Description\"
author = [\"Example\"]
version = \"1.0.0\"
license = \"MIT\"
",
                url
            ),
        ),
        InitializeType::TARLOCAL => (
            "TARLOCAL",
            format!(
                "\
type = \"TARLOCAL\"
path = \"{}\"

[[commands]]
file = \"path/to/executable/app\" # Relative to tar internal root
name = \"name\" # Simple name, like 'git', 'code', etc...

[project]
uid = \"example.app\"
name = \"Package Name\"
description = \"Description\"
author = [\"Example\"]
version = \"1.0.0\"
license = \"MIT\"
",
                url
            ),
        ),
        InitializeType::TARREMOTE => (
            "TARREMOTE",
            format!(
                "\
type = \"TARREMOTE\"
path = \"{}\"

[[commands]]
file = \"path/to/executable/app\" # Relative to tar internal root
name = \"name\" # Simple name, like 'git', 'code', etc...

[project]
uid = \"example.app\"
name = \"Package Name\"
description = \"Description\"
author = [\"Example\"]
version = \"1.0.0\"
license = \"MIT\"
",
                url
            ),
        ),
    };
    log_log(format!("Package type: {}", package_type));
    log_step("Creating configuration file.");
    match create_file("packit-config.toml", &content) {
        Ok(path) => {
            log_success(format!("Configuration created: '{}'.", path));
            0
        }
        Err(e) => {
            log_error(format!("Cannot create configuration: {}", e));
            1
        }
    }
}
