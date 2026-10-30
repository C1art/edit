use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use audio::SampleRate;
use crossbeam::channel::{Receiver, Sender};
use playback::{MonoMix, Piece, PlaybackError};
use timeline::{Transcript, Word};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::words::{quietest_cut, words_of, Token};
use crate::SpeechError;

/// Whisper hears 16 kHz mono.
const RATE: usize = 16_000;
/// Samples per hundredth of a second, the unit Whisper times words in.
const PER_HUNDREDTH: usize = RATE / 100;
/// How much is recognized at once, to keep memory flat on long recordings.
const STRETCH: usize = RATE * 600;
/// How far back from a stretch's end to look for a quiet place to cut it.
const CUT_SEARCH: usize = RATE * 30;
/// How much is mixed between checks for cancelling.
const MIX_STEP: usize = RATE * 10;
/// Whisper makes up words over long silences; segments it thinks this likely to be
/// silence are dropped.
const NO_SPEECH: f32 = 0.6;

#[derive(Debug)]
pub enum TranscribeEvent {
    /// The fraction done so far.
    Progress(f32),
    /// Some media could not be read and was heard as silence.
    Warning(PlaybackError),
    Done(Transcript),
    Failed(SpeechError),
}

/// Recognizes the speech in a piece of a sequence on a worker thread. Dropping the job
/// cancels it.
pub struct TranscribeJob {
    events: Receiver<TranscribeEvent>,
    cancel: Arc<AtomicBool>,
}

impl TranscribeJob {
    /// Words are timed from the start of the piece. `language` is a code as "en", or
    /// `None` for the model to tell.
    pub fn start(
        piece: Piece,
        model: PathBuf,
        language: Option<String>,
    ) -> Result<Self, SpeechError> {
        let (sender, events) = crossbeam::channel::unbounded();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        thread::Builder::new()
            .name("transcribe".into())
            .spawn(move || {
                let event = match transcribe(piece, &model, language, &sender, &worker_cancel) {
                    Ok(Some(transcript)) => TranscribeEvent::Done(transcript),
                    Ok(None) => return,
                    Err(error) => TranscribeEvent::Failed(error),
                };
                let _ = sender.send(event);
            })
            .map_err(SpeechError::WorkerSpawn)?;
        Ok(Self { events, cancel })
    }

    /// Everything that happened since the last poll; never blocks.
    pub fn poll(&self) -> Vec<TranscribeEvent> {
        self.events.try_iter().collect()
    }
}

impl Drop for TranscribeJob {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// The transcript, or `None` when cancelled.
fn transcribe(
    piece: Piece,
    model: &std::path::Path,
    language: Option<String>,
    events: &Sender<TranscribeEvent>,
    cancel: &Arc<AtomicBool>,
) -> Result<Option<Transcript>, SpeechError> {
    let timebase = piece.0.timebase;
    let model = model
        .to_str()
        .ok_or_else(|| SpeechError::Model("the model's path isn't valid Unicode".into()))?;
    let context = WhisperContext::new_with_params(model, WhisperContextParameters::default())?;
    let mut state = context.create_state()?;
    let last_text_token = context.token_eot();
    let mut mix = MonoMix::new(piece, SampleRate(RATE as u32));
    let total = mix.len().max(1);
    let mut transcript = Transcript {
        language: language.clone().unwrap_or_default(),
        words: Vec::new(),
    };
    let mut pending: Vec<f32> = Vec::with_capacity(STRETCH + MIX_STEP);
    let mut done = 0;
    let mut warnings = Vec::new();
    loop {
        let mut at_end = false;
        while pending.len() < STRETCH && !at_end {
            if cancel.load(Ordering::Relaxed) {
                return Ok(None);
            }
            at_end = mix.read(MIX_STEP, &mut pending, &mut warnings) < MIX_STEP;
            for warning in warnings.drain(..) {
                let _ = events.send(TranscribeEvent::Warning(warning));
            }
        }
        let cut = if at_end {
            pending.len()
        } else {
            quietest_cut(&pending, CUT_SEARCH, RATE)
        };
        if cut >= PER_HUNDREDTH {
            let params = params(language.as_deref(), (done, cut, total), events, cancel);
            if let Err(error) = state.full(params, &pending[..cut]) {
                return if cancel.load(Ordering::Relaxed) {
                    Ok(None)
                } else {
                    Err(error.into())
                };
            }
            let offset = i64::try_from(done / PER_HUNDREDTH).unwrap_or(i64::MAX);
            transcript
                .words
                .extend(recognized(&state, last_text_token, offset, timebase));
            if transcript.language.is_empty() {
                let found = whisper_rs::get_lang_str(state.full_lang_id_from_state());
                transcript.language = found.unwrap_or_default().to_owned();
            }
        }
        pending.drain(..cut);
        done += cut;
        if at_end {
            break;
        }
    }
    Ok(Some(transcript))
}

fn params<'a>(
    language: Option<&'a str>,
    (done, stretch, total): (usize, usize, usize),
    events: &Sender<TranscribeEvent>,
    cancel: &Arc<AtomicBool>,
) -> FullParams<'a, 'a> {
    let mut params = FullParams::new(SamplingStrategy::BeamSearch {
        beam_size: 5,
        patience: -1.0,
    });
    let threads = thread::available_parallelism().map_or(4, |count| count.get().min(8));
    params.set_n_threads(i32::try_from(threads).unwrap_or(4));
    params.set_token_timestamps(true);
    params.set_suppress_nst(true);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_language(Some(language.unwrap_or("auto")));
    let progress = events.clone();
    params.set_progress_callback_safe(move |percent: i32| {
        let heard = done as f32 + stretch as f32 * percent as f32 / 100.0;
        let _ = progress.send(TranscribeEvent::Progress(heard / total as f32));
    });
    let cancel = Arc::clone(cancel);
    params.set_abort_callback_safe(move || cancel.load(Ordering::Relaxed));
    params
}

fn recognized(
    state: &whisper_rs::WhisperState,
    last_text_token: i32,
    offset: i64,
    timebase: timeline::Timebase,
) -> Vec<Word> {
    let segments = (0..state.full_n_segments())
        .filter_map(|index| state.get_segment(index))
        .filter(|segment| segment.no_speech_probability() < NO_SPEECH);
    let mut words = Vec::new();
    for segment in segments {
        let tokens = (0..segment.n_tokens())
            .filter_map(|index| segment.get_token(index))
            .filter(|token| token.token_id() < last_text_token)
            .filter_map(|token| {
                let data = token.token_data();
                Some(Token {
                    bytes: token.to_bytes().ok()?,
                    start: data.t0,
                    end: data.t1,
                })
            });
        words.extend(words_of(tokens, offset, timebase));
    }
    words
}
