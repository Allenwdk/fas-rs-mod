// Copyright 2023 shadow3aaa@gitbub.com
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::time::Duration;

use anyhow::Result;

use super::{Event, Insider};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Fas,
    Normal,
}

impl Insider {
    pub fn event_loop(mut self) -> Result<()> {
        loop {
            let usage = self.current_usage_max()?;
            let target_freq_usage_based = self.usage_policy(usage)?;

            if self.always_userspace_governor() {
                let _ = self.set_userspace_governor_freq(target_freq_usage_based);
            }

            if let Some(event) = self.recv_event() {
                let _ = match event {
                    Event::InitDefault(b) => self.init_default(b),
                    Event::InitGame => self.init_game(),
                    Event::SetFasFreq(f) => self.set_fas_freq(f),
                };
            }
        }
    }

    fn recv_event(&self) -> Option<Event> {
        if self.always_userspace_governor() {
            self.rx.recv_timeout(Duration::from_millis(25)).ok()
        } else {
            self.rx.recv().ok()
        }
    }
}
