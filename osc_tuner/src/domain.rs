use static_assertions as sa;
use xbounded::{make_bounded, Bounded};

pub const C0: usize = 12;
pub const C1: usize = 24;
pub const C4: usize = 60;
pub const C9: usize = 120;

#[allow(dead_code)]
pub const MIDI_SCALE_20KHZ: f32 = 135.076_23;

make_bounded!(pub MidiOscDomain, f32 : [12..120]);

make_bounded!(pub MidiFilterDomain, f32 : [12..135.076_23]);
sa::const_assert!(MIDI_SCALE_20KHZ == <MidiFilterDomain as Bounded>::MAX_REP);
