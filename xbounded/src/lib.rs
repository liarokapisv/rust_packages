#![no_std]
#![feature(const_fn_floating_point_arithmetic)]

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "postcard")]
use postcard::experimental::max_size::MaxSize;

pub trait Bounded {
    type Rep;
    const MIN_REP: Self::Rep;
    const MAX_REP: Self::Rep;

    const MIN: Self;
    const MAX: Self;

    const HAS_DEFAULT: bool;
}

#[macro_export]
macro_rules! __priv_make_bounded {
([$($meta:meta),*], $has_default:expr, $vis:vis $name:ident, $ty:ty, $min:literal, $max:literal) => {
    #[derive($($meta,)*)]
    #[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[cfg_attr(feature = "postcard", derive(MaxSize))]

    $vis struct $name ($ty);

    impl $name {
        pub const fn new(value: $ty) -> Option<Self> {
            if value < $min as $ty || value > $max as $ty {
                return None;
            }

            Some(Self(value))
        }

        pub const unsafe fn new_unchecked(value: $ty) -> Self {
            Self(value)
        }

        pub const fn clamp(value: $ty) -> Self {
            if value < $min as $ty {
                return Self($min as $ty)
            }
            if value > $max as $ty {
                return Self($max as $ty)
            }
            Self(value)
        }

        pub const fn min_rep() -> $ty {
            $min as $ty
        }

        pub const fn min() -> Self {
            Self($min as $ty)
        }

        pub const fn max_rep() -> $ty {
            $max as $ty
        }

        pub const fn max() -> Self {
            Self($max as $ty)
        }


        pub const fn get(&self) -> $ty
        {
            self.0
        }
    }

    impl From<$name> for $ty {
        fn from(value: $name) -> $ty {
            value.0
        }
    }

    impl TryFrom<$ty> for $name {
        type Error = ();
        fn try_from(value: $ty) -> Result<$name, Self::Error> {
            $name::new(value).ok_or(())
        }
    }

    impl $crate::Bounded for $name {
        type Rep = $ty;
        const MIN_REP : Self::Rep = <$name>::min_rep();
        const MAX_REP : Self::Rep = <$name>::max_rep();
        const MIN : Self = <$name>::min();
        const MAX : Self = <$name>::max();
        const HAS_DEFAULT : bool = $has_default;

    }
};
}

#[macro_export]
macro_rules! make_bounded {
    ($vis:vis $name:ident, f32 : [-$min:literal..$max:literal]) => {
        $crate::__priv_make_bounded!([Default], true, $vis $name, f32, -$min, $max);
    };
    ($vis:vis $name:ident, f32 : [-$min:literal..0]) => {
        $crate::__priv_make_bounded!([Default], true, $vis $name, f32, -$min, 0);
    };
    ($vis:vis $name:ident, f32 : [0..$max:literal]) => {
        $crate::__priv_make_bounded!([Default], true, $vis $name, f32, 0, $max);
    };
    ($vis:vis $name:ident, f32 : [$min:literal..$max:literal]) => {
        $crate::__priv_make_bounded!([], false, $vis $name, f32, $min, $max);
    };
    ($vis:vis $name:ident, $ty:ty : [-$min:literal..$max:literal]) => {
        $crate::__priv_make_bounded!([Default, Ord, Eq], true, $vis $name, $ty, -$min, $max);
    };
    ($vis:vis $name:ident, $ty:ty : [-$min:literal..0]) => {
        $crate::__priv_make_bounded!([Default, Ord, Eq], true, $vis $name, $ty, -$min, 0);
    };
    ($vis:vis $name:ident, $ty:ty : [0..$max:literal]) => {
        $crate::__priv_make_bounded!([Default, Ord, Eq], true, $vis $name, $ty, 0, $max);
    };
    ($vis:vis $name:ident, $ty:ty : [$min:literal..$max:literal]) => {
        $crate::__priv_make_bounded!([Ord, Eq], false, $vis $name, $ty, $min, $max);
    };
}

make_bounded!(pub Norm, f32 : [0..1]);
make_bounded!(pub SNorm, f32 : [-1..1]);
