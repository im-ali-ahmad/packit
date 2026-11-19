mod builder;
mod finder;
mod initializer;
mod installer;
mod remover;

mod installer_manifest;
mod utils;

use std::{env::args, process::exit};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_help() {
    println!(
        "\
packit v{VERSION}
A lightweight Linux package manager and builder.

USAGE:
    packit <COMMAND> [ARGUMENT]

COMMANDS:
    init-project-local <PATH>   Initialize a package manifest from a local project folder
    init-tar-local <PATH>       Initialize a package manifest from a local .tar archive
    init-tar-remote <URL>       Initialize a package manifest from a remote .tar URL (HTTP/HTTPS)
    build <MANIFEST_FILE>       Build a distributable package from an initialized manifest file
    install <PACKAGE_FILE>      Install a built package onto the system
    remove <PACKAGE_UID>        Uninstall a package using its unique ID
    search <PACKAGE_UID>        Display metadata and details for an installed package
    list                        List all currently installed packages
    version                     Print the current version of packit
    help                        Show this help message"
    );
}

fn main() {
    let args: Vec<String> = args().collect();
    let argc: usize = args.len();

    if argc == 1 {
        println!("Use 'help' for usage.");
        exit(1);
    }

    match &*args[1] {
        "help" => {
            if argc > 2 {
                println!("Too many arguments. Use 'help' for usage.");
                exit(1);
            }

            print_help();
            exit(0);
        }

        "version" => {
            if argc > 2 {
                println!("Too many arguments. Use 'help' for usage.");
                exit(1);
            }

            println!("packit Version: {}", VERSION);
            exit(0);
        }

        "init-project-local" => {
            if argc != 3 {
                println!("Too many or less arguments. Use 'help' for usage.");
                exit(1);
            }

            exit(initializer::initialize(
                initializer::InitializeType::PROJECTLOCAL,
                args[2].clone(),
            ));
        }

        "init-tar-local" => {
            if argc != 3 {
                println!("Too many or less arguments. Use 'help' for usage.");
                exit(1);
            }

            exit(initializer::initialize(
                initializer::InitializeType::TARLOCAL,
                args[2].clone(),
            ));
        }

        "init-tar-remote" => {
            if argc != 3 {
                println!("Too many or less arguments. Use 'help' for usage.");
                exit(1);
            }

            exit(initializer::initialize(
                initializer::InitializeType::TARREMOTE,
                args[2].clone(),
            ));
        }

        "build" => {
            if argc != 3 {
                println!("Too many or less arguments. Use 'help' for usage.");
                exit(1);
            }

            exit(builder::build(args[2].clone()));
        }

        "install" => {
            if argc != 3 {
                println!("Too many or less arguments. Use 'help' for usage.");
                exit(1);
            }

            exit(installer::install(args[2].clone()));
        }

        "remove" => {
            if argc != 3 {
                println!("Too many or less arguments. Use 'help' for usage.");
                exit(1);
            }

            exit(remover::remove(args[2].clone()));
        }

        "list" => {
            if argc > 2 {
                println!("Too many arguments. Use 'help' for usage.");
                exit(1);
            }

            finder::find_all();
            exit(0);
        }

        "search" => {
            if argc != 3 {
                println!("Too many or less arguments. Use 'help' for usage.");
                exit(1);
            }

            exit(finder::find(args[2].clone()));
        }

        cmd => {
            println!("Unknown command '{}'. Use 'help' for usage.", cmd);
            exit(1);
        }
    }
}
