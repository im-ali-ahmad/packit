mod builder;
mod finder;
mod initializer;
mod installed_manifest;
mod installer;
mod installer_manifest;
mod remover;
mod utils;
use std::{env::args, process::exit};
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
    init-project-local <PATH>     Initialize a package manifest from a local project directory
    init-tar-local <PATH>         Initialize a package manifest from a local TAR archive
    init-tar-remote <URL>         Initialize a package manifest from a remote TAR archive (HTTP/HTTPS)

    build <MANIFEST_FILE>         Build a distributable package from a manifest
    install <PACKAGE_FILE>        Install a built package
    remove <PACKAGE_UID>          Uninstall an installed package
    search [PACKAGE_UID]          Display package details or list installed packages
    list                          List all installed packages
    version [--only-numbers]      Display the current version of Packit
    help                          Display this help message

EXAMPLES:
    packit init-project-local ./my-project
    packit init-tar-local ./source.tar.gz
    packit init-tar-remote https://example.com/source.tar.gz

    packit build packit.toml
    packit install package.pack
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
    let args: Vec<String> = args().collect();
    let argc = args.len();
    if argc == 1 {
        argument_error("No command provided.");
    }
    match args[1].as_str() {
        "help" => {
            if argc != 2 {
                argument_error("Too many arguments for 'help'.");
            }
            print_help();
            exit(0);
        }
        "version" => {
            if argc > 3 {
                argument_error("Too many arguments for 'version'.");
            }
            if argc == 3 {
                if args[2] == "--only-numbers" {
                    println!("{}", VERSION);
                    exit(0);
                }
                argument_error(format!("Unknown version argument '{}'.", args[2]));
            }
            log_log(format!("Packit version: {}", VERSION));
            exit(0);
        }
        "init-project-local" => {
            if argc != 3 {
                argument_error("'init-project-local' requires exactly one path argument.");
            }
            exit(initializer::initialize(
                initializer::InitializeType::PROJECTLOCAL,
                args[2].clone(),
            ));
        }
        "init-tar-local" => {
            if argc != 3 {
                argument_error("'init-tar-local' requires exactly one path argument.");
            }
            exit(initializer::initialize(
                initializer::InitializeType::TARLOCAL,
                args[2].clone(),
            ));
        }
        "init-tar-remote" => {
            if argc != 3 {
                argument_error("'init-tar-remote' requires exactly one URL argument.");
            }
            exit(initializer::initialize(
                initializer::InitializeType::TARREMOTE,
                args[2].clone(),
            ));
        }
        "build" => {
            if argc != 3 {
                argument_error("'build' requires exactly one manifest file argument.");
            }
            exit(builder::build(args[2].clone()));
        }
        "install" => {
            if argc != 3 {
                argument_error("'install' requires exactly one package file argument.");
            }
            exit(installer::install(args[2].clone()));
        }
        "remove" => {
            if argc != 3 {
                argument_error("'remove' requires exactly one package UID argument.");
            }
            exit(remover::remove(args[2].clone()));
        }
        "search" => {
            if argc > 3 {
                argument_error("Too many arguments for 'search'.");
            }
            if argc == 3 {
                exit(finder::find(args[2].clone()));
            }
            finder::find_all();
            exit(0);
        }
        "list" => {
            if argc != 2 {
                argument_error("'list' does not accept arguments.");
            }
            finder::find_all();
            exit(0);
        }
        command => {
            argument_error(format!("Unknown command '{}'.", command));
        }
    }
}
