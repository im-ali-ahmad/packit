mod builder;
mod finder;
mod initializer;
mod installed_manifest;
mod installer;
mod installer_manifest;
mod remover;
mod utils;
use std::{env, process::exit};
use utils::*;
const VERSION: &str = env!("CARGO_PKG_VERSION");
fn print_help() {
    println!(
        "\
packit v{VERSION}
A lightweight Linux package manager and package builder.

USAGE:
    packit <COMMAND> [ARGUMENTS]

COMMANDS:
    init-project-local <PATH>           Initialize a package manifest from a local project directory
    init-tar-local <PATH>               Initialize a package manifest from a local TAR archive
    init-tar-remote <URL>               Initialize a package manifest from a remote TAR archive (HTTP/HTTPS)
    build <MANIFEST_FILE> [--install]   Build a distributable package from a manifest, optionally install it
    install <PACKAGE_FILE> [--remote]   Install a built package
    remove <PACKAGE_UID>                Uninstall an installed package
    search [PACKAGE_UID]                Display package details or list installed packages
    list                                List all installed packages
    version [--only-numbers]            Display the current version of Packit
    help                                Display this help message

EXAMPLES:
    packit init-project-local ./my-project
    packit init-tar-local ./source.tar.gz
    packit init-tar-remote https://example.com/source.tar.gz
    packit build packit.toml --install
    packit install --remote https://example.com/package.pack
    packit remove com.example.package
    packit search com.example.package
    packit search
    packit list
    packit version
    packit version --only-numbers
    packit help"
    );
}
fn argument_error(message: impl std::fmt::Display) -> ! {
    log_error(message);
    log_log("Use 'packit help' for usage.");
    exit(1);
}
fn main() {
    let args: Vec<String> = env::args().collect();
    let Some(command) = args.get(1).map(String::as_str) else {
        argument_error("No command provided.");
    };
    let arguments = &args[2..];
    match command {
        "help" => {
            if !arguments.is_empty() {
                argument_error("Too many arguments for 'help'.");
            }
            print_help();
        }
        "version" => match arguments {
            [] => log_log(format!("Packit version: {VERSION}")),
            [flag] if flag == "--only-numbers" => println!("{VERSION}"),
            [flag] => argument_error(format!("Unknown version argument '{flag}'.")),
            _ => argument_error("Too many arguments for 'version'."),
        },
        "init-project-local" => {
            let [path] = arguments else {
                argument_error("'init-project-local' requires exactly one path argument.");
            };
            exit(initializer::initialize(
                initializer::InitializeType::PROJECTLOCAL,
                path.clone(),
            ));
        }
        "init-tar-local" => {
            let [path] = arguments else {
                argument_error("'init-tar-local' requires exactly one path argument.");
            };
            exit(initializer::initialize(
                initializer::InitializeType::TARLOCAL,
                path.clone(),
            ));
        }
        "init-tar-remote" => {
            let [url] = arguments else {
                argument_error("'init-tar-remote' requires exactly one URL argument.");
            };
            exit(initializer::initialize(
                initializer::InitializeType::TARREMOTE,
                url.clone(),
            ));
        }
        "build" => {
            match arguments {
                [manifest, flag] => {
                    if flag != "--install" {
                        argument_error("Invalid flag passed.");
                    }
                    let res = builder::build(manifest.clone());
                    if res.0 == 0
                        && let Some(f) = res.1
                    {
                        exit(installer::install(f.to_string_lossy().to_string()));
                    } else {
                        exit(1);
                    }
                }
                [manifest] => {
                    exit(builder::build(manifest.clone()).0);
                }
                _ => {
                    argument_error("'build' uses one manifest file and optional '--install' flag.");
                }
            };
        }
        "install" => match arguments {
            [package] => exit(installer::install(package.clone())),
            [flag, url] if flag == "--remote" => {
                exit(installer::install_remote(url.clone()));
            }
            [url, flag] if flag == "--remote" => {
                exit(installer::install_remote(url.clone()));
            }
            [_, _] => {
                argument_error("Invalid argument format for 'install' command.");
            }
            _ => {
                argument_error("'install' requires a package file or a remote URL.");
            }
        },
        "remove" => {
            let [package_uid] = arguments else {
                argument_error("'remove' requires exactly one package UID argument.");
            };
            exit(remover::remove(package_uid.clone()));
        }
        "search" => match arguments {
            [package_uid] => exit(finder::find(package_uid.clone())),
            [] => finder::find_all(),
            _ => argument_error("Too many arguments for 'search'."),
        },
        "list" => {
            if !arguments.is_empty() {
                argument_error("'list' does not accept arguments.");
            }
            finder::find_all();
        }
        _ => {
            argument_error(format!("Unknown command '{command}'."));
        }
    }
}
