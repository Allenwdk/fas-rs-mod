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
use std::{
    cell::RefCell,
    cmp::Ordering,
    ffi::OsStr,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use anyhow::{Ok, Result};
use likely_stable::LikelyOption;

use super::Freq;
use crate::error::Error;

#[derive(Debug, PartialEq, Eq)]
pub struct Policy {
    pub little: bool,
    pub num: u8,
    pub path: PathBuf,
    pub freqs: Vec<Freq>,
    gov_snapshot: RefCell<Option<String>>,
    force_bound: Option<PathBuf>,
}

impl Ord for Policy {
    fn cmp(&self, other: &Self) -> Ordering {
        self.num.cmp(&other.num)
    }
}

impl PartialOrd for Policy {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Policy {
    pub fn new<P: AsRef<Path>>(p: P) -> Result<Self> {
        let p = p.as_ref();

        let mut freqs: Vec<Freq> = fs::read_to_string(p.join("scaling_available_frequencies"))?
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();

        freqs.sort_unstable();

        let bound_path = Path::new("/proc/cpudvfs/cpufreq_debug");
        let force_bound = if bound_path.exists() {
            Some(bound_path.to_path_buf())
        } else {
            None
        };

        Ok(Self {
            little: false,
            num: Self::parse_policy(p.file_name().and_then_likely(OsStr::to_str).unwrap())
                .ok_or(Error::Other("Failed to parse cpufreq policy num"))?,
            path: p.to_path_buf(),
            freqs,
            gov_snapshot: RefCell::new(None),
            force_bound,
        })
    }

    pub fn init_default(&self) -> Result<()> {
        if let Some(ref p) = self.force_bound {
            self.force_freq_bound(
                self.freqs.first().copied().unwrap(),
                self.freqs.last().copied().unwrap(),
                p,
            )?;
        }

        self.set_min_freq(self.freqs[0])?;
        self.set_max_freq(self.freqs.last().copied().unwrap())?;
        self.reset_gov()
    }

    pub fn init_game(&self) -> Result<()> {
        self.set_fas_gov()?;
        self.set_fas_freq(self.freqs.last().copied().unwrap())
    }

    pub fn set_fas_freq(&self, f: Freq) -> Result<()> {
        self.set_max_freq(f)?;

        self.set_min_freq(self.freqs[0])?;
        if let Some(ref p) = self.force_bound {
            self.force_freq_bound(self.freqs[0], f, p)?;
        }

        Ok(())
    }

    pub fn reset_gov(&self) -> Result<()> {
        let path = self.path.join("scaling_governor");

        if let Some(ref gov) = *self.gov_snapshot.borrow() {
            let _ = fs::set_permissions(&path, PermissionsExt::from_mode(0o644));
            fs::write(&path, gov)?;
            let _ = fs::set_permissions(&path, PermissionsExt::from_mode(0o444));
        }

        Ok(())
    }

    pub fn set_fas_gov(&self) -> Result<()> {
        let path = self.path.join("scaling_governor");

        if !self.little {
            let cur_gov = fs::read_to_string(&path)?;
            if cur_gov.trim() != "performance" {
                self.gov_snapshot.replace(Some(cur_gov));
            }

            let _ = fs::set_permissions(&path, PermissionsExt::from_mode(0o644));
            fs::write(&path, "performance")?;
            let _ = fs::set_permissions(&path, PermissionsExt::from_mode(0o444));
        }

        Ok(())
    }

    fn set_max_freq(&self, f: Freq) -> Result<()> {
        let path = self.path.join("scaling_max_freq");
        let _ = fs::set_permissions(&path, PermissionsExt::from_mode(0o644));
        fs::write(path, f.to_string())?;
        Ok(())
    }

    fn set_min_freq(&self, f: Freq) -> Result<()> {
        let path = self.path.join("scaling_min_freq");
        let _ = fs::set_permissions(&path, PermissionsExt::from_mode(0o644));
        fs::write(path, f.to_string())?;
        Ok(())
    }

    fn force_freq_bound<P: AsRef<Path>>(&self, l: Freq, r: Freq, p: P) -> Result<()> {
        let message = format!("{} {l} {r}", self.num);
        fs::write(p, message)?;
        Ok(())
    }

    pub fn parse_policy<S: AsRef<str>>(p: S) -> Option<u8> {
        let p = p.as_ref();
        p.replace("policy", "").trim().parse().ok()
    }
}
