use crate::commands;
use crate::preview::render::{PreviewMode, PreviewOptions};
use crate::ui;
use clap::{crate_version, App, AppSettings, Arg, SubCommand};
use std::env;

const HELP_TEMPLATE: &str = "\
{bin} {version}
{about}

USAGE:
    {usage}

{all-args}";

const PROJECT_HELP: &str =
    "The Lithos project: either the path to a directory containing 'lithos.yml', 'lithos.json', \
     or legacy 'mantle.yml', or the path to a YAML/JSON configuration file. Directory lookup \
     checks 'lithos.yml', then 'lithos.json', then legacy 'mantle.yml'. Defaults to the current \
     directory.";

fn get_app() -> App<'static, 'static> {
    App::new("Lithos")
        .version(crate_version!())
        .about("Infra-as-code and deployment tool for Roblox (formerly Mantle)")
        .template(HELP_TEMPLATE)
        .setting(AppSettings::ArgRequiredElseHelp)
        .setting(AppSettings::VersionlessSubcommands)
        .subcommand(
            SubCommand::with_name("deploy")
                .about("Updates a Lithos environment with a project's latest configuration.")
                .arg(
                    Arg::with_name("PROJECT")
                        .index(1)
                        .help(PROJECT_HELP)
                        .takes_value(true))
                .arg(
                    Arg::with_name("environment")
                        .long("environment")
                        .short("e")
                        .help("The label of the environment to deploy to. If not specified, attempts to match the current git branch to each environment's `branches` property.")
                        .value_name("ENVIRONMENT")
                        .takes_value(true))
                .arg(
                    Arg::with_name("allow_purchases")
                        .long("allow-purchases")
                        .help("Gives Lithos permission to make purchases with Robux."))
                .arg(
                    Arg::with_name("yes")
                        .long("yes")
                        .short("y")
                        .help("Skip the interactive plan confirmation prompt and apply immediately. In non-interactive (CI/piped) environments deploy already auto-approves; this flag silences the auto-approve notice."))
                .arg(
                    Arg::with_name("no_preview")
                        .long("no-preview")
                        .help("Skip the pre-apply plan preview entirely. Implies --yes."))
                .arg(
                    Arg::with_name("plain_preview")
                        .long("plain-preview")
                        .help("Show a plain-text plan summary instead of the rich color-coded preview. Useful for CI logs."))
        )
         .subcommand(
            SubCommand::with_name("diff")
                .about("Prints the diff between the current state file and project configuration.")
                .arg(
                    Arg::with_name("PROJECT")
                        .index(1)
                        .help(PROJECT_HELP)
                        .takes_value(true))
                .arg(
                    Arg::with_name("environment")
                        .long("environment")
                        .short("e")
                        .help("The label of the environment to deploy to. If not specified, attempts to match the current git branch to each environment's `branches` property.")
                        .value_name("ENVIRONMENT")
                        .takes_value(true))
                .arg(
                    Arg::with_name("output")
                        .long("output")
                        .short("o")
                        .help("A file path to print the diff to, if a format is provided")
                        .value_name("FILE")
                        .takes_value(true))
                .arg(
                    Arg::with_name("format")
                        .long("format")
                        .short("f")
                        .help("The format to print the diff in")
                        .value_name("FORMAT")
                        .takes_value(true)
                        .possible_values(&["json","yaml"]))
                .arg(
                    Arg::with_name("live")
                        .long("live")
                        .help("Verify persisted state against the live Roblox platform before diffing. Detects out-of-band drift (e.g. assets deleted manually) and reports missing or unverifiable resources alongside the input diff."))
        )
        .subcommand(
            SubCommand::with_name("destroy")
                .about("Destroys a Lithos environment.")
                .arg(
                    Arg::with_name("PROJECT")
                        .index(1)
                        .help(PROJECT_HELP)
                        .takes_value(true))
                .arg(
                    Arg::with_name("environment")
                        .long("environment")
                        .short("e")
                        .help("The label of the environment to destroy. If not specified, attempts to match the current git branch to each environment's `branches` property.")
                        .value_name("ENVIRONMENT")
                        .takes_value(true))
        )
        .subcommand(
            SubCommand::with_name("undo")
                .about("Best-effort rollback to the last known good snapshot for a Lithos environment.")
                .arg(
                    Arg::with_name("PROJECT")
                        .index(1)
                        .help(PROJECT_HELP)
                        .takes_value(true))
                .arg(
                    Arg::with_name("environment")
                        .long("environment")
                        .short("e")
                        .help("The label of the environment to roll back. If not specified, attempts to match the current git branch to each environment's `branches` property.")
                        .value_name("ENVIRONMENT")
                        .takes_value(true))
                .arg(
                    Arg::with_name("allow_purchases")
                        .long("allow-purchases")
                        .help("Gives Lithos permission to make purchases with Robux if restoring the last known good state requires re-creating paid resources."))
                .arg(
                    Arg::with_name("yes")
                        .long("yes")
                        .short("y")
                        .help("Skip the interactive plan confirmation prompt and apply immediately. In non-interactive (CI/piped) environments undo already auto-approves; this flag silences the auto-approve notice."))
                .arg(
                    Arg::with_name("no_preview")
                        .long("no-preview")
                        .help("Skip the rollback plan preview entirely. Implies --yes."))
                .arg(
                    Arg::with_name("plain_preview")
                        .long("plain-preview")
                        .help("Show a plain-text rollback summary instead of the rich color-coded preview. Useful for CI logs."))
        )
        .subcommand(
            SubCommand::with_name("outputs")
                .about("Prints a Lithos environment's outputs to the console or a file in a machine-readable format.")
                .arg(
                    Arg::with_name("PROJECT")
                        .index(1)
                        .help(PROJECT_HELP)
                        .takes_value(true))
                .arg(
                    Arg::with_name("environment")
                        .long("environment")
                        .short("e")
                        .help("The label of the environment to print the outputs of. If not specified, attempts to match the current git branch to each environment's `branches` property.")
                        .value_name("ENVIRONMENT")
                        .takes_value(true))
                .arg(
                    Arg::with_name("output")
                        .long("output")
                        .short("o")
                        .help("A file path to print the outputs to")
                        .value_name("FILE")
                        .takes_value(true))
                .arg(
                    Arg::with_name("format")
                        .long("format")
                        .short("f")
                        .help("The format to print the outputs in")
                        .value_name("FORMAT")
                        .takes_value(true)
                        .possible_values(&["json","yaml"])
                        .default_value("json"))
        )
        .subcommand(
            SubCommand::with_name("import")
                .about("Imports an existing target into a Lithos environment.")
                .arg(
                    Arg::with_name("PROJECT")
                        .index(1)
                        .help(PROJECT_HELP)
                        .takes_value(true))
                .arg(
                    Arg::with_name("environment")
                        .long("environment")
                        .short("e")
                        .help("The label of the environment to print the outputs of. If not specified, attempts to match the current git branch to each environment's `branches` property.")
                        .value_name("ENVIRONMENT")
                        .takes_value(true))
                .arg(
                    Arg::with_name("target_id")
                        .long("target-id")
                        .help("The ID of the target to import.")
                        .value_name("ID")
                        .takes_value(true)
                        .required(true))
        )
        .subcommand(
            SubCommand::with_name("state")
                .about("Manage state files.")
                .setting(AppSettings::ArgRequiredElseHelp)
                .subcommand(
                    SubCommand::with_name("download")
                        .about("Download the remote state file for a project. Remote state must be configured for the Lithos project.")
                        .arg(
                            Arg::with_name("PROJECT")
                                .index(1)
                                .help(PROJECT_HELP)
                                .takes_value(true))
                        .arg(
                            Arg::with_name("key")
                                .long("key")
                                .help("A key to prefix the name of the state file (e.g. `--key custom` will result in `custom.lithos-state.yml`).")
                                .value_name("KEY")
                                .takes_value(true))
                )
                .subcommand(
                    SubCommand::with_name("upload")
                        .about("Upload a state file to a remote provider for a project. Remote state must be configured for the Lithos project.")
                        .arg(
                            Arg::with_name("PROJECT")
                                .index(1)
                                .help(PROJECT_HELP)
                                .takes_value(true))
                        .arg(
                            Arg::with_name("key")
                                .long("key")
                                .help("The prefix of the name of the state file (e.g. `--key custom` will load from `custom.lithos-state.yml`).")
                                .value_name("KEY")
                                .takes_value(true))
                )
        )
}

pub async fn run_with(args: Vec<String>) -> i32 {
    // Print the branded banner before clap parses, so it precedes both
    // command output and clap's auto-generated --help / error messages.
    // Skip for --version which should remain a clean machine-readable line.
    let bare_version = args.iter().skip(1).any(|a| a == "--version" || a == "-V");
    if !bare_version {
        let probable_cmd = args
            .iter()
            .skip(1)
            .find(|a| !a.starts_with('-'))
            .map(|s| s.as_str());
        ui::print_banner(probable_cmd);
    }

    let app = get_app();
    let matches = app.get_matches_from(args);
    match matches.subcommand() {
        ("deploy", Some(deploy_matches)) => {
            let no_preview = deploy_matches.is_present("no_preview");
            let plain_preview = deploy_matches.is_present("plain_preview");
            let preview_options = PreviewOptions {
                mode: if no_preview {
                    PreviewMode::Off
                } else if plain_preview {
                    PreviewMode::Plain
                } else {
                    PreviewMode::Auto
                },
                assume_yes: deploy_matches.is_present("yes") || no_preview,
            };
            commands::deploy::run(
                deploy_matches.value_of("PROJECT"),
                deploy_matches.value_of("environment"),
                deploy_matches.is_present("allow_purchases"),
                preview_options,
            )
            .await
        }
        ("diff", Some(diff_matches)) => {
            commands::diff::run(
                diff_matches.value_of("PROJECT"),
                diff_matches.value_of("environment"),
                diff_matches.value_of("output"),
                diff_matches.value_of("format"),
                diff_matches.is_present("live"),
            )
            .await
        }
        ("destroy", Some(destroy_matches)) => {
            commands::destroy::run(
                destroy_matches.value_of("PROJECT"),
                destroy_matches.value_of("environment"),
            )
            .await
        }
        ("undo", Some(undo_matches)) => {
            let no_preview = undo_matches.is_present("no_preview");
            let plain_preview = undo_matches.is_present("plain_preview");
            let preview_options = PreviewOptions {
                mode: if no_preview {
                    PreviewMode::Off
                } else if plain_preview {
                    PreviewMode::Plain
                } else {
                    PreviewMode::Auto
                },
                assume_yes: undo_matches.is_present("yes") || no_preview,
            };
            commands::undo::run(
                undo_matches.value_of("PROJECT"),
                undo_matches.value_of("environment"),
                undo_matches.is_present("allow_purchases"),
                preview_options,
            )
            .await
        }
        ("outputs", Some(outputs_matches)) => {
            commands::outputs::run(
                outputs_matches.value_of("PROJECT"),
                outputs_matches.value_of("environment"),
                outputs_matches.value_of("output"),
                outputs_matches.value_of("format").unwrap(),
            )
            .await
        }
        ("import", Some(import_matches)) => {
            commands::import::run(
                import_matches.value_of("PROJECT"),
                import_matches.value_of("environment"),
                import_matches.value_of("target_id").unwrap(),
            )
            .await
        }
        ("state", Some(state_matches)) => match state_matches.subcommand() {
            ("download", Some(download_matches)) => {
                commands::download::run(
                    download_matches.value_of("PROJECT"),
                    download_matches.value_of("key"),
                )
                .await
            }
            ("upload", Some(upload_matches)) => {
                commands::upload::run(
                    upload_matches.value_of("PROJECT"),
                    upload_matches.value_of("key"),
                )
                .await
            }
            _ => unreachable!(),
        },
        _ => unreachable!(),
    }
}

pub async fn run() -> i32 {
    run_with(env::args().collect()).await
}
