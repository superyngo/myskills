// types.rs — pure data; no I/O, no business logic.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

fn true_fn() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    #[serde(default)]
    pub version: Option<u32>,
    pub tiers: Vec<Tier>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Tier {
    pub id: String,
    pub agents: Vec<Agent>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Agent {
    pub id: String,
    pub cli: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: Vec<EnvEntry>,
    #[serde(default)]
    pub template: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum EnvEntry {
    File { name: String, path: String },
    Env { name: String, var: String },
    Source { path: String },
}

#[non_exhaustive]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FileInputMode {
    #[serde(rename = "arg")]
    Arg,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Template {
    #[serde(default)]
    pub detect_binary: Option<String>,
    #[serde(default)]
    pub subcommand: Option<String>,
    #[serde(default)]
    pub prompt_flag: Option<String>,
    #[serde(default)]
    pub prompt_positional: bool,
    #[serde(default)]
    pub model_flag: Option<String>,
    #[serde(default)]
    pub extra_args: Vec<String>,
    #[serde(default)]
    pub version_flag: Option<String>,
    #[serde(default)]
    pub file_input_mode: Option<FileInputMode>,
    #[serde(default = "true_fn")]
    pub verified: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DetectInfo {
    pub path: Option<String>,
    pub version: Option<String>,
    pub callable: bool,
    pub verified: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_entry_file_roundtrip() {
        let entry = EnvEntry::File {
            name: "X".into(),
            path: "/p".into(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: EnvEntry = serde_json::from_str(&json).unwrap();
        match back {
            EnvEntry::File { name, path } => {
                assert_eq!(name, "X");
                assert_eq!(path, "/p");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn env_entry_unknown_type_rejected() {
        let result = serde_json::from_str::<EnvEntry>(r#"{"type":"unknown","name":"x"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn file_input_mode_arg_roundtrip() {
        let result = serde_json::from_str::<FileInputMode>(r#""arg""#);
        assert!(matches!(result, Ok(FileInputMode::Arg)));
    }

    #[test]
    fn file_input_mode_unknown_rejected() {
        let result = serde_json::from_str::<FileInputMode>(r#""stdin""#);
        assert!(result.is_err());
    }

    #[test]
    fn template_verified_defaults_true() {
        let toml_str = "[t]\nprompt_flag = \"-p\"\n";
        #[derive(Deserialize)]
        struct Wrapper {
            t: Template,
        }
        let w: Wrapper = toml::from_str(toml_str).unwrap();
        assert!(w.t.verified);
    }

    #[test]
    fn config_round_trip() {
        let config = Config {
            version: Some(1),
            tiers: vec![Tier {
                id: "tier1".into(),
                agents: vec![Agent {
                    id: "agent1".into(),
                    cli: "claude".into(),
                    model: Some("sonnet".into()),
                    args: vec!["--arg".into()],
                    env: vec![],
                    template: None,
                }],
            }],
        };
        let s = toml::to_string_pretty(&config).unwrap();
        let back: Config = toml::from_str(&s).unwrap();
        assert_eq!(back.version, config.version);
        assert_eq!(back.tiers.len(), 1);
        assert_eq!(back.tiers[0].agents[0].id, "agent1");
    }
}
