//! Filesystem loading helpers for project configuration.
//!
//! Side-effectful boundary: reads from disk, prints to the logger.
//! Pure type definitions live in the parent [`config`](super) module.

use std::{
    fs,
    path::{Path, PathBuf},
};

use yansi::Paint;

use super::Config;

fn parse_project_path(project: Option<&str>) -> Result<(PathBuf, PathBuf), String> {
    let project = project.unwrap_or(".");
    let project_path = Path::new(project).to_owned();

    if project_path.is_dir() {
        // Prefer the new Lithos config name; fall back to the legacy Mantle name
        // for backward compatibility.
        let lithos_config = project_path.join("lithos.yml");
        if lithos_config.exists() {
            return Ok((project_path, lithos_config));
        }

        let legacy_config = project_path.join("mantle.yml");
        if legacy_config.exists() {
            logger::log(format!(
                "{} Loading legacy 'mantle.yml'. Rename to 'lithos.yml' to silence this notice.",
                Paint::yellow("warning:")
            ));
            return Ok((project_path, legacy_config));
        }

        return Err(format!(
            "Config file {} not found (also tried legacy {})",
            lithos_config.display(),
            legacy_config.display()
        ));
    } else if project_path.is_file() {
        return Ok((project_path.parent().unwrap().into(), project_path));
    }

    Err(format!("Unable to load project path: {}", project))
}

fn load_config_file(config_file: &Path) -> Result<Config, String> {
    let data = fs::read_to_string(config_file).map_err(|e| {
        format!(
            "Unable to read config file: {}\n\t{}",
            config_file.display(),
            e
        )
    })?;

    serde_yaml::from_str::<Config>(&data).map_err(|e| {
        format!(
            "Unable to parse config file {}\n\t{}",
            config_file.display(),
            e
        )
    })
}

pub fn load_project_config(project: Option<&str>) -> Result<(PathBuf, Config), String> {
    let (project_path, config_path) = parse_project_path(project)?;
    let config = load_config_file(&config_path)?;

    logger::log(format!(
        "Loaded config file {}",
        Paint::cyan(config_path.display())
    ));

    Ok((project_path, config))
}
