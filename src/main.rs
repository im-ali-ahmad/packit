mod builder;
mod finder;
mod initializer;
mod installed_manifest;
mod installer;
mod installer_manifest;
mod remover;
mod utils;
use std::{env::args, process::exit};
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
    search [PACKAGE_UID]          Display an installed package's details or list all installed packages

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
            if argc > 3 {
                println!("Too many arguments. Use 'help' for usage.");
                exit(1);
            }
            if argc == 3 {
                if args[2] == "--only-numbers" {
                    println!("{}", VERSION);
                    exit(0);
                } else {
                    println!("Unknown argument '{}'. Use 'help' for usage.", args[2]);
                    exit(1);
                }
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
        "search" => {
            if argc > 2 {
                if argc > 3 {
                    println!("Too many arguments. Use 'help' for usage.");
                    exit(1);
                }
                exit(finder::find(args[2].clone()));
            }
            finder::find_all();
            exit(0);
        }
        cmd => {
            println!("Unknown command '{}'. Use 'help' for usage.", cmd);
            exit(1);
        }
    }
}
