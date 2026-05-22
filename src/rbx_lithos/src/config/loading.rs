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

use super::{luau, Config};

const PRIMARY_CONFIG_FILENAMES: &[&str] = &[
    "lithos.yml",
    "lithos.yaml",
    "lithos.json",
    "lithos.luau",
    "lithos.lua",
];
const LEGACY_CONFIG_FILENAMES: &[&str] = &["mantle.yml", "mantle.yaml"];

fn config_candidates(project_path: &Path) -> Vec<PathBuf> {
    PRIMARY_CONFIG_FILENAMES
        .iter()
        .chain(LEGACY_CONFIG_FILENAMES.iter())
        .map(|file_name| project_path.join(file_name))
        .collect()
}

fn format_project_config_search_order(project_path: &Path) -> String {
    config_candidates(project_path)
        .iter()
        .map(|path| format!("'{}'", path.display()))
        .collect::<Vec<_>>()
        .join(", then ")
}

fn parse_project_path(project: Option<&str>) -> Result<(PathBuf, PathBuf), String> {
    let project = project.unwrap_or(".");
    let project_path = Path::new(project).to_owned();

    if project_path.is_dir() {
        let existing_configs = config_candidates(&project_path)
            .iter()
            .filter(|path| path.is_file())
            .cloned()
            .collect::<Vec<_>>();

        if let Some(config_path) = existing_configs.first() {
            if existing_configs.len() > 1 {
                let present_configs = existing_configs
                    .iter()
                    .map(|path| format!("'{}'", path.file_name().unwrap().to_string_lossy()))
                    .collect::<Vec<_>>()
                    .join(", ");

                logger::log(format!(
                    "{} Found multiple project config files ({}). Using {} because Lithos checks {}.",
                    Paint::yellow("warning:"),
                    present_configs,
                    Paint::cyan(config_path.display()),
                    format_project_config_search_order(&project_path)
                ));
            }

            if LEGACY_CONFIG_FILENAMES.iter().any(|legacy_name| {
                config_path.file_name().and_then(|name| name.to_str()) == Some(*legacy_name)
            }) {
                let legacy_name = config_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("mantle.yml");
                logger::log(format!(
                    "{} Loading legacy '{}'. Lithos checks {}. Rename the file to 'lithos.yml', 'lithos.yaml', or 'lithos.json' to silence this notice.",
                    Paint::yellow("warning:"),
                    legacy_name,
                    format_project_config_search_order(&project_path),
                ));
            }

            return Ok((project_path, config_path.clone()));
        }

        return Err(format!(
            "No project config found in {}. Lithos checks {}.",
            project_path.display(),
            format_project_config_search_order(&project_path)
        ));
    }

    if project_path.is_file() {
        return Ok((project_path.parent().unwrap().into(), project_path));
    }

    Err(format!("Unable to load project path: {}", project))
}

fn load_config_file(config_file: &Path) -> Result<Config, String> {
    if luau::is_lua_config_path(config_file) {
        return luau::load_lua_config(config_file).map(|eval| eval.config);
    }

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
        sync::atomic::{AtomicUsize, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        load_project_config, parse_project_path, LEGACY_CONFIG_FILENAMES, PRIMARY_CONFIG_FILENAMES,
    };

    static NEXT_TEMP_DIR_ID: AtomicUsize = AtomicUsize::new(0);

    const YML_CONFIG: &str = r#"environments:
  - label: yml-preferred
    branches: [main]
target:
  experience:
    places:
      start:
        file: place.rbxl
"#;

    const YAML_CONFIG: &str = r#"environments:
  - label: yaml-supported
    branches: [main]
target:
  experience:
    places:
      start:
        file: place.rbxl
"#;

    const JSON_CONFIG: &str = r#"{
  "environments": [
    {
      "label": "json-supported",
      "branches": ["main"]
    }
  ],
  "target": {
    "experience": {
      "places": {
        "start": {
          "file": "place.rbxl"
        }
      }
    }
  }
}"#;

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

    struct TempProjectDir {
        path: PathBuf,
    }

    impl TempProjectDir {
        fn new() -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos();
            let mut path = env::temp_dir();
            path.push(format!(
                "lithos-config-loading-{}-{}-{}",
                process::id(),
                timestamp,
                NEXT_TEMP_DIR_ID.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).expect("temp project directory should be created");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write(&self, file_name: &str, contents: &str) -> PathBuf {
            let file_path = self.path.join(file_name);
            fs::write(&file_path, contents).expect("project config file should be written");
            file_path
        }
    }

    impl Drop for TempProjectDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn discovers_lithos_yaml_when_directory_has_no_yml_or_json() {
        let project_dir = TempProjectDir::new();
        project_dir.write(PRIMARY_CONFIG_FILENAMES[1], YAML_CONFIG);

        let (project_path, config) =
            load_project_config(Some(project_dir.path().to_str().unwrap())).unwrap();

        assert_eq!(project_path, project_dir.path());
        assert_eq!(config.environments[0].label, "yaml-supported");
    }

    #[test]
    fn discovers_lithos_json_when_directory_has_no_yaml() {
        let project_dir = TempProjectDir::new();
        project_dir.write(PRIMARY_CONFIG_FILENAMES[2], JSON_CONFIG);

        let (project_path, config) =
            load_project_config(Some(project_dir.path().to_str().unwrap())).unwrap();

        assert_eq!(project_path, project_dir.path());
        assert_eq!(config.environments[0].label, "json-supported");
    }

    #[test]
    fn prefers_lithos_yml_over_lithos_yaml_and_json() {
        let project_dir = TempProjectDir::new();
        project_dir.write(PRIMARY_CONFIG_FILENAMES[0], YML_CONFIG);
        project_dir.write(PRIMARY_CONFIG_FILENAMES[1], YAML_CONFIG);
        project_dir.write(PRIMARY_CONFIG_FILENAMES[2], JSON_CONFIG);

        let (_, config) = load_project_config(Some(project_dir.path().to_str().unwrap())).unwrap();

        assert_eq!(config.environments[0].label, "yml-preferred");
    }

    #[test]
    fn loads_explicit_lithos_json_path() {
        let project_dir = TempProjectDir::new();
        let config_path = project_dir.write(PRIMARY_CONFIG_FILENAMES[2], JSON_CONFIG);

        let (project_path, config) =
            load_project_config(Some(config_path.to_str().unwrap())).unwrap();

        assert_eq!(project_path, project_dir.path());
        assert_eq!(config.environments[0].label, "json-supported");
    }

    #[test]
    fn missing_config_error_mentions_search_path() {
        let project_dir = TempProjectDir::new();

        let error = load_project_config(Some(project_dir.path().to_str().unwrap()))
            .err()
            .unwrap();

        assert!(error.contains(PRIMARY_CONFIG_FILENAMES[0]));
        assert!(error.contains(PRIMARY_CONFIG_FILENAMES[1]));
        assert!(error.contains(PRIMARY_CONFIG_FILENAMES[2]));
        assert!(error.contains(LEGACY_CONFIG_FILENAMES[0]));
        assert!(error.contains(LEGACY_CONFIG_FILENAMES[1]));
    }

    #[test]
    fn discovery_prefers_yaml_and_json_over_luau() {
        // Selection should be a pure function of which files exist; verify
        // precedence without actually evaluating any of them.
        let project_dir = TempProjectDir::new();
        project_dir.write("lithos.json", JSON_CONFIG);
        project_dir.write("lithos.luau", "return {}");
        project_dir.write("lithos.lua", "return {}");

        let (_, config_path) =
            parse_project_path(Some(project_dir.path().to_str().unwrap())).unwrap();

        assert_eq!(config_path.file_name().unwrap(), "lithos.json");
    }

    #[test]
    fn discovery_prefers_luau_over_lua_and_legacy_mantle() {
        let project_dir = TempProjectDir::new();
        project_dir.write("lithos.luau", "return {}");
        project_dir.write("lithos.lua", "return {}");
        project_dir.write("mantle.yml", YML_CONFIG);

        let (_, config_path) =
            parse_project_path(Some(project_dir.path().to_str().unwrap())).unwrap();

        assert_eq!(config_path.file_name().unwrap(), "lithos.luau");
    }

    #[test]
    fn missing_config_error_mentions_luau_in_search_path() {
        let project_dir = TempProjectDir::new();

        let error = load_project_config(Some(project_dir.path().to_str().unwrap()))
            .err()
            .unwrap();

        assert!(error.contains("lithos.luau"));
        assert!(error.contains("lithos.lua"));
    }

    #[test]
    fn load_project_config_loads_project_dotenv() {
        let env_key = "LITHOS_TEST_PROJECT_DOTENV_8740";
        let _env_guard = EnvVarGuard::capture(env_key);
        env::remove_var(env_key);

        let project_dir = TempProjectDir::new();
        fs::write(
            project_dir.path().join(".env"),
            format!("{}=loaded-from-project\n", env_key),
        )
        .expect("project dotenv file should be written");
        project_dir.write(PRIMARY_CONFIG_FILENAMES[0], YML_CONFIG);

        let project_path = project_dir.path().to_string_lossy().to_string();
        load_project_config(Some(&project_path)).expect("project config should load successfully");

        assert_eq!(
            env::var(env_key).as_deref(),
            Ok("loaded-from-project"),
            "expected the project root .env file to be loaded"
        );
    }
}
