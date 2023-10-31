/* Copyright 2023 shadow3aaa@gitbub.com
*
*  Licensed under the Apache License, Version 2.0 (the "License");
*  you may not use this file except in compliance with the License.
*  You may obtain a copy of the License at
*
*      http://www.apache.org/licenses/LICENSE-2.0
*
*  Unless required by applicable law or agreed to in writing, software
*  distributed under the License is distributed on an "AS IS" BASIS,
*  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
*  See the License for the specific language governing permissions and
*  limitations under the License. */
mod merge;
mod read;

use std::{fs, path::Path, sync::Arc, thread};

use likely_stable::LikelyOption;
use log::{error, info};
use parking_lot::RwLock;
use toml::Value;

use crate::{
    error::{Error, Result},
    node::Mode,
};

use read::wait_and_read;

type ConfData = RwLock<Value>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetFps {
    Value(u32),
    Array(Vec<u32>),
}

#[derive(Debug, Clone)]
pub struct Config {
    toml: Arc<ConfData>,
}

impl Config {
    pub fn new<P: AsRef<Path>>(p: P, sp: P) -> Result<Self> {
        let path = p.as_ref();
        let std_path = sp.as_ref();

        let ori = fs::read_to_string(path)?;

        let toml = toml::from_str(&ori)?;
        let toml = Arc::new(RwLock::new(toml));

        {
            let path = path.to_owned();
            let std_path = std_path.to_owned();
            let toml = toml.clone();

            thread::Builder::new()
                .name("ConfigThread".into())
                .spawn(move || wait_and_read(&path, &std_path, &toml))?;
        }

        info!("Config watcher started");

        Ok(Self { toml })
    }

    pub fn target_fps<S: AsRef<str>>(&self, pkg: S) -> Option<TargetFps> {
        let pkg = pkg.as_ref();
        let pkg = pkg.split(':').next()?;

        let toml = self.toml.read();
        let list = toml
            .get("game_list")
            .and_then_likely(Value::as_table)
            .cloned()
            .unwrap();
        let value = list.get(pkg)?.clone();

        drop(toml); // early-drop Rwlock

        match value {
            Value::Array(arr) => {
                let mut arr: Vec<_> = arr
                    .into_iter()
                    .filter_map(|v| v.as_integer())
                    .map(|i| i as u32)
                    .collect();
                arr.sort_unstable();
                Some(TargetFps::Array(arr))
            }
            Value::Integer(i) => Some(TargetFps::Value(i as u32)),
            Value::String(s) => {
                if s == "auto" {
                    Some(TargetFps::Array(vec![30, 45, 60, 90, 120, 144]))
                } else {
                    error!("Find target game {pkg} in config, but meet illegal data type");
                    error!("Sugg: try \'{pkg} = \"auto\"\'");
                    None
                }
            }
            _ => {
                error!("Find target game {pkg} in config, but meet illegal data type");
                error!("Sugg: try \'{pkg} = \"auto\"\'");
                None
            }
        }
    }

    pub fn get_mode_conf<S: AsRef<str>>(&self, m: Mode, l: S) -> Result<Value> {
        let label = l.as_ref();
        let mode = m.to_string();
        let toml = self.toml.read();

        toml.get(mode)
            .and_then_likely(|t| t.get(label).cloned())
            .ok_or(Error::ConfigValueNotFound)
    }

    pub fn get_conf<S: AsRef<str>>(&self, l: S) -> Result<Value> {
        let label = l.as_ref();
        let toml = self.toml.read();

        toml.get("config")
            .and_then_likely(|t| t.get(label).cloned())
            .ok_or(Error::ConfigValueNotFound)
    }
}
