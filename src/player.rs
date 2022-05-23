use midly::Smf;

use crate::{
    model::{self, MetaEvent, MidiEvent, PlayerResult, PlayerTimingInfo, TimeInfo, CDTrackEvent},
    trackmode::TrackMode,
};

pub struct MidiPlayerIter<'data, 'smf>(MidiPlayer<'data, 'smf>);

impl<'data, 'smf> Iterator for MidiPlayerIter<'data, 'smf> {
    type Item = PlayerResult<model::Event>;

    fn next(&mut self) -> Option<Self::Item> { self.0.next_event() }
}

pub struct MidiPlayer<'data, 'smf> {
    emit_delta_times: bool,
    emit_meta:        bool,
    extra_delta:      u64,
    timing:           PlayerTimingInfo,
    events:           TrackMode<'data, 'smf>,
}

impl<'data, 'smf> IntoIterator for MidiPlayer<'data, 'smf> {
    type Item = <MidiPlayerIter<'data, 'smf> as Iterator>::Item;

    type IntoIter = MidiPlayerIter<'data, 'smf>;

    fn into_iter(self) -> Self::IntoIter { MidiPlayerIter(self) }
}

impl<'data, 'smf> MidiPlayer<'data, 'smf> {
    pub fn new(smf: &'data Smf<'smf>, emit_meta: bool, delta_times: bool) -> Self {
        let timing = smf.header.timing.clone();
        Self {
            emit_meta,
            emit_delta_times: delta_times,
            extra_delta: 0,
            events: TrackMode::from_smf(smf),
            timing: PlayerTimingInfo::from(timing),
        }
    }

    pub fn next_event(&mut self) -> Option<PlayerResult<model::Event>> {
        self.events.next().map(|event| self._next_event(event))
    }

    fn _next_event(&mut self, event: CDTrackEvent) -> PlayerResult<model::Event> {
        match event.event.kind {
            midly::TrackEventKind::Midi { channel, message } => self.handle_midi(channel.as_int(), message, event.real_delta as u64),
            midly::TrackEventKind::SysEx(data) => self.handle_sysex(data, event.real_delta as u64),
            midly::TrackEventKind::Escape(data) => self.handle_escape(data, event.real_delta as u64),
            midly::TrackEventKind::Meta(message) => self.handle_meta(message, event.real_delta as u64),
        }
    }

    pub fn make_time_info(&mut self, delta: u64) -> TimeInfo {
        macro_rules! micros_to_secs {
           ($e:expr) => {{
               let __value = ($e as f64 / crate::model::MICROS_PER_SECOND as f64);
               (if (__value.fract() >= 0.5) { __value.ceil() } else { __value.floor() }) as f32
           }};
        }

        let time_info = self.timing.next_tick(self.extra_delta + delta);
        self.extra_delta = 0;

        if self.emit_delta_times {
            TimeInfo {
                tick: time_info.delta_tick,
                micros: time_info.delta_micros as u64,
                seconds: micros_to_secs!(time_info.delta_micros)
            }
        } else {
            TimeInfo {
                tick: time_info.abs_tick,
                micros: time_info.abs_micros as u64,
                seconds: micros_to_secs!(time_info.abs_micros)
            }
        }
    }

    fn handle_midi(&mut self, channel: u8, message: midly::MidiMessage, delta: u64) -> PlayerResult<model::Event> {
        let converted_msg = match message {
            midly::MidiMessage::NoteOff { key, vel } => MidiEvent::NoteOff {
                chan:     channel,
                note:     key.as_int(),
                velocity: vel.as_int(),
            },
            midly::MidiMessage::NoteOn { key, vel } => MidiEvent::NoteOn {
                chan:     channel,
                note:     key.as_int(),
                velocity: vel.as_int(),
            },
            midly::MidiMessage::Aftertouch { key, vel } => MidiEvent::Aftertouch {
                chan:     channel,
                note:     key.as_int(),
                velocity: vel.as_int(),
            },
            midly::MidiMessage::Controller { controller, value } => MidiEvent::Controller {
                chan:  channel,
                ctrl:  controller.as_int(),
                value: value.as_int(),
            },
            midly::MidiMessage::ProgramChange { program } => MidiEvent::ProgramChange {
                chan:    channel,
                program: program.as_int(),
            },
            midly::MidiMessage::ChannelAftertouch { vel } => MidiEvent::ChannelAftertouch {
                chan:     channel,
                velocity: vel.as_int(),
            },
            midly::MidiMessage::PitchBend {
                bend: midly::PitchBend(bend),
            } => MidiEvent::PitchBend {
                chan:    channel,
                bend_by: bend.as_int(),
            },
        };

        let time = self.make_time_info(delta);

        PlayerResult::Event(model::Event::Midi { time, data: converted_msg })
    }
    fn handle_meta(&mut self, message: midly::MetaMessage, delta: u64) -> PlayerResult<model::Event> {
        let parsed = match message {
            // normal meta messages, only emitted when emit_meta
            midly::MetaMessage::TrackNumber(tn) if self.emit_meta => Some(MetaEvent::TrackNumber(tn)),
            midly::MetaMessage::Text(tx) if self.emit_meta  => Some(MetaEvent::Text(Vec::from(tx))),
            midly::MetaMessage::Copyright(cp_text) if self.emit_meta  => Some(MetaEvent::Copyright(Vec::from(cp_text))),
            midly::MetaMessage::TrackName(tn) if self.emit_meta  => Some(MetaEvent::TrackName(Vec::from(tn))),
            midly::MetaMessage::InstrumentName(iname) => {
                Some(MetaEvent::InstrumentName(Vec::from(iname)))
            },
            midly::MetaMessage::Lyric(lyric) if self.emit_meta  => Some(MetaEvent::Lyric(Vec::from(lyric))),
            midly::MetaMessage::Marker(marker) if self.emit_meta  => Some(MetaEvent::Marker(Vec::from(marker))),
            midly::MetaMessage::CuePoint(cue_point) if self.emit_meta  => Some(MetaEvent::CuePoint(Vec::from(cue_point))),
            midly::MetaMessage::ProgramName(program_name) if self.emit_meta  => {
                Some(MetaEvent::ProgramName(Vec::from(program_name)))
            },
            midly::MetaMessage::DeviceName(device_name) if self.emit_meta  => {
                Some(MetaEvent::DeviceName(Vec::from(device_name)))
            },
            midly::MetaMessage::MidiChannel(mchan) if self.emit_meta  => Some(MetaEvent::MidiChannel(mchan.as_int())),
            midly::MetaMessage::MidiPort(mprt) if self.emit_meta  => Some(MetaEvent::MidiPort(mprt.as_int())),
            midly::MetaMessage::EndOfTrack if self.emit_meta => Some(MetaEvent::EndOfTrack),
            midly::MetaMessage::TimeSignature(n, d, cpt, n32q) if self.emit_meta => {
                Some(MetaEvent::TimeSignature(n, d, cpt, n32q))
            },
            midly::MetaMessage::KeySignature(ksig, minor) if self.emit_meta => Some(MetaEvent::KeySignature(ksig, minor)),
            midly::MetaMessage::Unknown(event, data) if self.emit_meta => Some(MetaEvent::Unknown(event, Vec::from(data))),

            // explicitly ignored meta messages
            midly::MetaMessage::SmpteOffset(_) => None,
            midly::MetaMessage::SequencerSpecific(_) => None,

            // tempo
            midly::MetaMessage::Tempo(tpb) => {
                // resets extra_delta and adds current delta
                let time = self.make_time_info(delta);
                self.timing.update_mpt(tpb.as_int());
                if self.emit_meta {
                    return PlayerResult::Event(model::Event::Meta { time, data: MetaEvent::Tempo(tpb.as_int()) });
                } else {
                    return PlayerResult::Ignored;
                }
            },

            // all remaining messages when !self.emit_meta
            _ => None
        };

        match parsed {
            None => {
                self.extra_delta += delta;
                PlayerResult::Ignored
            },
            Some(event) => {
                let time = self.make_time_info(delta);
                PlayerResult::Event(model::Event::Meta { time, data: event })
            }
        }
    }
    fn handle_escape(&mut self, _data: &[u8], delta: u64) -> PlayerResult<model::Event> {
        self.extra_delta += delta;
        PlayerResult::Ignored
    }
    fn handle_sysex(&mut self, _data: &[u8], delta: u64) -> PlayerResult<model::Event> {
        self.extra_delta += delta;
        PlayerResult::Ignored
    }

    // pub fn next_event(&mut self) -> Option<PlayerResult<model::Event>> {
    //     if let Some(event) = self.events.next() {
    //         return match event.event.kind {
    //             midly::TrackEventKind::Meta(midly::MetaMessage::EndOfTrack) => {
    //                 // EndOfTrack messages are ignored
    //                 self.extra_delta += event.real_delta as u64;
    //                 Some(PlayerResult::Ignored)
    //             },
    //             midly::TrackEventKind::Meta(meta) => match self.handle_meta(event.real_delta as u64, meta) {
    //                 PlayerResult::Event(mevent) => {
    //                     let time = self.make_time_info(event.real_delta as u64);
    //                     Some(PlayerResult::Event(model::Event::Meta {
    //                         time,
    //                         data: mevent,
    //                     }))
    //                 },
    //                 PlayerResult::Ignored => {
    //                     self.extra_delta += event.real_delta as u64;
    //                     Some(PlayerResult::Ignored)
    //                 },
    //             },
    //             _ => {
    //                 // systex and escape messages are ignored
    //                 self.extra_delta += event.real_delta as u64;
    //                 Some(PlayerResult::Ignored)
    //             },
    //         };
    //     }
    //     None
    // }
}
