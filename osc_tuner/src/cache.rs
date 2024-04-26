use core::{array::from_fn, cell::Cell};

use crate::key_frequencies::MicrosPeriod;
use num_traits::AsPrimitive;
use uxt::Ux;

pub trait Cache {
    type Index;
    fn get(&self, index: Self::Index) -> Option<MicrosPeriod>;
    fn set(&self, index: Self::Index, value: MicrosPeriod);
}

pub struct FixedCache<const SIZE: usize> {
    periods: [Option<Cell<MicrosPeriod>>; SIZE],
}

impl<const SIZE: usize> FixedCache<SIZE> {
    pub fn new() -> Self {
        Self {
            periods: from_fn(|_| None),
        }
    }

    pub fn get<T>(&self, index: T) -> Option<MicrosPeriod>
    where
        T: Ux<VALUE_COUNT = { SIZE }>,
        T::Rep: Copy + 'static + AsPrimitive<usize>,
    {
        Some(self.periods[index.into().as_()].as_ref()?.get())
    }

    pub fn set<T>(&mut self, index: T, value: MicrosPeriod)
    where
        T: Ux<VALUE_COUNT = { SIZE }>,
        T::Rep: Copy + 'static + AsPrimitive<usize>,
    {
        self.periods[index.into().as_()] = Some(value.into());
    }
}

impl<const SIZE: usize> Default for FixedCache<SIZE> {
    fn default() -> Self {
        Self::new()
    }
}
