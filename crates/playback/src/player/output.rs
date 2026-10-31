use std::sync::Arc;

use audio::AudioWriter;

use super::Player;
use crate::command::Command;
use crate::PlaybackError;

impl Player {
    /// Plays through `writer` from now on, such as a newly opened device after the last one
    /// went away, keeping the position and what's shown. The device's sample rate drives
    /// the clock, so the workers start over around it.
    pub fn set_output(&mut self, writer: Option<AudioWriter>) -> Result<(), PlaybackError> {
        let mut player = Player::new(Arc::clone(&self.sequence), writer)?;
        player.proxies = self.proxies;
        player.send_sequence(Arc::clone(&self.sequence));
        player.resolution = self.resolution;
        let _ = player
            .video_commands
            .send(Command::Resolution(self.resolution));
        player.previews = Arc::clone(&self.previews);
        let _ = player
            .video_commands
            .send(Command::Previews(Arc::clone(&self.previews)));
        player.looping = self.looping.clone();
        player.scrub_audio = self.scrub_audio;
        player.speed = self.speed;
        player.seek(self.position());
        if let Some(shuttle) = self.shuttle {
            player.shuttle(shuttle.rate());
        } else if self.playing {
            player.play();
            // Play In to Out and Play Around still end where they were going to.
            player.stop = self.stop;
        }
        *self = player;
        Ok(())
    }
}
