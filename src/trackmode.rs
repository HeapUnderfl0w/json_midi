use crate::model::{CDTrackEvent, RepeatFirst};
use itertools::Itertools;
use midly::{Smf, TrackEvent};
use std::marker::PhantomData;

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
