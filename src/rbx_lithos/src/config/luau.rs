//! Luau / Lua project config evaluation.
//!
//! Lithos delegates execution to [Lune](https://lune-org.github.io/docs), an
//! external Luau runtime aimed at Roblox tooling. We do not interpret Luau
//! ourselves: a small wrapper script is handed to `lune run`, which `require`s
//! the user's config file and prints the resulting table as JSON between
//! sentinel markers. Rust then parses the markers and decodes the JSON
//! payload into [`Config`].
//!
//! This keeps the surface area small while letting users write idiomatic
//! Luau (helper functions, loops, environment-variable branching, ...) just
//! like any other Lune script. Documented capabilities and limits live in the
//! `docs/site/pages/docs/configuration` pages.
//!
//! Side-effectful boundary: spawns a subprocess, writes a temp file, reads
//! environment variables, and prints to the logger on failure.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

use super::Config;

/// Environment variable that overrides the `lune` binary used to evaluate
/// `.luau` / `.lua` configs. Useful for pinning a specific Lune build in CI
/// or pointing at a forked runtime.
pub const LUNE_BIN_ENV: &str = "LITHOS_LUNE";

const DEFAULT_LUNE_BIN: &str = "lune";

const CONFIG_BEGIN_MARKER: &str = "@@LITHOS_CONFIG_BEGIN@@";
const CONFIG_END_MARKER: &str = "@@LITHOS_CONFIG_END@@";

/// Lune wrapper script. Receives the require-path to the user's config (no
/// extension, relative to this wrapper's location) as `process.args[1]` and
/// prints the resulting config table as JSON between sentinel markers.
///
/// Errors are written to stderr and surfaced via the process exit code.
const WRAPPER_SCRIPT: &str = include_str!("luau_wrapper.luau");

/// Result of evaluating a Luau / Lua config file.
pub struct LuauEvaluation {
    pub config: Config,
    /// Names of hook functions the user defined at the top level of the
    /// returned table (e.g. `onConfigLoaded`, `onBeforeDeploy`). Lithos uses
    /// this list to log which hooks were registered and to know whether to
    /// re-invoke Lune for lifecycle hooks in future deploy steps.
    #[allow(dead_code)] // Reserved for upcoming deploy-lifecycle integration.
    pub hooks: Vec<String>,
}

/// Returns `true` for file paths Lithos should evaluate via Lune.
pub fn is_lua_config_path(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|s| s.to_str()),
        Some("lua") | Some("luau")
    )
}

/// Evaluate a Luau or Lua project config and decode its returned table as
/// a [`Config`].
pub fn load_lua_config(config_file: &Path) -> Result<LuauEvaluation, String> {
    let canonical_raw = config_file.canonicalize().map_err(|e| {
        format!(
            "Unable to resolve Luau config path {}: {}",
            config_file.display(),
            e
        )
    })?;
    // Windows `canonicalize()` returns paths with the `\\?\` verbatim
    // prefix. Lune does not normalize that prefix when resolving relative
    // `require()` calls from the running script, so we strip it before
    // handing the wrapper path to Lune.
    let canonical = strip_verbatim_prefix(&canonical_raw);
    let user_dir = canonical.parent().ok_or_else(|| {
        format!(
            "Luau config path {} has no parent directory",
            config_file.display()
        )
    })?;
    let user_stem = canonical
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| {
            format!(
                "Luau config path {} has no valid file stem",
                config_file.display()
            )
        })?;

    let lune_bin = env::var(LUNE_BIN_ENV).unwrap_or_else(|_| DEFAULT_LUNE_BIN.to_string());

    // Place the wrapper inside the same directory as the user's config so the
    // `require()` path is just `./<stem>`. Lune's require() resolves relative
    // to the calling script and cross-directory traversal has proven brittle
    // on Windows (canonical `\\?\` prefixes, separator normalization, etc.).
    let wrapper = WrapperFile::new_in(user_dir)?;
    let require_path = format!("./{}", user_stem);

    let output = Command::new(&lune_bin)
        .arg("run")
        .arg(wrapper.path())
        .arg(&require_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| {
            format!(
                "Failed to launch Lune ('{}') to evaluate {}: {}\n\
                 Hint: install Lune from https://lune-org.github.io/docs and ensure it is on PATH,\n\
                 or set the {} environment variable to point at a Lune binary.",
                lune_bin,
                config_file.display(),
                e,
                LUNE_BIN_ENV,
            )
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(format!(
            "Lune failed to evaluate {} (exit code {}):\n{}",
            config_file.display(),
            output
                .status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "<signal>".into()),
            indent_block(stderr.trim_end()),
        ));
    }

    let json_payload = extract_marker_payload(&stdout, &stderr, config_file)?;

    #[derive(serde::Deserialize)]
    struct Envelope {
        config: serde_json::Value,
        #[serde(default)]
        hooks: Vec<String>,
    }

    let envelope: Envelope = serde_json::from_str(&json_payload).map_err(|e| {
        format!(
            "Luau config {} produced a payload Lithos could not decode:\n\t{}",
            config_file.display(),
            e
        )
    })?;

    let config: Config = serde_json::from_value(envelope.config).map_err(|e| {
        format!(
            "Luau config {} returned data that does not match the Lithos config schema:\n\t{}",
            config_file.display(),
            e
        )
    })?;

    Ok(LuauEvaluation {
        config,
        hooks: envelope.hooks,
    })
}

fn extract_marker_payload(
    stdout: &str,
    stderr: &str,
    config_file: &Path,
) -> Result<String, String> {
    let start = stdout.find(CONFIG_BEGIN_MARKER).ok_or_else(|| {
        format!(
            "Lune did not emit a config payload for {}. Make sure your script returns a table or \
             `{{ config = <table> }}`.\n\
             stdout:\n{}\n\
             stderr:\n{}",
            config_file.display(),
            indent_block(stdout.trim_end()),
            indent_block(stderr.trim_end()),
        )
    })?;
    let after_begin = start + CONFIG_BEGIN_MARKER.len();
    let end_rel = stdout[after_begin..]
        .find(CONFIG_END_MARKER)
        .ok_or_else(|| {
            format!(
                "Lune emitted a truncated config payload for {} (missing end marker).",
                config_file.display()
            )
        })?;
    Ok(stdout[after_begin..after_begin + end_rel]
        .trim()
        .to_string())
}

/// Strips the Windows verbatim `\\?\` prefix from a path if present. On
/// non-Windows platforms (and for paths without the prefix) the input is
/// returned unchanged. Lune chokes on verbatim-prefixed script paths when
/// resolving relative requires, so we hand it normal `C:\...` paths.
fn strip_verbatim_prefix(path: &Path) -> PathBuf {
    let lossy = path.to_string_lossy();
    if let Some(stripped) = lossy.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

fn indent_block(text: &str) -> String {
    if text.is_empty() {
        return "\t<empty>".to_string();
    }
    text.lines()
        .map(|line| format!("\t{}", line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Computes a `require()`-compatible path from `from_dir` to `target_file`
/// with the extension stripped. The result always starts with `./` or `../`
/// so Lune accepts it. Both paths must already be absolute.
///
/// Kept available for tests and possible future cross-directory use; the
/// production loader places the wrapper inside the user's config directory
/// instead and uses the simpler `./<stem>` form.
#[cfg(test)]
fn relative_require_path(from_dir: &Path, target_file: &Path) -> String {
    let stripped = target_file.with_extension("");
    let from = normalize_components(from_dir);
    let to = normalize_components(&stripped);

    let mut common = 0;
    while common < from.len() && common < to.len() && from[common] == to[common] {
        common += 1;
    }

    let ups = from.len() - common;
    let mut parts: Vec<String> = Vec::new();
    if ups == 0 {
        parts.push(".".to_string());
    } else {
        for _ in 0..ups {
            parts.push("..".to_string());
        }
    }
    for c in &to[common..] {
        parts.push(c.clone());
    }
    parts.join("/")
}

#[cfg(test)]
fn normalize_components(path: &Path) -> Vec<String> {
    use std::path::Component;
    path.components()
        .filter_map(|c| match c {
            Component::Prefix(prefix) => Some(prefix.as_os_str().to_string_lossy().to_string()),
            Component::RootDir => None,
            Component::CurDir => None,
            Component::ParentDir => Some("..".to_string()),
            Component::Normal(s) => Some(s.to_string_lossy().to_string()),
        })
        .collect()
}

/// Self-cleaning temp file holding the Lune wrapper script. The wrapper is
/// written into `host_dir` with a unique hidden file name so the wrapper can
/// `require("./<user-stem>")` to reach the user's config.
struct WrapperFile {
    path: PathBuf,
}

impl WrapperFile {
    fn new_in(host_dir: &Path) -> Result<Self, String> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let name = format!(
            ".lithos-luau-eval-{}-{}.luau",
            std::process::id(),
            timestamp
        );
        let path = host_dir.join(name);
        fs::write(&path, WRAPPER_SCRIPT).map_err(|e| {
            format!(
                "Unable to write Luau wrapper script to {}: {}",
                path.display(),
                e
            )
        })?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for WrapperFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn relative_require_path_within_same_subtree() {
        let from = PathBuf::from(if cfg!(windows) {
            r"C:\tmp\wrap"
        } else {
            "/tmp/wrap"
        });
        let target = PathBuf::from(if cfg!(windows) {
            r"C:\projects\demo\lithos.luau"
        } else {
            "/projects/demo/lithos.luau"
        });
        let rel = relative_require_path(&from, &target);
        // Always starts with ./ or ../
        assert!(rel.starts_with("./") || rel.starts_with("../"));
        assert!(rel.ends_with("/lithos"));
    }

    #[test]
    fn is_lua_config_path_detects_both_extensions() {
        assert!(is_lua_config_path(Path::new("lithos.lua")));
        assert!(is_lua_config_path(Path::new("lithos.luau")));
        assert!(!is_lua_config_path(Path::new("lithos.yml")));
        assert!(!is_lua_config_path(Path::new("lithos.json")));
    }

    // ----- Lune-gated end-to-end tests ----------------------------------
    //
    // These tests shell out to a real `lune` binary. They are skipped (and
    // print a clear message) when Lune is not on PATH so contributors who
    // have not installed Lune locally can still run `cargo test`.

    fn lune_available() -> bool {
        let bin = std::env::var(LUNE_BIN_ENV).unwrap_or_else(|_| DEFAULT_LUNE_BIN.to_string());
        Command::new(bin)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    struct TempLuauDir(PathBuf);

    impl TempLuauDir {
        fn new() -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let mut p = env::temp_dir();
            p.push(format!("lithos-luau-test-{}-{}", std::process::id(), nanos));
            fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn write(&self, name: &str, content: &str) -> PathBuf {
            let f = self.0.join(name);
            fs::write(&f, content).unwrap();
            f
        }
    }

    impl Drop for TempLuauDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const VALID_LUAU: &str = r#"
local config = {
    environments = {
        { label = "production", branches = { "main" } },
    },
    target = {
        experience = {
            places = {
                start = { file = "place.rbxl" },
            },
        },
    },
}
return config
"#;

    #[test]
    fn evaluates_valid_luau_config_to_typed_config() {
        if !lune_available() {
            eprintln!("skipping: lune not available on PATH");
            return;
        }
        let dir = TempLuauDir::new();
        let file = dir.write("lithos.luau", VALID_LUAU);

        let eval = match load_lua_config(&file) {
            Ok(eval) => eval,
            Err(err) => panic!("luau evaluation should succeed: {}", err),
        };

        assert_eq!(eval.config.environments.len(), 1);
        assert_eq!(eval.config.environments[0].label, "production");
    }

    #[test]
    fn accepts_table_with_explicit_config_field() {
        if !lune_available() {
            eprintln!("skipping: lune not available on PATH");
            return;
        }
        let dir = TempLuauDir::new();
        let file = dir.write(
            "lithos.luau",
            r#"
return {
    config = {
        environments = { { label = "wrapped", branches = { "main" } } },
        target = { experience = { places = { start = { file = "p.rbxl" } } } },
    },
}
"#,
        );

        let eval = match load_lua_config(&file) {
            Ok(eval) => eval,
            Err(err) => panic!("luau evaluation should succeed: {}", err),
        };
        assert_eq!(eval.config.environments[0].label, "wrapped");
    }

    #[test]
    fn surfaces_runtime_errors_with_config_path() {
        if !lune_available() {
            eprintln!("skipping: lune not available on PATH");
            return;
        }
        let dir = TempLuauDir::new();
        let file = dir.write(
            "lithos.luau",
            r#"error("boom from user config")
"#,
        );

        let err = match load_lua_config(&file) {
            Ok(_) => panic!("runtime error should fail load"),
            Err(e) => e,
        };
        assert!(
            err.contains(file.file_name().unwrap().to_string_lossy().as_ref()),
            "error should mention the config file path; got: {}",
            err
        );
        assert!(
            err.contains("boom from user config"),
            "error should preserve the underlying message; got: {}",
            err
        );
    }

    #[test]
    fn rejects_non_table_return_values() {
        if !lune_available() {
            eprintln!("skipping: lune not available on PATH");
            return;
        }
        let dir = TempLuauDir::new();
        let file = dir.write("lithos.luau", "return 42\n");

        let err = match load_lua_config(&file) {
            Ok(_) => panic!("scalar return should fail"),
            Err(e) => e,
        };
        assert!(err.to_lowercase().contains("table"), "got: {}", err);
    }

    #[test]
    fn rejects_invalid_config_shape_with_schema_error() {
        if !lune_available() {
            eprintln!("skipping: lune not available on PATH");
            return;
        }
        let dir = TempLuauDir::new();
        // `environments` and `target` are required.
        let file = dir.write(
            "lithos.luau",
            "return { totallyUnknownTopLevelKey = true }\n",
        );

        let err = match load_lua_config(&file) {
            Ok(_) => panic!("schema mismatch should fail"),
            Err(e) => e,
        };
        assert!(
            err.contains("schema") || err.contains("missing") || err.contains("unknown"),
            "expected a schema-style error; got: {}",
            err
        );
    }

    #[test]
    fn on_config_loaded_hook_can_transform_config() {
        if !lune_available() {
            eprintln!("skipping: lune not available on PATH");
            return;
        }
        let dir = TempLuauDir::new();
        let file = dir.write(
            "lithos.luau",
            r#"
return {
    config = {
        environments = { { label = "production", branches = { "main" } } },
        target = { experience = { places = { start = { file = "p.rbxl" } } } },
    },
    onConfigLoaded = function(config)
        table.insert(config.environments, { label = "staging", branches = { "develop" } })
        return config
    end,
    onBeforeDeploy = function() end,
}
"#,
        );

        let eval = match load_lua_config(&file) {
            Ok(eval) => eval,
            Err(err) => panic!("hook evaluation should succeed: {}", err),
        };
        assert_eq!(eval.config.environments.len(), 2);
        assert_eq!(eval.config.environments[1].label, "staging");
        let mut hook_names = eval.hooks.clone();
        hook_names.sort();
        assert_eq!(hook_names, vec!["onBeforeDeploy", "onConfigLoaded"]);
    }

    #[test]
    fn on_config_loaded_hook_failure_is_surfaced() {
        if !lune_available() {
            eprintln!("skipping: lune not available on PATH");
            return;
        }
        let dir = TempLuauDir::new();
        let file = dir.write(
            "lithos.luau",
            r#"
return {
    config = {
        environments = { { label = "production", branches = { "main" } } },
        target = { experience = { places = { start = { file = "p.rbxl" } } } },
    },
    onConfigLoaded = function() error("hook exploded") end,
}
"#,
        );

        let err = match load_lua_config(&file) {
            Ok(_) => panic!("hook failure should propagate"),
            Err(e) => e,
        };
        assert!(
            err.contains("onConfigLoaded") && err.contains("hook exploded"),
            "expected hook error to surface; got: {}",
            err
        );
    }
}
