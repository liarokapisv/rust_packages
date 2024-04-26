use core::cell::Cell;

use crate::domain::{C4, C9};
use uxt::Ux;

pub const C4_TO_C9: usize = C9 - C4 + 1;

pub struct Table<T: Ux> {
    pub c0: Cell<T>,
    pub c1: Cell<T>,
    pub c4_to_c9_with_filler: [Cell<T>; C4_TO_C9 + 1],
}
