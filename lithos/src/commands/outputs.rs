use std::{collections::BTreeMap, fmt::Write as _, fs, path::Path};

use serde_json::{Map, Value};
use yansi::Paint;

use rbx_lithos::{
    config::load_project_config,
    project::{load_project, Project},
    resource_graph::Resource,
    roblox_resource_manager::RobloxOutputs,
};

type OutputsMap = BTreeMap<String, Option<RobloxOutputs>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OutputFormat {
    Json,
    Yaml,
    Luau,
}

impl OutputFormat {
    fn parse(format: &str) -> Result<Self, String> {
        match format {
            "json" => Ok(Self::Json),
            "yaml" => Ok(Self::Yaml),
            "lua" | "luau" => Ok(Self::Luau),
            _ => Err(format!("Unknown format: {}", format)),
        }
    }

    fn infer(output: Option<&str>, format: Option<&str>) -> Result<Self, String> {
        if let Some(format) = format {
            return Self::parse(format);
        }

        Ok(output
            .and_then(Self::from_output_path)
            .unwrap_or(Self::Json))
    }

    fn from_output_path(output_path: &str) -> Option<Self> {
        let lower = output_path.to_ascii_lowercase();
        match Path::new(&lower).extension().and_then(|ext| ext.to_str()) {
            Some("luau") | Some("lua") => Some(Self::Luau),
            _ => None,
        }
    }
}

pub async fn run(
    project: Option<&str>,
    environment: Option<&str>,
    output: Option<&str>,
    format: Option<&str>,
    roblox_ts: bool,
) -> i32 {
    logger::start_action("Load outputs:");
    let (project_path, config) = match load_project_config(project) {
        Ok(v) => v,
        Err(e) => {
            logger::end_action(Paint::red(e));
            return 1;
        }
    };
    let Project { current_graph, .. } =
        match load_project(project_path.clone(), config, environment).await {
            Ok(Some(v)) => v,
            Ok(None) => {
                logger::end_action("No outputs available");
                return 0;
            }
            Err(e) => {
                logger::end_action(Paint::red(e));
                return 1;
            }
        };

    let resources = current_graph.get_resource_list();
    let outputs_map = resources
        .iter()
        .map(|r| (r.get_id(), r.get_outputs()))
        .collect::<OutputsMap>();

    let format = match OutputFormat::infer(output, format) {
        Ok(v) => v,
        Err(e) => {
            logger::end_action(Paint::red(e));
            return 1;
        }
    };

    let declaration_output =
        match serialize_roblox_ts_sidecar(&outputs_map, format, output, roblox_ts) {
            Ok(v) => v,
            Err(e) => {
                logger::end_action(Paint::red(e));
                return 1;
            }
        };

    let outputs_string = match serialize_outputs(&outputs_map, format) {
        Ok(v) => v,
        Err(e) => {
            logger::end_action(Paint::red(format!("Failed to serialize outputs: {}", e)));
            return 1;
        }
    };
    logger::end_action("Succeeded");

    if let Some(output) = output {
        if let Err(e) = fs::write(output, outputs_string)
            .map_err(|e| format!("Unable to write outputs file: {}\n\t{}", output, e))
        {
            logger::log(Paint::red(e));
            return 1;
        }

        if let Some((declaration_path, declaration_string)) = declaration_output {
            if let Err(e) = fs::write(&declaration_path, declaration_string).map_err(|e| {
                format!(
                    "Unable to write roblox-ts declaration file: {}\n\t{}",
                    declaration_path, e
                )
            }) {
                logger::log(Paint::red(e));
                return 1;
            }
        }
    } else {
        print!("{}", outputs_string);
    }

    0
}

fn serialize_outputs(outputs_map: &OutputsMap, format: OutputFormat) -> Result<String, String> {
    match format {
        OutputFormat::Json => serde_json::to_string_pretty(outputs_map)
            .map(|value| value + "\n")
            .map_err(|e| e.to_string()),
        OutputFormat::Yaml => serde_yaml::to_string(outputs_map).map_err(|e| e.to_string()),
        OutputFormat::Luau => outputs_to_value(outputs_map).map(|value| render_luau_module(&value)),
    }
}

fn serialize_roblox_ts_sidecar(
    outputs_map: &OutputsMap,
    format: OutputFormat,
    output: Option<&str>,
    roblox_ts: bool,
) -> Result<Option<(String, String)>, String> {
    if !roblox_ts {
        return Ok(None);
    }

    if format != OutputFormat::Luau {
        return Err("The --roblox-ts flag is only supported with Luau outputs.".to_owned());
    }

    let output = output.ok_or_else(|| {
        "The --roblox-ts flag requires --output so Lithos can write a .d.ts sidecar.".to_owned()
    })?;
    let declaration_path = roblox_ts_declaration_path(output)?;
    let declaration_string =
        outputs_to_value(outputs_map).map(|value| render_dts_module(&value))?;

    Ok(Some((declaration_path, declaration_string)))
}

fn roblox_ts_declaration_path(output: &str) -> Result<String, String> {
    let output_path = Path::new(output);
    match output_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("lua") | Some("luau") => Ok(output_path
            .with_extension("d.ts")
            .to_string_lossy()
            .into_owned()),
        _ => Err(
            "The --roblox-ts flag requires the Luau output file to end in .lua or .luau."
                .to_owned(),
        ),
    }
}

fn outputs_to_value(outputs_map: &OutputsMap) -> Result<Value, String> {
    serde_json::to_value(outputs_map).map_err(|e| e.to_string())
}

fn render_luau_module(value: &Value) -> String {
    format!("return {}\n", render_luau_value(value, 0))
}

fn render_luau_value(value: &Value, indent: usize) -> String {
    match value {
        Value::Null => "nil".to_owned(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => render_luau_string(value),
        Value::Array(values) => render_luau_array(values, indent),
        Value::Object(values) => render_luau_table(values, indent),
    }
}

fn render_luau_table(values: &Map<String, Value>, indent: usize) -> String {
    if values.is_empty() {
        return "{}".to_owned();
    }

    let next_indent = indent + 1;
    let mut entries = String::new();
    for (key, value) in sorted_object_entries(values) {
        let _ = writeln!(
            entries,
            "{}{} = {},",
            indent_str(next_indent),
            render_luau_key(key),
            render_luau_value(value, next_indent)
        );
    }

    format!("{{\n{}{}}}", entries, indent_str(indent))
}

fn render_luau_array(values: &[Value], indent: usize) -> String {
    if values.is_empty() {
        return "{}".to_owned();
    }

    let next_indent = indent + 1;
    let mut entries = String::new();
    for (index, value) in values.iter().enumerate() {
        let _ = writeln!(
            entries,
            "{}[{}] = {},",
            indent_str(next_indent),
            index + 1,
            render_luau_value(value, next_indent)
        );
    }

    format!("{{\n{}{}}}", entries, indent_str(indent))
}

fn render_luau_key(key: &str) -> String {
    if is_valid_luau_identifier(key) {
        key.to_owned()
    } else {
        format!("[{}]", render_luau_string(key))
    }
}

fn render_luau_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');

    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => escaped.push_str(&format!("\\{:03}", ch as u32)),
            ch => escaped.push(ch),
        }
    }

    escaped.push('"');
    escaped
}

fn render_ts_string(value: &str) -> String {
    serde_json::to_string(value).expect("serializing string literals should succeed")
}

fn render_dts_module(value: &Value) -> String {
    format!(
        "declare const outputs: {};\n\nexport = outputs;\n",
        render_dts_type(value, 0)
    )
}

fn render_dts_type(value: &Value, indent: usize) -> String {
    match value {
        Value::Null => "undefined".to_owned(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => render_ts_string(value),
        Value::Array(values) => render_dts_tuple(values, indent),
        Value::Object(values) => render_dts_object(values, indent),
    }
}

fn render_dts_object(values: &Map<String, Value>, indent: usize) -> String {
    if values.is_empty() {
        return "{}".to_owned();
    }

    let next_indent = indent + 1;
    let mut entries = String::new();
    for (key, value) in sorted_object_entries(values) {
        let _ = writeln!(
            entries,
            "{}readonly {}: {};",
            indent_str(next_indent),
            render_dts_key(key),
            render_dts_type(value, next_indent)
        );
    }

    format!("{{\n{}{}}}", entries, indent_str(indent))
}

fn render_dts_tuple(values: &[Value], indent: usize) -> String {
    if values.is_empty() {
        return "readonly []".to_owned();
    }

    let next_indent = indent + 1;
    let mut entries = String::new();
    for value in values {
        let _ = writeln!(
            entries,
            "{}{},",
            indent_str(next_indent),
            render_dts_type(value, next_indent)
        );
    }

    format!("readonly [\n{}{}]", entries, indent_str(indent))
}

fn render_dts_key(key: &str) -> String {
    if is_valid_ts_identifier(key) {
        key.to_owned()
    } else {
        render_ts_string(key)
    }
}

fn sorted_object_entries(values: &Map<String, Value>) -> Vec<(&str, &Value)> {
    let mut entries = values
        .iter()
        .map(|(key, value)| (key.as_str(), value))
        .collect::<Vec<_>>();
    entries.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
    entries
}

fn indent_str(indent: usize) -> String {
    "    ".repeat(indent)
}

fn is_valid_luau_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    if !(first == '_' || first.is_ascii_alphabetic()) || is_luau_keyword(value) {
        return false;
    }

    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn is_luau_keyword(value: &str) -> bool {
    matches!(
        value,
        "and"
            | "break"
            | "do"
            | "else"
            | "elseif"
            | "end"
            | "false"
            | "for"
            | "function"
            | "if"
            | "in"
            | "local"
            | "nil"
            | "not"
            | "or"
            | "repeat"
            | "return"
            | "then"
            | "true"
            | "until"
            | "while"
    )
}

fn is_valid_ts_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    if !(first == '_' || first == '$' || first.is_ascii_alphabetic()) {
        return false;
    }

    chars.all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use rbx_lithos::roblox_resource_manager::{ExperienceOutputs, RobloxOutputs};

    fn sample_outputs() -> OutputsMap {
        BTreeMap::from([(
            "experience_singleton".to_owned(),
            Some(RobloxOutputs::Experience(ExperienceOutputs {
                asset_id: 3_296_599_132,
                start_place_id: 8_667_346_609,
            })),
        )])
    }

    #[test]
    fn infer_format_from_output_extension() {
        assert_eq!(OutputFormat::infer(None, None).unwrap(), OutputFormat::Json);
        assert_eq!(
            OutputFormat::infer(Some("generated/Outputs.luau"), None).unwrap(),
            OutputFormat::Luau
        );
        assert_eq!(
            OutputFormat::infer(Some("generated/Outputs.lua"), None).unwrap(),
            OutputFormat::Luau
        );
        assert_eq!(
            OutputFormat::infer(Some("generated/outputs.yml"), None).unwrap(),
            OutputFormat::Json
        );
        assert_eq!(
            OutputFormat::infer(Some("generated/Outputs.luau"), Some("json")).unwrap(),
            OutputFormat::Json
        );
    }

    #[test]
    fn render_luau_module_matches_serialized_shape() {
        let value = outputs_to_value(&sample_outputs()).unwrap();
        let variant = value["experience_singleton"]
            .as_object()
            .and_then(|resource| resource.keys().next())
            .unwrap();

        assert_eq!(
            render_luau_module(&value),
            format!(
                "return {{\n    experience_singleton = {{\n        {} = {{\n            assetId = 3296599132,\n            startPlaceId = 8667346609,\n        }},\n    }},\n}}\n",
                variant
            )
        );
    }

    #[test]
    fn render_dts_module_quotes_invalid_keys() {
        let value = json!({
            "badge-icon": {
                "end": true,
                "display name": "Badge",
            },
            "optionalValue": null,
        });

        let dts_module = render_dts_module(&value);
        assert!(dts_module.contains("readonly \"badge-icon\": {"));
        assert!(dts_module.contains("readonly \"display name\": \"Badge\";"));
        assert!(dts_module.contains("readonly end: true;"));
        assert!(dts_module.contains("readonly optionalValue: undefined;"));
        assert!(dts_module.ends_with("export = outputs;\n"));
    }

    #[test]
    fn roblox_ts_sidecar_requires_luau_output_path() {
        assert_eq!(
            roblox_ts_declaration_path("generated/outputs.luau").unwrap(),
            "generated/outputs.d.ts"
        );
        assert_eq!(
            roblox_ts_declaration_path("generated/outputs.lua").unwrap(),
            "generated/outputs.d.ts"
        );
        assert!(roblox_ts_declaration_path("generated/outputs.json").is_err());
    }

    #[test]
    fn roblox_ts_sidecar_rejects_non_luau_modes() {
        let error = serialize_roblox_ts_sidecar(&sample_outputs(), OutputFormat::Json, None, true)
            .unwrap_err();
        assert_eq!(
            error,
            "The --roblox-ts flag is only supported with Luau outputs."
        );
    }

    #[test]
    fn render_luau_quotes_non_identifiers_and_keywords() {
        let value = json!({
            "badge-icon": {
                "end": true,
            },
        });

        let luau_module = render_luau_module(&value);
        assert!(luau_module.contains("[\"badge-icon\"] = {"));
        assert!(luau_module.contains("[\"end\"] = true,"));
    }
}
