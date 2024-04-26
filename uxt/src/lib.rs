#![no_std]

pub use ux::{
    u1, u10, u11, u12, u13, u14, u15, u17, u18, u19, u2, u20, u21, u22, u23, u24, u25, u26, u27,
    u28, u29, u3, u30, u31, u4, u5, u6, u7, u9,
};

pub trait Ux: Into<Self::Rep>
where
    Self: TryFrom<Self::Rep>,
{
    type Rep;
    const MAX_REP: Self::Rep;
    const MIN_REP: Self::Rep;

    const MIN: Self;
    const MAX: Self;

    const BITS: usize;

    const VALUE_COUNT: usize;
    const U32_BITSET_SIZE: usize;
}

macro_rules! impl_ux {
    ($ty:ty, $rep:ty) => {
        impl Ux for $ty {
            type Rep = $rep;
            const MIN_REP: $rep = 0;
            const MAX_REP: $rep = (1 << Self::BITS - 1);
            const MIN: $ty = <$ty>::MIN;
            const MAX: $ty = <$ty>::MAX;
            const BITS: usize = <$ty>::BITS as usize;
            const VALUE_COUNT: usize = (1 << Self::BITS);
            const U32_BITSET_SIZE: usize = Self::VALUE_COUNT / 32;
        }
    };
}

impl_ux!(u1, u8);
impl_ux!(u2, u8);
impl_ux!(u3, u8);
impl_ux!(u4, u8);
impl_ux!(u5, u8);
impl_ux!(u6, u8);
impl_ux!(u7, u8);
impl_ux!(u8, u8);
impl_ux!(u9, u16);
impl_ux!(u10, u16);
impl_ux!(u11, u16);
impl_ux!(u12, u16);
impl_ux!(u13, u16);
impl_ux!(u14, u16);
impl_ux!(u15, u16);
impl_ux!(u16, u16);
impl_ux!(u17, u32);
impl_ux!(u18, u32);
impl_ux!(u19, u32);
impl_ux!(u20, u32);
impl_ux!(u21, u32);
impl_ux!(u22, u32);
impl_ux!(u23, u32);
impl_ux!(u24, u32);
impl_ux!(u25, u32);
impl_ux!(u26, u32);
impl_ux!(u27, u32);
impl_ux!(u28, u32);
impl_ux!(u29, u32);
impl_ux!(u30, u32);
impl_ux!(u31, u32);
impl_ux!(u32, u32);
