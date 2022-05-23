use midly::TrackEvent;

#[derive(Debug, serde::Serialize)]
pub struct Track {
    pub generated:        String,
    pub source_file:      String,
    pub events_processed: usize,
    pub events_emitted:   usize,
    pub emitted_meta:     bool,
    pub events:           Vec<Event>,
}

/// Event proxy containing an extra delta field that contains the correct delta
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct CDTrackEvent<'smf> {
    pub real_delta: usize,
    pub event:      TrackEvent<'smf>,
}

#[derive(Debug)]
pub enum PlayerResult<T> {
    Event(T),
    Ignored,
}

impl<T> PlayerResult<T> {
    pub fn map<U, F>(self, f: F) -> PlayerResult<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Self::Event(e) => PlayerResult::Event((f)(e)),
            Self::Ignored => PlayerResult::Ignored,
        }
    }
}

impl<T> From<Option<T>> for PlayerResult<T> {
    fn from(other: Option<T>) -> Self {
        match other {
            Some(v) => Self::Event(v),
            None => Self::Ignored,
        }
    }
}

const MICROS_PER_SECOND: u64 = 1_000_000;
pub struct PlayerTimingInfo {
    // state
    current_tick: u64,
    current_ms: f64,

    // timing data
    timing_data: TimingData
}

impl PlayerTimingInfo {
    pub fn next_tick(&mut self, delta: u64) -> NextTickInfo {
        let delta_len = self.timing_data.get_len(delta);

        self.current_tick += delta;
        self.current_ms += delta_len;

        NextTickInfo {
            delta_tick: delta,
            delta_micros: delta_len,
            abs_tick: self.current_tick,
            abs_micros: self.current_ms
        }
    }

    pub fn update_mpt(&mut self, delta: u64, npt: u32) {
        if let TimingData::Metric { ppqn, .. } = self.timing_data {
            self.current_tick += delta;
            self.current_ms += self.timing_data.get_len(delta);
            self.timing_data = TimingData::Metric { ppqn, npt: npt as f64 }
        }
    }
}

impl From<midly::Timing> for PlayerTimingInfo {
    fn from(t: midly::Timing) -> Self {
        let td = match t {
            midly::Timing::Metrical(ppqn) => TimingData::Metric { ppqn: ppqn.as_int() as f64, npt: 500_000f64 },
            midly::Timing::Timecode(fps, npt) => TimingData::Fps { fps: fps.as_f32(), tpf: npt },
        };

        PlayerTimingInfo { current_tick: 0, current_ms: 0.0, timing_data: td }
    }
}

#[derive(Debug)]
pub enum TimingData {
    Fps { fps: f32, tpf: u8  },
    Metric { ppqn: f64, npt: f64 }
}

impl TimingData {
    pub fn get_len(&self, ticks: u64) -> f64 {
        match self {
            TimingData::Fps { fps, tpf } => (MICROS_PER_SECOND as f64 / *fps as f64 / *tpf as f64) * ticks as f64,
            TimingData::Metric { ppqn, npt } => (npt/ppqn) * ticks as f64,
        }
    }
}

#[derive(Debug)]
pub struct NextTickInfo {
    pub delta_tick:   u64,
    pub delta_micros: f64,
    pub abs_tick:     u64,
    pub abs_micros:   f64,
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
    pub seconds: f32,
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
        bend_by: u16,
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
pub struct RepeatFirstN<I>
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

pub trait RepeatFirst
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
