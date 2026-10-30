use std::collections::HashMap;

use crossbeam::channel::{unbounded, Receiver, Sender};
use plugins::{ClassId, ParamId, Plugin};

use super::{cache, PluginKey, Reply, Request};

/// Any rate will do for reading parameters; nothing is played through these instances.
const SAMPLE_RATE: u32 = 48_000;
const MAX_BLOCK: usize = 512;

/// Starts the thread that runs plugin code for the interface. If it can't start, the
/// requests go nowhere and no plugins are listed.
pub(super) fn spawn() -> (Sender<Request>, Receiver<Reply>) {
    let (requests, incoming) = unbounded();
    let (outgoing, replies) = unbounded();
    let _ = std::thread::Builder::new()
        .name("plugins".into())
        .spawn(move || serve(&incoming, &outgoing));
    (requests, replies)
}

fn serve(incoming: &Receiver<Request>, outgoing: &Sender<Reply>) {
    if outgoing.send(Reply::Installed(cache::read())).is_err() {
        return;
    }
    let mut loaded: HashMap<PluginKey, Result<Plugin, String>> = HashMap::new();
    for request in incoming {
        let reply = match request {
            Request::Scan => {
                let installed = plugins::scan(&plugins::default_folders());
                cache::write(&installed);
                Reply::Installed(installed)
            }
            Request::Describe(key) => {
                let described = match instance(&mut loaded, &key) {
                    Ok(plugin) => Ok(plugin
                        .parameters()
                        .into_iter()
                        .filter(|param| !param.flags.hidden() && !param.flags.read_only())
                        .collect()),
                    Err(error) => Err(error.clone()),
                };
                Reply::Described(key, described)
            }
            Request::Show(key, id, value) => {
                let text = instance(&mut loaded, &key)
                    .as_ref()
                    .ok()
                    .and_then(|plugin| plugin.display_value(ParamId(id), f64::from(value)).ok())
                    .unwrap_or_else(|| format!("{:.0}%", value * 100.0));
                Reply::Shown(key, id, value, text)
            }
        };
        if outgoing.send(reply).is_err() {
            return;
        }
    }
}

fn instance<'a>(
    loaded: &'a mut HashMap<PluginKey, Result<Plugin, String>>,
    key: &PluginKey,
) -> &'a Result<Plugin, String> {
    loaded.entry(key.clone()).or_insert_with(|| {
        Plugin::load(&key.0, ClassId(key.1), SAMPLE_RATE, MAX_BLOCK)
            .map_err(|error| error.to_string())
    })
}
