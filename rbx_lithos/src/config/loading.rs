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

const PRIMARY_PROJECT_CONFIG_NAME: &str = "lithos.yml";
const JSON_PROJECT_CONFIG_NAME: &str = "lithos.json";
const LEGACY_PROJECT_CONFIG_NAME: &str = "mantle.yml";

fn project_config_candidates(project_path: &Path) -> [PathBuf; 3] {
    [
        project_path.join(PRIMARY_PROJECT_CONFIG_NAME),
        project_path.join(JSON_PROJECT_CONFIG_NAME),
        project_path.join(LEGACY_PROJECT_CONFIG_NAME),
    ]
}

fn format_project_config_search_order(project_path: &Path) -> String {
    let candidates = project_config_candidates(project_path);
    format!(
        "'{}', then '{}', then legacy '{}'",
        candidates[0].display(),
        candidates[1].display(),
        candidates[2].display()
    )
}

fn parse_project_path(project: Option<&str>) -> Result<(PathBuf, PathBuf), String> {
    let project = project.unwrap_or(".");
    let project_path = Path::new(project).to_owned();

    if project_path.is_dir() {
        let config_candidates = project_config_candidates(&project_path);
        let existing_configs = config_candidates
            .iter()
            .filter(|path| path.exists())
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

            if config_path.file_name().and_then(|name| name.to_str())
                == Some(LEGACY_PROJECT_CONFIG_NAME)
            {
                logger::log(format!(
                    "{} Loading legacy '{}'. Lithos checks {}. Rename the file to '{}' or '{}' to silence this notice.",
                    Paint::yellow("warning:"),
                    LEGACY_PROJECT_CONFIG_NAME,
                    format_project_config_search_order(&project_path),
                    PRIMARY_PROJECT_CONFIG_NAME,
                    JSON_PROJECT_CONFIG_NAME
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

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        path::{Path, PathBuf},
        process,
        sync::atomic::{AtomicUsize, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        load_project_config, JSON_PROJECT_CONFIG_NAME, LEGACY_PROJECT_CONFIG_NAME,
        PRIMARY_PROJECT_CONFIG_NAME,
    };

    static NEXT_TEMP_DIR_ID: AtomicUsize = AtomicUsize::new(0);

    const YAML_CONFIG: &str = r#"environments:
  - label: yaml-preferred
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

    struct TempProjectDir {
        path: PathBuf,
    }

    impl TempProjectDir {
        fn new() -> Self {
            let mut path = env::temp_dir();
            path.push(format!(
                "lithos-config-loading-{}-{}-{}",
                process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT_TEMP_DIR_ID.fetch_add(1, Ordering::Relaxed)
            ));

            fs::create_dir_all(&path).unwrap();

            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write(&self, file_name: &str, contents: &str) -> PathBuf {
            let file_path = self.path.join(file_name);
            fs::write(&file_path, contents).unwrap();
            file_path
        }
    }

    impl Drop for TempProjectDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn discovers_lithos_json_when_directory_has_no_yaml() {
        let project_dir = TempProjectDir::new();
        project_dir.write(JSON_PROJECT_CONFIG_NAME, JSON_CONFIG);

        let (project_path, config) =
            load_project_config(Some(project_dir.path().to_str().unwrap())).unwrap();

        assert_eq!(project_path, project_dir.path());
        assert_eq!(config.environments[0].label, "json-supported");
    }

    #[test]
    fn prefers_lithos_yml_over_lithos_json() {
        let project_dir = TempProjectDir::new();
        project_dir.write(PRIMARY_PROJECT_CONFIG_NAME, YAML_CONFIG);
        project_dir.write(JSON_PROJECT_CONFIG_NAME, JSON_CONFIG);

        let (_, config) = load_project_config(Some(project_dir.path().to_str().unwrap())).unwrap();

        assert_eq!(config.environments[0].label, "yaml-preferred");
    }

    #[test]
    fn loads_explicit_lithos_json_path() {
        let project_dir = TempProjectDir::new();
        let config_path = project_dir.write(JSON_PROJECT_CONFIG_NAME, JSON_CONFIG);

        let (project_path, config) =
            load_project_config(Some(config_path.to_str().unwrap())).unwrap();

        assert_eq!(project_path, project_dir.path());
        assert_eq!(config.environments[0].label, "json-supported");
    }

    #[test]
    fn missing_config_error_mentions_json_search_path() {
        let project_dir = TempProjectDir::new();

        let error = load_project_config(Some(project_dir.path().to_str().unwrap()))
            .err()
            .unwrap();

        assert!(error.contains(PRIMARY_PROJECT_CONFIG_NAME));
        assert!(error.contains(JSON_PROJECT_CONFIG_NAME));
        assert!(error.contains(LEGACY_PROJECT_CONFIG_NAME));
    }
}
