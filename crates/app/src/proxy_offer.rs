//! Offers proxies for media too heavy to play smoothly, once an import finishes.

use egui::{Context, Key};
use timeline::{MediaItem, SourceId};

use crate::editor::Editor;

/// Pictures a second above 1080p60: 4K, or HD at high frame rates, which most laptops
/// can't decode in time for smooth playback.
const HEAVY_PIXEL_RATE: f64 = 1920.0 * 1080.0 * 60.0;
/// Names listed in the offer before the rest are counted.
const LISTED: usize = 5;

/// What to do when imported media is heavy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum HeavyMedia {
    #[default]
    Ask,
    CreateProxies,
    Ignore,
}

impl HeavyMedia {
    pub(crate) const ALL: [Self; 3] = [Self::Ask, Self::CreateProxies, Self::Ignore];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Ask => "Ask",
            Self::CreateProxies => "Always Create Proxies",
            Self::Ignore => "Never Create Proxies",
        }
    }

    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::CreateProxies => "create_proxies",
            Self::Ignore => "ignore",
        }
    }

    pub(crate) fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|choice| choice.key() == key)
    }
}

#[derive(Default)]
pub(crate) struct ProxyOffer {
    pub(crate) choice: HeavyMedia,
    /// Heavy media imported since the last offer, waiting for the import to finish so
    /// one offer covers the whole batch.
    pending: Vec<SourceId>,
    dont_ask_again: bool,
}

impl ProxyOffer {
    pub(crate) fn imported(&mut self, item: &MediaItem) {
        if self.choice != HeavyMedia::Ignore && is_heavy(item) {
            self.pending.push(item.id);
        }
    }
}

fn is_heavy(item: &MediaItem) -> bool {
    let Some(video) = &item.video else {
        return false;
    };
    if item.still || item.generator.is_some() || item.nest.is_some() || item.proxy.is_some() {
        return false;
    }
    let rate = item.interpreted_rate.unwrap_or(video.frame_rate);
    f64::from(video.width) * f64::from(video.height) * rate.approx_fps() > HEAVY_PIXEL_RATE
}

impl Editor {
    fn create_offered_proxies(&mut self, sources: Vec<SourceId>) {
        let Some(maker) = &mut self.proxies else {
            return;
        };
        let sequence = self.timeline.sequence();
        for source in sources {
            if let Some(item) = sequence.media_item(source) {
                maker.request(source, item.path.clone());
            }
        }
        if !self.use_proxies {
            self.use_proxies = true;
            self.notice("Proxies are on; Sequence > Use Proxies turns them off".to_owned());
        }
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &Context) {
    let offer = &mut editor.proxy_offer;
    let importing = editor
        .importer
        .as_ref()
        .is_some_and(|importer| importer.is_busy());
    if offer.pending.is_empty() || importing {
        return;
    }
    let sequence = editor.timeline.sequence();
    // Dropped meanwhile, or given a proxy another way.
    offer.pending.retain(|&source| {
        sequence
            .media_item(source)
            .is_some_and(|item| item.proxy.is_none())
    });
    let answer = match offer.choice {
        HeavyMedia::Ask => ask(offer, sequence, ctx),
        choice => Some(choice),
    };
    let Some(answer) = answer else {
        return;
    };
    let offer = &mut editor.proxy_offer;
    if std::mem::take(&mut offer.dont_ask_again) {
        offer.choice = answer;
    }
    let sources = std::mem::take(&mut offer.pending);
    if answer == HeavyMedia::CreateProxies {
        editor.create_offered_proxies(sources);
    }
}

/// Shows the offer, returning what was chosen once it's answered.
fn ask(offer: &mut ProxyOffer, sequence: &timeline::Sequence, ctx: &Context) -> Option<HeavyMedia> {
    let names: Vec<&str> = offer
        .pending
        .iter()
        .filter_map(|&source| sequence.media_item(source))
        .map(|item| item.name.as_str())
        .collect();
    let (count, what) = match names.len() {
        0 => return Some(HeavyMedia::Ignore),
        1 => ("This clip is".to_owned(), "it"),
        count => (format!("These {count} clips are"), "them"),
    };
    let mut answer = None;
    crate::dialog::window(ctx, "Create Proxies?").show(ctx, |ui| {
        ui.label(format!(
            "{count} 4K or high frame rate, and may stutter while you edit."
        ));
        for name in names.iter().take(LISTED) {
            ui.label(format!("  • {name}"));
        }
        if names.len() > LISTED {
            ui.weak(format!("  and {} more", names.len() - LISTED));
        }
        ui.weak(format!(
            "Proxies are smaller copies of {what} made in the background for smooth \
             playback. Export always uses the originals."
        ));
        ui.checkbox(&mut offer.dont_ask_again, "Don't ask again");
        crate::dialog::footer(ui, |ui| {
            if ui.button("Not Now").clicked() || ui.input(|input| input.key_pressed(Key::Escape)) {
                answer = Some(HeavyMedia::Ignore);
            }
            if ui.add(crate::dialog::primary("Create Proxies")).clicked()
                || ui.input(|input| input.key_pressed(Key::Enter))
            {
                answer = Some(HeavyMedia::CreateProxies);
            }
        });
    });
    answer
}
