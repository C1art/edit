use super::*;

impl Edit for ImportXmeml {
    fn name(&self) -> &'static str {
        "Import XML"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let timebase = sequence.timebase;
        let ticks = |rate: FrameRate, frames: i64| rate.frame_start(frames, timebase);
        // The sound linked to a retimed picture plays at its speed.
        let link_retimes: HashMap<usize, Retime> = self
            .video
            .iter()
            .flatten()
            .filter_map(|clip| Some((clip.link?, clip.retime?)))
            .collect();
        let retime_of = |clip: &XmlClip| {
            clip.retime
                .or_else(|| link_retimes.get(&clip.link?).copied())
        };
        let used = |file: usize| {
            self.video
                .iter()
                .chain(&self.audio)
                .flatten()
                .filter(|clip| clip.file == file)
                .map(|clip| {
                    let length = ticks(self.rate, clip.end - clip.start);
                    let speed = retime_of(clip).map_or(Some(Speed::NORMAL), |retime| retime.speed);
                    ticks(clip.rate, clip.source_in)
                        + speed.map_or(Ticks(1), |speed| speed.source_span(length))
                })
                .max()
                .unwrap_or(Ticks(1))
        };
        let mut sources = Vec::new();
        for (index, file) in self.files.iter().enumerate() {
            let existing = sequence.media.iter().find(|item| {
                item.nest.is_none() && item.generator.is_none() && item.path == file.path
            });
            let id = match existing {
                Some(item) => item.id,
                None => {
                    let id = SourceId(sequence.ids.next());
                    let length = file
                        .frames
                        .map_or(Ticks::ZERO, |frames| ticks(file.rate, frames));
                    let (width, height) = file
                        .size
                        .unwrap_or((sequence.format.width, sequence.format.height));
                    Arc::make_mut(&mut sequence.media).push(MediaItem {
                        id,
                        name: file.name.clone(),
                        path: file.path.clone(),
                        duration: length.max(used(index)),
                        video: file.video.then_some(VideoFormat {
                            width,
                            height,
                            frame_rate: file.rate,
                            ..VideoFormat::default()
                        }),
                        has_audio: file.audio,
                        proxy: None,
                        generator: None,
                        nest: None,
                        bin: None,
                        subclip: None,
                        interpreted_rate: None,
                        interpreted_aspect: None,
                        interpreted_color: None,
                        interpreted_fields: None,
                        alpha: Default::default(),
                        log: Default::default(),
                        offline: false,
                        still: false,
                        channel_map: None,
                        placed: None,
                        frame_fit: None,
                        timecode: None,
                        poster: None,
                        transcript: None,
                    });
                    id
                }
            };
            sources.push(id);
        }
        let links: Vec<LinkId> = (0..self.links)
            .map(|_| LinkId(sequence.ids.next()))
            .collect();
        let transition =
            |(kind, frames): (TransitionKind, i64)| Transition::new(kind, ticks(self.rate, frames));
        let mut tracks = |clips: &[Vec<XmlClip>]| -> Vec<Track> {
            clips
                .iter()
                .map(|clips| {
                    let mut track = Track::default();
                    for xml in clips {
                        let start = ticks(self.rate, xml.start);
                        let mut clip = Clip::new(
                            ClipId(sequence.ids.next()),
                            sources[xml.file],
                            start,
                            ticks(self.rate, xml.end) - start,
                            ticks(xml.rate, xml.source_in),
                        );
                        if let Some(retime) = retime_of(xml) {
                            match retime.speed {
                                Some(speed) => clip.speed = speed,
                                None => clip.hold = true,
                            }
                            clip.reverse = retime.reverse;
                        }
                        clip.disabled = xml.disabled;
                        clip.gain = xml.gain;
                        clip.link = xml.link.map(|index| links[index]);
                        clip.transition_in =
                            xml.transition_in
                                .map(|(kind, frames, alignment)| Transition {
                                    alignment,
                                    ..transition((kind, frames))
                                });
                        clip.transition_out = xml.transition_out.map(transition);
                        clip.markers = xml
                            .markers
                            .iter()
                            .map(|marker| marker.marker(&mut sequence.ids, xml.rate, timebase))
                            .collect();
                        track.clips.push(clip.into());
                    }
                    track
                })
                .collect()
        };
        let (video, audio) = (tracks(&self.video), tracks(&self.audio));
        let markers = self
            .markers
            .iter()
            .map(|marker| marker.marker(&mut sequence.ids, self.rate, timebase))
            .collect();
        let nest = Nest {
            video,
            audio,
            markers,
            ..Nest::default()
        };
        let id = SourceId(sequence.ids.next());
        Arc::make_mut(&mut sequence.media).push(MediaItem {
            id,
            name: self.name.clone(),
            path: PathBuf::new(),
            duration: Ticks::ZERO,
            video: None,
            has_audio: false,
            proxy: None,
            generator: None,
            nest: Some(nest),
            bin: None,
            subclip: None,
            interpreted_rate: None,
            interpreted_aspect: None,
            interpreted_color: None,
            interpreted_fields: None,
            alpha: Default::default(),
            log: Default::default(),
            offline: false,
            still: false,
            channel_map: None,
            placed: None,
            frame_fit: None,
            timecode: None,
            poster: None,
            transcript: None,
        });
        sequence.fit_nest(id);
        Ok(())
    }
}
