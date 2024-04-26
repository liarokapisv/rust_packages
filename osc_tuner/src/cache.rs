use core::{array::from_fn, cell::Cell, marker::PhantomData};

use crate::key_frequencies::MicrosPeriod;
use num_traits::AsPrimitive;
use uxt::Ux;

pub trait Cache {
    type Index;
    fn get(&self, index: Self::Index) -> Option<MicrosPeriod>;
    fn set(&self, index: Self::Index, value: MicrosPeriod);
}

pub struct NoCache<Index>(PhantomData<Index>);

impl<Index> Default for NoCache<Index> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Index> NoCache<Index> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<Index> Cache for NoCache<Index> {
    type Index = Index;

    fn get(&self, _index: Self::Index) -> Option<MicrosPeriod> {
        None
    }

    fn set(&self, _index: Self::Index, _value: MicrosPeriod) {}
}

pub struct FixedCache<Index, const SIZE: usize> {
    periods: [Cell<Option<MicrosPeriod>>; SIZE],
    phantom: PhantomData<Index>,
}

impl<Index, const SIZE: usize> FixedCache<Index, SIZE> {
    pub fn new() -> Self {
        Self {
            periods: from_fn(|_| Cell::new(None)),
            phantom: PhantomData,
        }
    }
}

impl<Index, const SIZE: usize> Default for FixedCache<Index, SIZE> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const SIZE: usize, Index> Cache for FixedCache<Index, SIZE>
where
    Index: Ux<VALUE_COUNT = { SIZE }>,
    Index::Rep: Copy + 'static + AsPrimitive<usize>,
{
    type Index = Index;

    fn get(&self, index: Self::Index) -> Option<MicrosPeriod> {
        self.periods[index.into().as_()].get()
    }

    fn set(&self, index: Self::Index, value: MicrosPeriod) {
        self.periods[index.into().as_()].set(Some(value))
    }
}
