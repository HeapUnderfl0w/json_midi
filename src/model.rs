use std::marker::PhantomData;

use itertools::Itertools;
use midly::{Smf, TrackEvent};

#[derive(Debug, serde::Serialize)]
pub struct Track {
    pub generated:        String,
    pub source_file:      String,
    pub events_processed: usize,
    pub events_emitted:   usize,
    pub emitted_meta:     bool,
    pub events:           Vec<Event>,
}

pub struct TrackMode<'data, 'smf> {
    event_index: usize,
    it:          Box<dyn Iterator<Item = CDTrackEvent<'smf>> + 'data>,
}

impl<'data, 'smf> TrackMode<'data, 'smf> {
    pub fn from_smf(smf: &'data Smf<'smf>) -> Self {
        let iter: Box<dyn Iterator<Item = CDTrackEvent<'smf>> + 'data> = match smf.header.format {
            midly::Format::SingleTrack => Box::new(smf.tracks[0].iter().map(|el| CDTrackEvent {
                real_delta: el.delta.as_int() as usize,
                event:      *el,
            })),
            midly::Format::Parallel => Box::new(
                smf.tracks
                    .iter()
                    .map(|e| {
                        let mut ioff = 0usize;
                        e.iter().map(move |event| {
                            ioff += event.delta.as_int() as usize;
                            SortableTrackEvent {
                                absolute_tick: ioff,
                                tevent:        *event,
                                _p:            &PhantomData,
                            }
                        })
                    })
                    .kmerge_by(|l, r| l < r)
                    .repeat_first_n(1)
                    .tuple_windows()
                    .map(|(left, right)| CDTrackEvent {
                        real_delta: right.absolute_tick - left.absolute_tick,
                        event:      right.tevent,
                    }),
            ),
            midly::Format::Sequential => {
                Box::new(smf.tracks.iter().flatten().map(|el| CDTrackEvent {
                    real_delta: el.delta.as_int() as usize,
                    event:      *el,
                }))
            },
        };

        Self {
            it:          iter,
            event_index: 0,
        }
    }
}

impl<'data, 'smf> Iterator for TrackMode<'data, 'smf> {
    type Item = CDTrackEvent<'smf>;

    fn size_hint(&self) -> (usize, Option<usize>) { self.it.size_hint() }

    fn next(&mut self) -> Option<Self::Item> {
        self.event_index += 1;
        self.it.next()
    }
}

/// Event proxy containing an extra delta field that contains the correct delta
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct CDTrackEvent<'smf> {
    pub real_delta: usize,
    pub event:      TrackEvent<'smf>,
}

#[derive(Debug, PartialEq, Clone, Copy)]
struct SortableTrackEvent<'smf> {
    pub absolute_tick: usize,
    pub tevent:        TrackEvent<'smf>,
    _p:                &'smf PhantomData<Self>,
}

impl<'smf> PartialOrd for SortableTrackEvent<'smf> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // for sorting we *only* care about the absolute tick. the sorting *has* to be
        // stable
        self.absolute_tick.partial_cmp(&other.absolute_tick)
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    Midi { time: TimeInfo, data: MidiEvent },
    Meta { time: TimeInfo, data: MetaEvent },
}

#[derive(Debug, serde::Serialize)]
pub struct TimeInfo {
    pub tick:    u64,
    pub micros:  u64,
    pub seconds: f64,

}

#[derive(Debug, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MidiEvent {
    NoteOff {
        chan:     u8,
        note:     u8,
        velocity: u8,
    },
    NoteOn {
        chan:     u8,
        note:     u8,
        velocity: u8,
    },
    Aftertouch {
        chan:     u8,
        note:     u8,
        velocity: u8,
    },
    Controller {
        chan:  u8,
        ctrl:  u8,
        value: u8,
    },
    ProgramChange {
        chan:    u8,
        program: u8,
    },
    ChannelAftertouch {
        chan:     u8,
        velocity: u8,
    },
    PitchBend {
        chan:    u8,
        bend_by: i16,
    },
}

#[derive(Debug, serde::Serialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum MetaEvent {
    TrackNumber(Option<u16>),
    Text(Vec<u8>),
    Copyright(Vec<u8>),
    TrackName(Vec<u8>),
    InstrumentName(Vec<u8>),
    Lyric(Vec<u8>),
    Marker(Vec<u8>),
    CuePoint(Vec<u8>),
    ProgramName(Vec<u8>),
    DeviceName(Vec<u8>),
    MidiChannel(u8),
    MidiPort(u8),
    EndOfTrack,
    Tempo(u32),
    TimeSignature(u8, u8, u8, u8),
    KeySignature(i8, bool),
    Unknown(u8, Vec<u8>),
}

/// Repeat the first element N times. For use with tools like
/// `itertools::Iterator`
struct RepeatFirstN<I>
where
    I: Iterator,
    <I as Iterator>::Item: Clone,
{
    it: I,
    e:  Option<<I as Iterator>::Item>,
    c:  usize,
    n:  usize,
}

impl<I> Iterator for RepeatFirstN<I>
where
    I: Iterator,
    I::Item: Clone,
{
    type Item = <I as Iterator>::Item;

    fn size_hint(&self) -> (usize, Option<usize>) { self.it.size_hint() }

    fn next(&mut self) -> Option<Self::Item> {
        if self.c <= self.n {
            self.c += 1;
            return self.e.clone();
        }

        self.it.next()
    }
}

trait RepeatFirst
where
    Self: Sized + Iterator,
    <Self as Iterator>::Item: Clone,
{
    fn repeat_first_n(self, n: usize) -> RepeatFirstN<Self>;
}

impl<T, I> RepeatFirst for T
where
    T: Iterator<Item = I>,
    I: Clone,
{
    fn repeat_first_n(mut self, n: usize) -> RepeatFirstN<Self> {
        let el = self.next();
        RepeatFirstN {
            it: self,
            e: el,
            c: 0,
            n,
        }
    }
}
