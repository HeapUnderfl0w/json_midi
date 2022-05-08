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

#[derive(Debug)]
pub struct PlayerTimingInfo {
    offset_tick:    u64,
    offset_micros:  u64,
    // o-o-o-o-o-o-o-o-o-o
    micros_per_qn:  u64,
    ticks_per_beat: u64,

    is_timecode:    bool,
    nanos_per_tick: u64,
}

const MICROS_PER_SECOND: u64 = 1_000_000;
impl Default for PlayerTimingInfo {
    fn default() -> Self {
        let mut s = Self {
            offset_tick:   Default::default(),
            offset_micros: Default::default(),

            is_timecode: false,

            // ticks in micros
            nanos_per_tick: 0,

            // default to 120 bpm
            micros_per_qn:  500_000,
            ticks_per_beat: 120,
        };
        s.recalc_ticklen();
        s
    }
}

#[derive(Debug)]
pub struct NextTickInfo {
    pub delta_tick:   u64,
    pub delta_micros: u64,
    pub abs_tick:     u64,
    pub abs_micros:   u64,
}

impl PlayerTimingInfo {
    pub fn next_tick(&mut self, delta: u64) -> NextTickInfo {
        let new_t_off = self.nanos_per_tick * delta;
        self.offset_micros += new_t_off;
        self.offset_tick += delta;
        NextTickInfo {
            delta_tick:   delta,
            delta_micros: new_t_off,
            abs_tick:     self.offset_tick,
            abs_micros:   self.offset_micros,
        }
    }

    pub fn set_timecode(&mut self, fps: midly::Fps, tpf: u64) -> () {
        self.is_timecode = true;
        self.nanos_per_tick = MICROS_PER_SECOND / (fps.as_int() as u64 * tpf)
    }

    fn recalc_ticklen(&mut self) {
        if !self.is_timecode {
            self.nanos_per_tick = self.micros_per_qn / self.ticks_per_beat;
        }
    }

    pub fn set_micros_per_qn(&mut self, mc: u64) {
        self.micros_per_qn = mc;
        self.recalc_ticklen();
    }

    pub fn set_ticks_per_beat(&mut self, tpb: u64) {
        self.ticks_per_beat = tpb;
        self.recalc_ticklen();
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
