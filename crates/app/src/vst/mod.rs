//! Installed VST3 plugins and what their parameters are called. Plugin code runs only
//! on the host's worker thread: loading one can take a while, and a slow one mustn't
//! stall the interface.

mod cache;
mod worker;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crossbeam::channel::{Receiver, Sender};
use plugins::{ParameterInfo, PluginInfo};

/// A plugin class, as an effect names it.
pub(crate) type PluginKey = (PathBuf, [u8; 16]);

pub(crate) enum Described {
    Params(Vec<ParameterInfo>),
    Failed(String),
}

enum Request {
    Scan,
    Describe(PluginKey),
    Show(PluginKey, u32, f32),
}

enum Reply {
    Installed(Vec<PluginInfo>),
    Described(PluginKey, Result<Vec<ParameterInfo>, String>),
    Shown(PluginKey, u32, f32, String),
}

pub(crate) struct PluginHost {
    pub(crate) installed: Vec<PluginInfo>,
    pub(crate) scanning: bool,
    requests: Sender<Request>,
    replies: Receiver<Reply>,
    described: HashMap<PluginKey, Described>,
    /// The plugin's text for each parameter's value, and the value it was asked about.
    shown: HashMap<(PluginKey, u32), (f32, String)>,
    asked: HashSet<(PluginKey, u32)>,
}

impl PluginHost {
    /// Starts the worker, which reads the list of plugins found by the last scan.
    pub(crate) fn start() -> Self {
        let (requests, replies) = worker::spawn();
        Self {
            installed: Vec::new(),
            scanning: false,
            requests,
            replies,
            described: HashMap::new(),
            shown: HashMap::new(),
            asked: HashSet::new(),
        }
    }

    /// Looks through the standard VST3 folders again, loading each plugin found.
    pub(crate) fn scan(&mut self) {
        self.scanning = self.requests.send(Request::Scan).is_ok();
    }

    /// The parameters of `key`, asked of the worker the first time.
    pub(crate) fn describe(&mut self, key: &PluginKey) -> Option<&Described> {
        if !self.described.contains_key(key) && self.asked.insert((key.clone(), u32::MAX)) {
            let _ = self.requests.send(Request::Describe(key.clone()));
        }
        self.described.get(key)
    }

    /// How the plugin writes `value` of parameter `id`, once the worker has said; until
    /// then the last text it gave.
    pub(crate) fn shown(&mut self, key: &PluginKey, id: u32, value: f32) -> Option<String> {
        let entry = (key.clone(), id);
        let current = self.shown.get(&entry);
        if current.is_none_or(|(shown, _)| *shown != value) && self.asked.insert(entry) {
            let _ = self.requests.send(Request::Show(key.clone(), id, value));
        }
        current.map(|(_, text)| text.clone())
    }

    /// Takes in what the worker has answered; whether anything changed.
    pub(crate) fn poll(&mut self) -> bool {
        let mut changed = false;
        for reply in self.replies.try_iter() {
            changed = true;
            match reply {
                Reply::Installed(installed) => {
                    self.installed = installed;
                    self.scanning = false;
                }
                Reply::Described(key, described) => {
                    self.asked.remove(&(key.clone(), u32::MAX));
                    let described = match described {
                        Ok(params) => Described::Params(params),
                        Err(error) => Described::Failed(error),
                    };
                    self.described.insert(key, described);
                }
                Reply::Shown(key, id, value, text) => {
                    let entry = (key, id);
                    self.asked.remove(&entry);
                    self.shown.insert(entry, (value, text));
                }
            }
        }
        changed
    }
}
