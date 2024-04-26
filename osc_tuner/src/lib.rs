#![feature(const_refs_to_static)]
#![feature(const_fn_floating_point_arithmetic)]
#![feature(const_float_bits_conv)]
#![feature(associated_const_equality)]
#![feature(type_alias_impl_trait)]
#![feature(unboxed_closures)]
#![feature(associated_type_defaults)]
#![feature(tuple_trait)]
#![feature(fn_traits)]
#![feature(const_option)]
#![feature(impl_trait_in_assoc_type)]
#![no_std]

use core::{cell::Cell, future::Future, marker::Tuple, ops::Sub};

use array_const_fn_init::array_const_fn_init;
use cache::Cache;
use static_assertions as sa;

use domain::{C0, C1, C4, C9};
use key_frequencies::{nth_key_period, MicrosPeriod};
use midpoint::MidpointViaPrimitivePromotionExt;
use num_traits::{One, Zero};
use table::Table;
use uxt::Ux;

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

pub trait AsyncFnOnce<Args: Tuple> {
    type Ret;
    type Output: Future<Output = Self::Ret>;

    fn call(self, args: Args) -> Self::Output;
}

impl<Args: Tuple, R, I: Future<Output = R>, F: FnOnce<Args, Output = I>> AsyncFnOnce<Args> for F {
    type Ret = R;

    type Output = impl Future<Output = Self::Ret>;

    fn call(self, args: Args) -> Self::Output {
        self.call_once(args)
    }
}

pub trait AsyncFnOnceGen<O: Oscf + ?Sized, Args: Tuple> {
    type Ret;
    type AsyncFnOnceType<'s>: AsyncFnOnce<Args, Ret = Self::Ret>
    where
        Self: 's,
        O: 's;

    fn call<'s>(&'s self, o: &'s mut O) -> Self::AsyncFnOnceType<'s>;
}

pub trait Oscf {
    type DacValue;

    fn get_period(&mut self) -> impl Future<Output = MicrosPeriod>;
    fn set_main_dac(&mut self, value: Self::DacValue) -> impl Future<Output = ()>;
    fn set_offset_dac(&mut self, value: Self::DacValue) -> impl Future<Output = ()>;
}

trait OscfExtPriv: Oscf
where
    <Self as Oscf>::DacValue: Ux + Default + Copy + 'static,
    <<Self as Oscf>::DacValue as Ux>::Rep: Sub<Output = <<Self as Oscf>::DacValue as Ux>::Rep>
        + Zero
        + One
        + PartialOrd
        + MidpointViaPrimitivePromotionExt
        + Copy,
{
    async fn async_search(
        &mut self,
        async_get: impl AsyncFnOnceGen<Self, (Self::DacValue,), Ret = MicrosPeriod>,
        mut low: Self::DacValue,
        mut high: Self::DacValue,
        target: MicrosPeriod,
    ) -> Self::DacValue {
        loop {
            if high.into() - low.into() <= <<<Self as Oscf>::DacValue as Ux>::Rep as One>::one() {
                if async_get.call(self).call((high,)).await >= target {
                    return high;
                } else {
                    return low;
                }
            }

            let mid = <<Self as Oscf>::DacValue as TryFrom<
                <<Self as Oscf>::DacValue as Ux>::Rep,
            >>::try_from(
                low.into().midpoint_via_primitive_promotion(&high.into())
            )
            .unwrap_or_else(|_| panic!("should never happen"));

            if target >= async_get.call(self).call((mid,)).await {
                high = mid;
            } else {
                low = mid;
            }
        }
    }

    async fn async_search_full(
        &mut self,
        async_get: impl AsyncFnOnceGen<Self, (Self::DacValue,), Ret = MicrosPeriod>,
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
        async_get: impl AsyncFnOnceGen<Self, (Self::DacValue,), Ret = MicrosPeriod>,
        cache: &Option<&dyn Cache<Index = Self::DacValue>>,
        target: MicrosPeriod,
    ) -> Self::DacValue {
        self.async_search_full(
            {
                struct CacheAsyncFnOnceGenAdapter<'a, G, C: ?Sized>(G, &'a Option<&'a C>);
                impl<
                        'a,
                        DacValue: Copy + 'static,
                        O: Oscf<DacValue = DacValue> + ?Sized,
                        G: AsyncFnOnceGen<O, (DacValue,), Ret = MicrosPeriod>,
                        C: Cache<Index = DacValue> + ?Sized,
                    > AsyncFnOnceGen<O, (DacValue,)> for CacheAsyncFnOnceGenAdapter<'a, G, C>
                {
                    type Ret = G::Ret;
                    type AsyncFnOnceType<'s> = impl AsyncFnOnce<(DacValue,), Ret = Self::Ret>
                        where
                            Self: 's, O: 's;

                    fn call<'s>(&'s self, o: &'s mut O) -> Self::AsyncFnOnceType<'s> {
                        move |dac| async move {
                            if let Some(ref cache) = self.1 {
                                if let Some(period) = cache.get(dac) {
                                    return period;
                                }
                            }

                            let period = self.0.call(o).call((dac,)).await;

                            if let Some(ref cache) = self.1 {
                                cache.set(dac, period);
                            }

                            period
                        }
                    }
                }
                CacheAsyncFnOnceGenAdapter(async_get, cache)
            },
            target,
        )
        .await
    }

    async fn tune_midi_frequencies_impl(
        &mut self,
        table: &Table<Self::DacValue>,
        async_get_c0: impl AsyncFnOnceGen<Self, (Self::DacValue,), Ret = MicrosPeriod>,
        async_get_c1: impl AsyncFnOnceGen<Self, (Self::DacValue,), Ret = MicrosPeriod>,
        async_get_c4_to_c9: impl AsyncFnOnceGen<Self, (usize, Self::DacValue), Ret = MicrosPeriod>
            + Copy,
        cache: &Option<&dyn Cache<Index = Self::DacValue>>,
    ) {
        table.c0.set(
            self.async_search_full_cached(async_get_c0, cache, C0_PERIOD)
                .await,
        );
        table.c1.set(
            self.async_search_full_cached(async_get_c1, cache, C1_PERIOD)
                .await,
        );

        for i in 0..C4_TO_C9_SIZE {
            table.c4_to_c9_with_filler[i].set(
                self.async_search_full_cached({
                        struct IndexedFnOnceAdapterGen<G>(usize, G);
                        impl<
                                DacValue: Copy + 'static,
                                O: Oscf<DacValue = DacValue> + ?Sized,
                                G: AsyncFnOnceGen<O, (usize, DacValue), Ret = MicrosPeriod>,
                            > AsyncFnOnceGen<O, (DacValue,)> for IndexedFnOnceAdapterGen<G>
                        {
                            type Ret = MicrosPeriod;
                            type AsyncFnOnceType<'s> = impl AsyncFnOnce<(DacValue,), Ret = Self::Ret>
                            where
                                Self: 's, O: 's;

                            fn call<'s>(&'s self, o: &'s mut O) -> Self::AsyncFnOnceType<'s> {
                                move |dac| async move { self.1.call(o).call((self.0, dac)).await }
                            }
                        }
                        IndexedFnOnceAdapterGen(i, async_get_c4_to_c9)
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
        async_get: impl AsyncFnOnceGen<Self, (Self::DacValue,), Ret = MicrosPeriod> + Copy,
        cache: &Option<&dyn Cache<Index = Self::DacValue>>,
    ) {
        self.tune_midi_frequencies_impl(
            table,
            async_get,
            async_get,
            {
                #[derive(Clone, Copy)]
                struct IgnoreFnOnceAdapterGen<G>(G);
                impl<
                        DacValue: Copy + 'static,
                        O: Oscf<DacValue = DacValue> + ?Sized,
                        G: AsyncFnOnceGen<O, (DacValue,), Ret = MicrosPeriod>,
                    > AsyncFnOnceGen<O, (usize, DacValue)> for IgnoreFnOnceAdapterGen<G>
                {
                    type Ret = MicrosPeriod;
                    type AsyncFnOnceType<'s> = impl AsyncFnOnce<(usize, DacValue,), Ret = Self::Ret>
                    where
                        Self: 's, O: 's;

                    fn call<'s>(&'s self, o: &'s mut O) -> Self::AsyncFnOnceType<'s> {
                        move |_, dac| async move { self.0.call(o).call((dac,)).await }
                    }
                }
                IgnoreFnOnceAdapterGen(async_get)
            },
            cache,
        )
        .await;
    }

    async fn find_ratio(&mut self) -> Self::DacValue {
        let try_from = |x| {
            <<Self as Oscf>::DacValue as TryFrom<<<Self as Oscf>::DacValue as Ux>::Rep>>::try_from(
                x,
            )
            .unwrap_or_else(|_| panic!("should never happen"))
        };

        let zero: <Self as Oscf>::DacValue = Default::default();
        let zero_rep = zero.into();
        let one_rep = <<<Self as Oscf>::DacValue as Ux>::Rep as One>::one();
        let max_rep = <<Self as Oscf>::DacValue as Ux>::MAX_REP;

        let main_dac_target = try_from(zero_rep.midpoint_via_primitive_promotion(&max_rep));

        self.set_offset_dac(zero).await;
        self.set_main_dac(main_dac_target).await;

        let period = self.get_period().await;

        let main_dac_target_minus_one = try_from(main_dac_target.into() - one_rep);

        self.set_main_dac(main_dac_target_minus_one).await;
        self.async_search_full(
            {
                struct OffsetDacGetPeriodFnOnceGen;
                impl<DacValue: Copy + 'static, O: Oscf<DacValue = DacValue> + ?Sized>
                    AsyncFnOnceGen<O, (DacValue,)> for OffsetDacGetPeriodFnOnceGen
                {
                    type Ret = MicrosPeriod;
                    type AsyncFnOnceType<'s> = impl AsyncFnOnce<(DacValue,), Ret = Self::Ret>
                    where
                        Self: 's, O: 's;

                    fn call<'s>(&self, o: &'s mut O) -> Self::AsyncFnOnceType<'s> {
                        move |dac| async move {
                            o.set_offset_dac(dac).await;
                            o.get_period().await
                        }
                    }
                }
                OffsetDacGetPeriodFnOnceGen
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
        struct ChainOffsetFnOnceAdapterGen<'a, D>(&'a Cell<D>);
        impl<'a, DacValue: Copy + 'static, O: Oscf<DacValue = DacValue> + ?Sized>
            AsyncFnOnceGen<O, (DacValue,)> for ChainOffsetFnOnceAdapterGen<'a, DacValue>
        {
            type Ret = MicrosPeriod;
            type AsyncFnOnceType<'s> = impl AsyncFnOnce<(DacValue,), Ret = Self::Ret>
            where
                Self: 's, O: 's;

            fn call<'s>(&'s self, o: &'s mut O) -> Self::AsyncFnOnceType<'s> {
                move |dac| async move {
                    o.set_main_dac(self.0.get()).await;
                    o.set_offset_dac(dac).await;
                    o.get_period().await
                }
            }
        }

        self.tune_midi_frequencies_impl(
            offset_table,
            ChainOffsetFnOnceAdapterGen(&main_table.c0),
            ChainOffsetFnOnceAdapterGen(&main_table.c1),
            {
                #[derive(Clone, Copy)]
                struct IndexedChainOffsetFnOnceAdapterGen<'a, D: Ux>(&'a Table<D>);
                impl<'a, DacValue: Copy + 'static + Ux, O: Oscf<DacValue = DacValue> + ?Sized>
                    AsyncFnOnceGen<O, (usize, DacValue)>
                    for IndexedChainOffsetFnOnceAdapterGen<'a, DacValue>
                {
                    type Ret = MicrosPeriod;
                    type AsyncFnOnceType<'s> = impl AsyncFnOnce<(usize, DacValue,), Ret = Self::Ret>
                    where
                        Self: 's, O: 's;

                    fn call<'s>(&'s self, o: &'s mut O) -> Self::AsyncFnOnceType<'s> {
                        move |i: usize, dac| async move {
                            o.set_main_dac(self.0.c4_to_c9_with_filler[i].get()).await;
                            o.set_offset_dac(dac).await;
                            o.get_period().await
                        }
                    }
                }
                IndexedChainOffsetFnOnceAdapterGen(&main_table)
            },
            &None,
        )
        .await
    }

    async fn tune_midi_frequencies(
        &mut self,
        main_table: &mut Table<Self::DacValue>,
        offset_table: &mut Table<Self::DacValue>,
        cache: &Option<&dyn Cache<Index = Self::DacValue>>,
    ) -> Self::DacValue {
        let zero: <Self as Oscf>::DacValue = Default::default();
        self.set_offset_dac(zero).await;
        self.tune_midi_frequencies_single(
            main_table,
            {
                #[derive(Copy, Clone)]
                struct MainDacGetPeriodFnOnceGen;
                impl<DacValue: Copy + 'static, O: Oscf<DacValue = DacValue> + ?Sized>
                    AsyncFnOnceGen<O, (DacValue,)> for MainDacGetPeriodFnOnceGen
                {
                    type Ret = MicrosPeriod;
                    type AsyncFnOnceType<'s> = impl AsyncFnOnce<(DacValue,), Ret = Self::Ret>
                    where
                        Self : 's, O : 's;

                    fn call<'s>(&'s self, o: &'s mut O) -> Self::AsyncFnOnceType<'s> {
                        move |dac| async move {
                            o.set_main_dac(dac).await;
                            o.get_period().await
                        }
                    }
                }
                MainDacGetPeriodFnOnceGen
            },
            cache,
        )
        .await;
        self.tune_midi_frequencies_offset(main_table, offset_table)
            .await;
        self.find_ratio().await
    }
}

impl<F: OscfExt + ?Sized> OscfExtPriv for F
where
    <Self as Oscf>::DacValue: Ux + Default + Copy + 'static,
    <<Self as Oscf>::DacValue as Ux>::Rep: Sub<Output = <<Self as Oscf>::DacValue as Ux>::Rep>
        + Zero
        + One
        + PartialOrd
        + MidpointViaPrimitivePromotionExt
        + Copy,
{
}

pub trait OscfExt: Oscf
where
    <Self as Oscf>::DacValue: Ux + Default + Copy + 'static,
    <<Self as Oscf>::DacValue as Ux>::Rep: Sub<Output = <<Self as Oscf>::DacValue as Ux>::Rep>
        + Zero
        + One
        + PartialOrd
        + MidpointViaPrimitivePromotionExt
        + Copy,
{
    fn tune_midi_frequencies(
        &mut self,
        main_table: &mut Table<Self::DacValue>,
        offset_table: &mut Table<Self::DacValue>,
        cache: &Option<&dyn Cache<Index = Self::DacValue>>,
    ) -> impl core::future::Future<Output = Self::DacValue> {
        <Self as OscfExtPriv>::tune_midi_frequencies(self, main_table, offset_table, cache)
    }
}
