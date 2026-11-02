use crate::ui::{DisplaySnapshot, UiProvider};

use super::{Core, TrackId, VoiceId, consumers::Consumers};

impl Core {
    /// Select an existing track, one of its voices, and one of its modulation lanes.
    /// Invalid selections leave the current selection unchanged.
    pub fn select(&mut self, track: TrackId, voice: VoiceId, lane: usize) -> bool {
        let assigned = self
            .tracks
            .iter()
            .any(|candidate| candidate.id == track && candidate.voices.contains(&voice));
        let exists = self
            .voices
            .iter()
            .any(|candidate| candidate.id == voice && lane < candidate.modulations.len());
        if !assigned || !exists {
            return false;
        }
        let changed =
            (track, voice, lane) != (self.selected_track, self.selected_voice, self.selected_lane);
        self.ui_dirty |= changed;
        self.selected_track = track;
        self.selected_voice = voice;
        self.selected_lane = lane;
        if changed {
            self.sync_slider_pickup(true);
        }
        self.publish_ui();
        true
    }

    /// Resolve UI navigation to the first envelope on the track's first voice.
    pub(super) fn select_track(&mut self, id: TrackId) {
        let Some(track) = self.tracks.iter().find(|track| track.id == id) else {
            return;
        };
        let Some(&voice_id) = track.voices.first() else {
            return;
        };
        let Some(voice) = self.voices.iter().find(|voice| voice.id == voice_id) else {
            return;
        };
        let Some(lane) = voice
            .modulations
            .iter()
            .position(|consumer| matches!(consumer, Consumers::ModTrigger(_)))
        else {
            return;
        };
        self.select(track.id, voice_id, lane);
    }

    pub(super) fn publish_ui(&mut self) {
        if self.has_updates() {
            self.ui_writer.write(self.ui_snapshot());
        }
    }
}

impl UiProvider for Core {
    type Model = DisplaySnapshot;

    fn ui_snapshot(&self) -> Self::Model {
        let assigned = self.tracks.iter().any(|track| {
            track.id == self.selected_track && track.voices.contains(&self.selected_voice)
        });
        let model = if assigned {
            self.voices
                .iter()
                .find(|voice| voice.id == self.selected_voice)
                .and_then(|voice| voice.modulations.get(self.selected_lane))
                .and_then(UiProvider::ui_snapshot)
        } else {
            None
        };
        DisplaySnapshot {
            track: self.selected_track.0,
            voice: self.selected_voice.0,
            lane: self.selected_lane as u8,
            usb_connected: self.usb_connected,
            model,
        }
    }

    fn has_updates(&mut self) -> bool {
        // Acknowledge both page and global changes, including on page switches.
        let page_changed = self
            .voices
            .iter_mut()
            .find(|voice| voice.id == self.selected_voice)
            .and_then(|voice| voice.modulations.get_mut(self.selected_lane))
            .is_some_and(UiProvider::has_updates);
        core::mem::take(&mut self.ui_dirty) | page_changed
    }
}
