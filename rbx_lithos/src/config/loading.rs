//! Filesystem loading helpers for project configuration.
//!
//! Side-effectful boundary: reads from disk, prints to the logger.
//! Pure type definitions live in the parent [`config`](super) module.

use std::{
    fs,
    path::{Path, PathBuf},
};

use dotenv::from_path;
use log::info;
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

fn load_project_dotenv(project_path: &Path) {
    let dotenv_path = project_path.join(".env");
    if !dotenv_path.is_file() {
        return;
    }

    match from_path(&dotenv_path) {
        Ok(()) => info!(
            "Loaded variables from project dotenv file: {}",
            dotenv_path.display()
        ),
        Err(err) => info!(
            "Failed to load variables from project dotenv file {}: {}",
            dotenv_path.display(),
            err
        ),
    }
}

pub fn load_project_config(project: Option<&str>) -> Result<(PathBuf, Config), String> {
    let (project_path, config_path) = parse_project_path(project)?;
    let config = load_config_file(&config_path)?;
    load_project_dotenv(&project_path);

    logger::log(format!(
        "Loaded config file {}",
        Paint::cyan(config_path.display())
    ));

    Ok((project_path, config))
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        ffi::OsString,
        fs,
        path::{Path, PathBuf},
        process,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::load_project_config;

    struct EnvVarGuard {
        key: &'static str,
        previous_value: Option<OsString>,
    }

    impl EnvVarGuard {
        fn capture(key: &'static str) -> Self {
            Self {
                key,
                previous_value: env::var_os(key),
            }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(value) = self.previous_value.as_ref() {
                env::set_var(self.key, value);
            } else {
                env::remove_var(self.key);
            }
        }
    }

    struct TempDirGuard {
        path: PathBuf,
    }

    impl TempDirGuard {
        fn new(prefix: &str) -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos();
            let path = env::temp_dir().join(format!("{}-{}-{}", prefix, process::id(), timestamp));
            fs::create_dir_all(&path).expect("temp project directory should be created");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn load_project_config_loads_project_dotenv() {
        let env_key = "LITHOS_TEST_PROJECT_DOTENV_8740";
        let _env_guard = EnvVarGuard::capture(env_key);
        env::remove_var(env_key);

        let project_dir = TempDirGuard::new("lithos-project-dotenv");
        fs::write(
            project_dir.path().join(".env"),
            format!("{}=loaded-from-project\n", env_key),
        )
        .expect("project dotenv file should be written");
        fs::write(
            project_dir.path().join("lithos.yml"),
            r#"environments:
  - label: dev
target:
  experience:
    configuration:
      genre: building
      playableDevices: [computer]
    places:
      start:
        file: game.rbxlx
        configuration:
          name: Example
"#,
        )
        .expect("project config file should be written");

        let project_path = project_dir.path().to_string_lossy().to_string();
        load_project_config(Some(&project_path)).expect("project config should load successfully");

        assert_eq!(
            env::var(env_key).as_deref(),
            Ok("loaded-from-project"),
            "expected the project root .env file to be loaded"
        );
    }
}
