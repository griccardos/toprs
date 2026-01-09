use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::sorted::SortType;

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum Mode {
    Gui,
    Tui,
}

#[derive(Serialize, Deserialize)]
pub struct TuiConfig {
    pub sort_column: usize,
    pub sort_type: SortType,
    pub show_cpu_per_core: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub mode: Mode,
    pub tui: TuiConfig,
}

impl Config {
    pub fn load() -> Config {
        let path = get_home_config();
        if path.exists() {
            if let Ok(contents) = std::fs::read_to_string(path) {
                if let Ok(config) = toml::from_str::<Config>(&contents) {
                    return config;
                }
            }
        }

        //default
        Config {
            mode: Mode::Tui,
            tui: TuiConfig {
                sort_column: 0,
                sort_type: SortType::None,
                show_cpu_per_core: true,
            },
        }
    }
    pub fn save(&self) {
        let path = get_home_config();
        if let Ok(toml_str) = toml::to_string(self) {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(path, toml_str);
        }
    }
}

fn get_home_config() -> PathBuf {
    //home directory
    if let Some(mut dir) = dirs::home_dir() {
        dir.push(".config");
        dir.push("toprs");
        dir.push("config.toml");
        return dir;
    }
    //should not happen, but just in case
    PathBuf::from("config.toml")
}
