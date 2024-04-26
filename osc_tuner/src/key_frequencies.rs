use core::num::NonZeroU16;

use const_soft_float::soft_f32::SoftF32;

const TBLSIZE: usize = 16;

const EXP2FT: [u64; TBLSIZE] = [
    0x3fe6a09e667f3bcd,
    0x3fe7a11473eb0187,
    0x3fe8ace5422aa0db,
    0x3fe9c49182a3f090,
    0x3feae89f995ad3ad,
    0x3fec199bdd85529c,
    0x3fed5818dcfba487,
    0x3feea4afa2a490da,
    0x3ff0000000000000,
    0x3ff0b5586cf9890f,
    0x3ff172b83c7d517b,
    0x3ff2387a6e756238,
    0x3ff306fe0a31b715,
    0x3ff3dea64c123422,
    0x3ff4bfdad5362a27,
    0x3ff5ab07dd485429,
];

const fn exp2f(mut x: f32) -> f32 {
    let redux = f32::from_bits(0x4b400000) / TBLSIZE as f32;
    let p1 = f32::from_bits(0x3f317218);
    let p2 = f32::from_bits(0x3e75fdf0);
    let p3 = f32::from_bits(0x3d6359a4);
    let p4 = f32::from_bits(0x3c1d964e);

    // double_t t, r, z;
    // uint32_t ix, i0, k;

    let x1p127 = f32::from_bits(0x7f000000);

    /* Filter out exceptional cases. */
    let ui = f32::to_bits(x);
    let ix = ui & 0x7fffffff;
    if ix > 0x42fc0000 {
        /* |x| > 126 */
        if ix > 0x7f800000 {
            /* NaN */
            return x;
        }
        if ui >= 0x43000000 && ui < 0x80000000 {
            /* x >= 128 */
            x *= x1p127;
            return x;
        }
        if ui >= 0x80000000 {
            /* x < -126 */
            if ui >= 0xc3160000 || (ui & 0x0000ffff != 0) {
                return f32::from_bits(0x80000001) / x;
            }
            if ui >= 0xc3160000 {
                /* x <= -150 */
                return 0.0;
            }
        }
    } else if ix <= 0x33000000 {
        /* |x| <= 0x1p-25 */
        return 1.0 + x;
    }

    /* Reduce x, computing z, i0, and k. */
    let ui = f32::to_bits(x + redux);
    let mut i0 = ui;
    i0 += TBLSIZE as u32 / 2;
    let k = i0 / TBLSIZE as u32;
    let ukf = f64::from_bits(((0x3ff + k) as u64) << 52);
    i0 &= TBLSIZE as u32 - 1;
    let mut uf = f32::from_bits(ui);
    uf -= redux;
    let z: f64 = (x - uf) as f64;
    /* Compute r = exp2(y) = exp2ft[i0] * p(z). */
    let r: f64 = f64::from_bits(EXP2FT[i0 as usize]);
    let t: f64 = r * z;
    let r: f64 = r + t * (p1 as f64 + z * p2 as f64) + t * (z * z) * (p3 as f64 + z * p4 as f64);

    /* Scale by 2**k */
    (r * ukf) as f32
}

const fn round(n: f32) -> f32 {
    SoftF32(n).round().0
}

pub const fn nth_key_frequency(n: f32) -> f32 {
    exp2f((n - 57.0) / 12.0) * 440.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MicrosPeriod(NonZeroU16);

pub const fn nth_key_period(n: f32) -> MicrosPeriod {
    let period = round(1000000.0 / nth_key_frequency(n));
    MicrosPeriod(NonZeroU16::new(period as u16).unwrap())
}
