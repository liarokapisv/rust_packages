#![feature(const_fn_floating_point_arithmetic)]
#![feature(const_float_bits_conv)]
#![feature(associated_const_equality)]
#![feature(unboxed_closures)]
#![feature(const_option)]
#![feature(impl_trait_in_assoc_type)]
#![feature(const_trait_impl)]
#![no_std]

use core::{cell::Cell, future::Future, ops::Sub};

use array_const_fn_init::array_const_fn_init;
use cache::Cache;
use num_integer::Average;
use static_assertions as sa;

use domain::{C0, C1, C4, C9};
use key_frequencies::{nth_key_period, MicrosPeriod};
use num_traits::One;
use table::Table;
use uxt::Ux;

use crate::cache::NoCache;

const C4_TO_C9_SIZE: usize = C9 - C4 + 1;

pub mod cache;
pub mod domain;
pub mod key_frequencies;
pub mod table;

const C0_PERIOD: MicrosPeriod = nth_key_period(C0 as f32);
const C1_PERIOD: MicrosPeriod = nth_key_period(C1 as f32);

const fn index_to_period(i: usize) -> MicrosPeriod {
    nth_key_period(48.0 + i as f32)
}

sa::const_assert!(C4_TO_C9_SIZE == 61);
const C4_TO_C9_PERIODS: [MicrosPeriod; 61] = array_const_fn_init![index_to_period; 61];

pub trait Oscf {
    type DacValue: Copy + 'static + Ux;

    fn get_period(&mut self) -> impl Future<Output = MicrosPeriod>;
    fn set_main_dac(&mut self, value: Self::DacValue) -> impl Future<Output = ()>;
    fn set_offset_dac(&mut self, value: Self::DacValue) -> impl Future<Output = ()>;
}

trait OscfExtPriv: Oscf
where
    <Self::DacValue as Ux>::Rep:
        Sub<Output = <Self::DacValue as Ux>::Rep> + One + PartialOrd + Average + Copy,
{
    async fn async_search(
        &mut self,
        async_get: impl AsyncGetPeriodGen<Self>,
        mut low: Self::DacValue,
        mut high: Self::DacValue,
        target: MicrosPeriod,
    ) -> Self::DacValue {
        loop {
            if high.into() - low.into() <= <<Self::DacValue as Ux>::Rep as One>::one() {
                if async_get.call(self).call(high).await >= target {
                    return high;
                } else {
                    return low;
                }
            }

            let mid = <Self::DacValue as TryFrom<<Self::DacValue as Ux>::Rep>>::try_from(
                low.into().average_floor(&high.into()),
            )
            .unwrap_or_else(|_| panic!("should never happen"));

            if target >= async_get.call(self).call(mid).await {
                high = mid;
            } else {
                low = mid;
            }
        }
    }

    async fn async_search_full(
        &mut self,
        async_get: impl AsyncGetPeriodGen<Self>,
        target: MicrosPeriod,
    ) -> Self::DacValue {
        self.async_search(
            async_get,
            <Self::DacValue as Ux>::MIN,
            <Self::DacValue as Ux>::MAX,
            target,
        )
        .await
    }

    async fn async_search_full_cached(
        &mut self,
        async_get: impl AsyncGetPeriodGen<Self>,
        cache: &impl Cache<Index = Self::DacValue>,
        target: MicrosPeriod,
    ) -> Self::DacValue {
        self.async_search_full(
            {
                impl<
                        'a,
                        O: Oscf + ?Sized,
                        G: AsyncGetPeriodGen<O>,
                        C: Cache<Index = O::DacValue>,
                    > AsyncGetPeriodGen<O> for Impl<'a, G, C>
                {
                    type Ret<'s> = impl AsyncGetPeriod<O>
                            where
                                Self: 's, O: 's;

                    fn call<'s>(&'s self, o: &'s mut O) -> Self::Ret<'s> {
                        move |dac| async move {
                            if let Some(period) = self.1.get(dac) {
                                return period;
                            }
                            let period = self.0.call(o).call(dac).await;
                            self.1.set(dac, period);
                            period
                        }
                    }
                }
                struct Impl<'a, G, C>(G, &'a C);
                Impl(async_get, cache)
            },
            target,
        )
        .await
    }

    async fn tune_midi_frequencies_impl(
        &mut self,
        table: &Table<Self::DacValue>,
        async_get_c0: impl AsyncGetPeriodGen<Self>,
        async_get_c1: impl AsyncGetPeriodGen<Self>,
        async_get_c4_to_c9: impl IndexedAsyncGetPeriodGen<Self> + Copy,
        cache: &impl Cache<Index = Self::DacValue>,
    ) {
        table.c0.set(
            self.async_search_full_cached(async_get_c0, cache, C0_PERIOD)
                .await,
        );
        table.c1.set(
            self.async_search_full_cached(async_get_c1, cache, C1_PERIOD)
                .await,
        );

        for (i, slot) in table.c4_to_c9_with_filler.iter().enumerate() {
            slot.set(
                self.async_search_full_cached(
                    {
                        impl<O: Oscf + ?Sized, G: IndexedAsyncGetPeriodGen<O>>
                            AsyncGetPeriodGen<O> for Impl<G>
                        {
                            type Ret<'s> = impl AsyncGetPeriod<O>
                            where
                                Self: 's, O: 's;

                            fn call<'s>(&'s self, o: &'s mut O) -> Self::Ret<'s> {
                                move |dac| async move { self.1.call(o).call(self.0, dac).await }
                            }
                        }
                        struct Impl<G>(usize, G);
                        Impl(i, async_get_c4_to_c9)
                    },
                    cache,
                    C4_TO_C9_PERIODS[i],
                )
                .await,
            );
        }
        table.c4_to_c9_with_filler[C4_TO_C9_SIZE]
            .set(table.c4_to_c9_with_filler[C4_TO_C9_SIZE - 1].get());
    }

    async fn tune_midi_frequencies_single(
        &mut self,
        table: &Table<Self::DacValue>,
        async_get: impl AsyncGetPeriodGen<Self> + Copy,
        cache: &impl Cache<Index = Self::DacValue>,
    ) {
        self.tune_midi_frequencies_impl(
            table,
            async_get,
            async_get,
            {
                impl<O: Oscf + ?Sized, G: AsyncGetPeriodGen<O>>
                    IndexedAsyncGetPeriodGen<O> for Impl<G>
                {
                    type Ret<'s> = impl IndexedAsyncGetPeriod<O>
                    where
                        Self: 's, O: 's;

                    fn call<'s>(&'s self, o: &'s mut O) -> Self::Ret<'s> {
                        move |_, dac| async move { self.0.call(o).call(dac).await }
                    }
                }
                #[derive(Clone, Copy)]
                struct Impl<G>(G);
                Impl(async_get)
            },
            cache,
        )
        .await;
    }

    async fn find_ratio(&mut self) -> Self::DacValue {
        let try_from = |x| {
            <Self::DacValue as TryFrom<<Self::DacValue as Ux>::Rep>>::try_from(x)
                .unwrap_or_else(|_| panic!("should never happen"))
        };

        let zero: Self::DacValue = Default::default();
        let zero_rep = zero.into();
        let one_rep = <<Self::DacValue as Ux>::Rep as One>::one();
        let max_rep = <Self::DacValue as Ux>::MAX_REP;

        let main_dac_target = try_from(zero_rep.average_floor(&max_rep));

        self.set_offset_dac(zero).await;
        self.set_main_dac(main_dac_target).await;

        let period = self.get_period().await;

        let main_dac_target_minus_one = try_from(main_dac_target.into() - one_rep);

        self.set_main_dac(main_dac_target_minus_one).await;
        self.async_search_full(
            {
                impl<O: Oscf + ?Sized> AsyncGetPeriodGen<O> for Impl {
                    type Ret<'s> = impl AsyncGetPeriod<O>
                    where
                        Self: 's, O: 's;

                    fn call<'s>(&self, o: &'s mut O) -> Self::Ret<'s> {
                        move |dac| async move {
                            o.set_offset_dac(dac).await;
                            o.get_period().await
                        }
                    }
                }
                struct Impl;
                Impl
            },
            period,
        )
        .await
    }

    async fn tune_midi_frequencies_offset(
        &mut self,
        main_table: &Table<Self::DacValue>,
        offset_table: &Table<Self::DacValue>,
    ) {
        impl<'a, O: Oscf + ?Sized> AsyncGetPeriodGen<O> for Impl<'a, O::DacValue> {
            type Ret<'s> = impl AsyncGetPeriod<O>
            where
                Self: 's, O: 's;

            fn call<'s>(&'s self, o: &'s mut O) -> Self::Ret<'s> {
                move |dac| async move {
                    o.set_main_dac(self.0.get()).await;
                    o.set_offset_dac(dac).await;
                    o.get_period().await
                }
            }
        }
        struct Impl<'a, D>(&'a Cell<D>);

        self.tune_midi_frequencies_impl(
            offset_table,
            Impl(&main_table.c0),
            Impl(&main_table.c1),
            {
                impl<'a, O: Oscf + ?Sized> IndexedAsyncGetPeriodGen<O> for Impl<'a, O::DacValue> {
                    type Ret<'s> = impl IndexedAsyncGetPeriod<O>
                    where
                        Self: 's, O: 's;

                    fn call<'s>(&'s self, o: &'s mut O) -> Self::Ret<'s> {
                        move |i: usize, dac| async move {
                            o.set_main_dac(self.0.c4_to_c9_with_filler[i].get()).await;
                            o.set_offset_dac(dac).await;
                            o.get_period().await
                        }
                    }
                }
                #[derive(Clone, Copy)]
                struct Impl<'a, D: Ux>(&'a Table<D>);
                Impl(main_table)
            },
            &NoCache::new(),
        )
        .await
    }

    async fn tune_midi_frequencies(
        &mut self,
        main_table: &mut Table<Self::DacValue>,
        offset_table: &mut Table<Self::DacValue>,
        cache: &impl Cache<Index = Self::DacValue>,
    ) -> Self::DacValue {
        self.set_offset_dac(Default::default()).await;
        self.tune_midi_frequencies_single(
            main_table,
            {
                impl<O: Oscf + ?Sized> AsyncGetPeriodGen<O> for Impl {
                    type Ret<'s> = impl AsyncGetPeriod<O>
                    where O : 's;
                    fn call<'s>(&'s self, o: &'s mut O) -> Self::Ret<'s> {
                        move |dac| async move {
                            o.set_main_dac(dac).await;
                            o.get_period().await
                        }
                    }
                }
                #[derive(Copy, Clone)]
                struct Impl;
                Impl
            },
            cache,
        )
        .await;
        self.tune_midi_frequencies_offset(main_table, offset_table)
            .await;
        self.find_ratio().await
    }
}

impl<F: OscfExt + ?Sized> OscfExtPriv for F where
    <Self::DacValue as Ux>::Rep:
        Sub<Output = <Self::DacValue as Ux>::Rep> + One + PartialOrd + Average + Copy
{
}

pub trait OscfExt: Oscf
where
    <Self::DacValue as Ux>::Rep:
        Sub<Output = <Self::DacValue as Ux>::Rep> + One + PartialOrd + Average + Copy,
{
    fn tune_midi_frequencies(
        &mut self,
        main_table: &mut Table<Self::DacValue>,
        offset_table: &mut Table<Self::DacValue>,
        cache: &impl Cache<Index = Self::DacValue>,
    ) -> impl core::future::Future<Output = Self::DacValue> {
        <Self as OscfExtPriv>::tune_midi_frequencies(self, main_table, offset_table, cache)
    }
}

trait AsyncGetPeriod<O: Oscf + ?Sized>: FnOnce<(O::DacValue,)> {
    type Ret: Future<Output = MicrosPeriod>;
    fn call(self, value: O::DacValue) -> Self::Ret;
}

trait IndexedAsyncGetPeriod<O: Oscf + ?Sized>: FnOnce<(usize, O::DacValue)> {
    type Ret: Future<Output = MicrosPeriod>;
    fn call(self, index: usize, value: O::DacValue) -> Self::Ret;
}

impl<O: Oscf + ?Sized, I: Future<Output = MicrosPeriod>, F: FnOnce<(O::DacValue,), Output = I>>
    AsyncGetPeriod<O> for F
{
    type Ret = Self::Output;
    fn call(self, value: O::DacValue) -> Self::Ret {
        self(value)
    }
}

impl<
        O: Oscf + ?Sized,
        I: Future<Output = MicrosPeriod>,
        F: FnOnce<(usize, O::DacValue), Output = I>,
    > IndexedAsyncGetPeriod<O> for F
{
    type Ret = Self::Output;
    fn call(self, index: usize, value: O::DacValue) -> Self::Ret {
        self(index, value)
    }
}

trait AsyncGetPeriodGen<O: Oscf + ?Sized> {
    type Ret<'s>: AsyncGetPeriod<O>
    where
        Self: 's,
        O: 's;

    fn call<'s>(&'s self, o: &'s mut O) -> Self::Ret<'s>;
}

trait IndexedAsyncGetPeriodGen<O: Oscf + ?Sized> {
    type Ret<'s>: IndexedAsyncGetPeriod<O>
    where
        Self: 's,
        O: 's;

    fn call<'s>(&'s self, o: &'s mut O) -> Self::Ret<'s>;
}
