use midly::Smf;

use crate::{
    model::{self, MetaEvent, MidiEvent, PlayerResult, PlayerTimingInfo, TimeInfo},
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
        let mut e = Self {
            emit_meta,
            emit_delta_times: delta_times,
            events: TrackMode::from_smf(smf),
            timing: Default::default(),
        };

        match timing {
            midly::Timing::Metrical(mt) => e.timing.set_ticks_per_beat(mt.as_int() as u64),
            midly::Timing::Timecode(fps, tpf) => e.timing.set_timecode(fps, tpf.into()),
        }

        e
    }

    pub fn make_time_info(&mut self, delta: u64) -> TimeInfo {
        let tinfo = self.timing.next_tick(delta);

        if self.emit_delta_times {
            TimeInfo {
                tick:    tinfo.delta_tick,
                micros:  tinfo.delta_micros,
                seconds: tinfo.delta_micros as f64 / 1_000_000.0f64,
            }
        } else {
            TimeInfo {
                tick:    tinfo.abs_tick,
                micros:  tinfo.abs_micros,
                seconds: tinfo.abs_micros as f64 / 1_000_000.0f64,
            }
        }
    }

    pub fn next_event(&mut self) -> Option<PlayerResult<model::Event>> {
        if let Some(event) = self.events.next() {
            let time = self.make_time_info(event.real_delta as u64);
            return match event.event.kind {
                midly::TrackEventKind::Midi { channel, message } => Some(
                    self.handle_event(channel.as_int(), message)
                        .map(|v| model::Event::Midi { time, data: v }),
                ),
                // stop at first end-of-track
                midly::TrackEventKind::Meta(midly::MetaMessage::EndOfTrack) => None,
                midly::TrackEventKind::Meta(meta) => Some(
                    self.handle_meta(meta)
                        .map(|v| model::Event::Meta { time, data: v }),
                ),
                _ => Some(PlayerResult::Ignored), // systex and escape messages are ignored
            };
        }
        None
    }

    fn handle_event(
        &mut self,
        channel: u8,
        event: midly::MidiMessage,
    ) -> PlayerResult<model::MidiEvent> {
        let v = match event {
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
            midly::MidiMessage::PitchBend { bend } => MidiEvent::PitchBend {
                chan:    channel,
                bend_by: bend.as_int(),
            },
        };

        PlayerResult::Event(v)
    }

    fn handle_meta(&mut self, meta: midly::MetaMessage) -> PlayerResult<model::MetaEvent> {
        let v = match meta {
            midly::MetaMessage::TrackNumber(tn) => MetaEvent::TrackNumber(tn),
            midly::MetaMessage::Text(tx) => MetaEvent::Text(Vec::from(tx)),
            midly::MetaMessage::Copyright(cp_text) => MetaEvent::Copyright(Vec::from(cp_text)),
            midly::MetaMessage::TrackName(tn) => MetaEvent::TrackName(Vec::from(tn)),
            midly::MetaMessage::InstrumentName(iname) => {
                MetaEvent::InstrumentName(Vec::from(iname))
            },
            midly::MetaMessage::Lyric(lyric) => MetaEvent::Lyric(Vec::from(lyric)),
            midly::MetaMessage::Marker(marker) => MetaEvent::Marker(Vec::from(marker)),
            midly::MetaMessage::CuePoint(cue_point) => MetaEvent::CuePoint(Vec::from(cue_point)),
            midly::MetaMessage::ProgramName(program_name) => {
                MetaEvent::ProgramName(Vec::from(program_name))
            },
            midly::MetaMessage::DeviceName(device_name) => {
                MetaEvent::DeviceName(Vec::from(device_name))
            },
            midly::MetaMessage::MidiChannel(mchan) => MetaEvent::MidiChannel(mchan.as_int()),
            midly::MetaMessage::MidiPort(mprt) => MetaEvent::MidiPort(mprt.as_int()),
            midly::MetaMessage::EndOfTrack => MetaEvent::EndOfTrack,
            midly::MetaMessage::Tempo(tpb) => {
                let v = tpb.as_int();
                self.timing.set_micros_per_qn(v as u64);
                MetaEvent::Tempo(v)
            },
            midly::MetaMessage::SmpteOffset(_) => return PlayerResult::Ignored,
            midly::MetaMessage::TimeSignature(n, d, cpt, n32q) => {
                MetaEvent::TimeSignature(n, d, cpt, n32q)
            },
            midly::MetaMessage::KeySignature(ksig, minor) => MetaEvent::KeySignature(ksig, minor),
            midly::MetaMessage::SequencerSpecific(_) => return PlayerResult::Ignored,
            midly::MetaMessage::Unknown(event, data) => MetaEvent::Unknown(event, Vec::from(data)),
        };

        if !self.emit_meta {
            return PlayerResult::Ignored;
        }
        PlayerResult::Event(v)
    }
}