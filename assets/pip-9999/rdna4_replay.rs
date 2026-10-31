//! Reference replay for an RDNA4 FP8 device: `MatMul_RDNA4`, the FP8 matrix
//! product of AMD gfx1200/gfx1201 (`v_wmma_f32_16x16x16_fp8_fp8`, E4M3 x E4M3
//! -> FP32 accumulate) as measured on RX 9070 XT and RX 9060 XT silicon.
//!
//! `GFloat`, `multiply_fp8_to_gfloat`, `windowed_group_sum` and the windowed
//! matmul loop (used here only for the B200 contrast) are copied from
//! pearl-research-labs/pearl, zk-pow/src/v4/api/utils.rs (branch fp8-scheme
//! @ 684f7045), with `anyhow` replaced by panics so the file builds alone.
//! Copied code: Copyright (c) 2025-2026 Pearl Research Labs, ISC License.
//! Build and run:
//!
//!     rustc --edition 2021 --test rdna4_replay.rs -o rdna4_replay_test && ./rdna4_replay_test

const FP32_MIN_NONZERO_EXPONENT: i32 = -126;
const MMA_GROUP_PRODUCTS: usize = 32;
const B200_FP8_WIDTH: u16 = 26;

/// RDNA4 collapses the accumulator every 8 products (two per 16-deep WMMA).
pub const RDNA4_GROUP_PRODUCTS: usize = 8;
/// Inside a group, products are pre-summed in quads of 4.
const RDNA4_QUAD_PRODUCTS: usize = 4;
/// Fractional bits kept below the anchor exponent, at both levels.
const RDNA4_WINDOW_BITS: i32 = 24;
/// The carry's anchor is its FP32 exponent minus this (products carry two
/// integer bits above their stored exponent, the FP32 carry one).
const RDNA4_CARRY_ANCHOR_OFFSET: i32 = 2;

fn check_not_nan_or_inf_f32(value: f32) {
    assert!(value.is_finite(), "f32 value {value} is not finite");
}

// ---- copied from utils.rs ------------------------------------------------

#[derive(Clone, Copy)]
pub struct GFloat {
    pub sign: bool,
    pub exponent: i32,
    pub significand: u32,
}

impl From<f32> for GFloat {
    fn from(value: f32) -> Self {
        check_not_nan_or_inf_f32(value);
        let a_bits = value.to_bits();
        let sign = (a_bits >> 31) & 1 == 1;
        let exponent = ((a_bits >> 23) & 0xFF) as i32;
        let significand = a_bits & 0x7FFFFF;
        if exponent == 0 && significand == 0 {
            return GFloat { sign, exponent: -133, significand: 0 };
        }
        let (exponent, significand) = if exponent != 0 { (exponent, 0x800000 | significand) } else { (1, significand) };
        GFloat { sign, exponent: exponent - 127, significand }
    }
}

impl From<GFloat> for f32 {
    fn from(val: GFloat) -> f32 {
        if val.significand == 0 {
            return f32::from_bits((val.sign as u32) << 31);
        }
        let mut exponent = val.exponent + 127;
        if (val.significand & 0x800000) == 0 {
            exponent -= 1;
        }
        let exp_bits = u32::try_from(exponent).expect("GFloat exponent below f32 range");
        let value = f32::from_bits((val.sign as u32) << 31 | (exp_bits & 0xFF) << 23 | (val.significand & 0x7FFFFF));
        check_not_nan_or_inf_f32(value);
        value
    }
}

fn multiply_fp8_to_gfloat(a: u8, b: u8, zero_exp: i32) -> GFloat {
    let sign_a = (a >> 7) & 1 == 1;
    let mut exp_a = (a >> 3) & 0x0F;
    let sig_a = a & 0x07;
    let sign_b = (b >> 7) & 1 == 1;
    let mut exp_b = (b >> 3) & 0x0F;
    let sig_b = b & 0x07;
    let significand_a = if exp_a != 0 { sig_a | 0x08 } else { sig_a };
    let significand_b = if exp_b != 0 { sig_b | 0x08 } else { sig_b };
    if exp_a == 0 {
        exp_a = 1;
    }
    if exp_b == 0 {
        exp_b = 1;
    }
    let result_significand = (significand_a as u32 * significand_b as u32) << 17;
    if result_significand == 0 {
        GFloat { sign: sign_a ^ sign_b, exponent: zero_exp, significand: 0 }
    } else {
        GFloat { sign: sign_a ^ sign_b, exponent: exp_a as i32 + exp_b as i32 - 14, significand: result_significand }
    }
}

fn windowed_group_sum(values: &[GFloat], zero_exp: i32, significand_width: u16) -> GFloat {
    let base_sig = 24;
    let is_neg_shift = significand_width < base_sig;
    let diff = significand_width.abs_diff(base_sig) as u32;
    let significand_width = significand_width as i32;
    let signed_zero = |sign: bool| GFloat { sign, exponent: zero_exp, significand: 0 };
    let Some(max_exponent) = values.iter().filter(|g| g.significand != 0).map(|g| g.exponent).max() else {
        return signed_zero(false);
    };
    let mut acc: i64 = 0;
    for v in values {
        if v.significand == 0 {
            continue;
        }
        let shift = max_exponent - v.exponent;
        if shift >= 32 {
            continue;
        }
        let shifted = if is_neg_shift { v.significand >> diff } else { v.significand << diff };
        let aligned = (shifted >> shift) as i64;
        acc += if v.sign { -aligned } else { aligned };
    }
    let sign = acc < 0;
    let mut significand = acc.unsigned_abs() as u32;
    if significand == 0 {
        return signed_zero(sign);
    }
    let width = 32 - significand.leading_zeros() as i32;
    let mut exponent = max_exponent + width - significand_width;
    if width > significand_width {
        significand >>= (width - significand_width) as u32;
    } else {
        significand <<= (significand_width - width) as u32;
    }
    if exponent < FP32_MIN_NONZERO_EXPONENT {
        let shift = FP32_MIN_NONZERO_EXPONENT - exponent;
        significand = if shift >= 32 { 0 } else { significand >> shift };
        exponent = FP32_MIN_NONZERO_EXPONENT;
    }
    significand = if is_neg_shift { significand << diff } else { significand >> diff };
    if significand == 0 {
        return signed_zero(sign);
    }
    GFloat { sign, exponent, significand }
}

/// B200 (`matmul_fp8_windowed` at width 26, group 32), for contrast only.
pub fn b200_matmul_fp8(a: &[u8], b: &[u8], acc: Option<&[f32]>, m: usize, n: usize, k: usize) -> Vec<f32> {
    let zero_exp = -139;
    let group_size = MMA_GROUP_PRODUCTS + 1;
    let mut out = vec![0.0; m * n];
    let mut accumulator = Vec::with_capacity(group_size);
    for i in 0..m {
        for j in 0..n {
            accumulator.clear();
            accumulator.push(GFloat::from(acc.map_or(0.0, |acc| acc[i * n + j])));
            for (a_val, b_val) in a[i * k..(i + 1) * k].iter().zip(&b[j * k..(j + 1) * k]) {
                accumulator.push(multiply_fp8_to_gfloat(*a_val, *b_val, zero_exp));
                if accumulator.len() == group_size {
                    let collapsed: f32 = windowed_group_sum(&accumulator, zero_exp, B200_FP8_WIDTH).into();
                    accumulator.clear();
                    accumulator.push(GFloat::from(collapsed));
                }
            }
            out[i * n + j] = windowed_group_sum(&accumulator, zero_exp, B200_FP8_WIDTH).into();
        }
    }
    out
}

// ---- RDNA4 ---------------------------------------------------------------

/// `v * 2^shift`, rounded towards negative infinity when `shift < 0` (the
/// hardware aligns two's-complement terms and drops the shifted-out bits).
fn shl_floor(v: i64, shift: i32) -> i64 {
    if shift >= 0 {
        v << shift
    } else if shift <= -63 {
        if v < 0 { -1 } else { 0 }
    } else {
        v >> -shift
    }
}

/// `s * 2^base` rounded to FP32, round to nearest, ties to even (with FP32
/// subnormals). `s != 0`.
fn round_fp32_rne(s: i64, base: i32) -> f32 {
    let mag = s.unsigned_abs();
    let top = 63 - mag.leading_zeros() as i32 + base;
    let ulp = (top - 23).max(-149);
    let shift = ulp - base;
    let mut q = if shift <= 0 {
        mag << -shift
    } else if shift >= 64 {
        0
    } else {
        mag >> shift
    };
    if shift > 0 && shift <= 64 {
        let rem = if shift == 64 { mag } else { mag - (q << shift) };
        let half = 1u64 << (shift - 1);
        if rem > half || (rem == half && q & 1 == 1) {
            q += 1;
        }
    }
    // q <= 2^24 and ulp >= -149, so the f64 is an exactly representable f32.
    let value = (q as f64 * 2f64.powi(ulp)) as f32;
    check_not_nan_or_inf_f32(value);
    if s < 0 { -value } else { value }
}

/// One RDNA4 accumulation group: the FP32 carry `c` plus up to 8 exact
/// products (Pearl `GFloat`s: `|p| = significand * 2^(exponent - 23)`, with
/// the stored, unnormalized exponent).
///
/// 1. Each quad (products 0-3 and 4-7) is anchored at its own largest
///    product exponent `eta_q`; every product is floored to `2^(eta_q - 24)`
///    and the quad is summed exactly.
/// 2. The group anchor is `eta = max(max product exponent, e(c) - 2)`. Each
///    quad sum is floored to `2^(eta - 24)`. The carry is floored to the same
///    grid, or dropped when `e(c) < eta - 24` (it lies wholly below the grid).
/// 3. The exact sum is rounded once to FP32, nearest-even; zero is `+0`.
///
/// A group without a nonzero product returns `c`, except that `-0` becomes `+0`.
pub fn rdna4_group(c: f32, products: &[GFloat]) -> f32 {
    assert!(products.len() <= RDNA4_GROUP_PRODUCTS);
    let Some(product_eta) = products.iter().filter(|p| p.significand != 0).map(|p| p.exponent).max() else {
        return if c == 0.0 { 0.0 } else { c };
    };
    let carry = GFloat::from(c);
    let eta = if carry.significand != 0 { product_eta.max(carry.exponent - RDNA4_CARRY_ANCHOR_OFFSET) } else { product_eta };
    let signed = |g: &GFloat| if g.sign { -(g.significand as i64) } else { g.significand as i64 };

    // All sums are integers in units of 2^(eta - 24).
    let mut sum: i64 = 0;
    for quad in products.chunks(RDNA4_QUAD_PRODUCTS) {
        let Some(quad_eta) = quad.iter().filter(|p| p.significand != 0).map(|p| p.exponent).max() else {
            continue;
        };
        let quad_sum: i64 = quad
            .iter()
            .filter(|p| p.significand != 0)
            .map(|p| shl_floor(signed(p), p.exponent - 23 - (quad_eta - RDNA4_WINDOW_BITS)))
            .sum();
        sum += shl_floor(quad_sum, quad_eta - eta);
    }
    if carry.significand != 0 && carry.exponent >= eta - RDNA4_WINDOW_BITS {
        sum += shl_floor(signed(&carry), carry.exponent - 23 - (eta - RDNA4_WINDOW_BITS));
    }
    if sum == 0 {
        return 0.0;
    }
    round_fp32_rne(sum, eta - RDNA4_WINDOW_BITS)
}

/// `MatMul_RDNA4`: `a` is `m x k`, `b` is `n x k` (row-major, `b` transposed),
/// `acc` the optional `m x n` carry-in. Each cell chains `rdna4_group` over
/// ascending 8-product groups of `k` (a trailing partial group behaves as if
/// zero-padded), with no intermediate promotion.
pub fn rdna4_matmul_fp8(a: &[u8], b: &[u8], acc: Option<&[f32]>, m: usize, n: usize, k: usize) -> Vec<f32> {
    assert_eq!(a.len(), m * k);
    assert_eq!(b.len(), n * k);
    let zero_exp = -139;
    let mut out = vec![0.0; m * n];
    let mut products = Vec::with_capacity(RDNA4_GROUP_PRODUCTS);
    for i in 0..m {
        for j in 0..n {
            let mut c = acc.map_or(0.0, |acc| acc[i * n + j]);
            check_not_nan_or_inf_f32(c);
            let (a_row, b_row) = (&a[i * k..(i + 1) * k], &b[j * k..(j + 1) * k]);
            for (a_group, b_group) in a_row.chunks(RDNA4_GROUP_PRODUCTS).zip(b_row.chunks(RDNA4_GROUP_PRODUCTS)) {
                products.clear();
                products.extend(a_group.iter().zip(b_group).map(|(&x, &y)| multiply_fp8_to_gfloat(x, y, zero_exp)));
                c = rdna4_group(c, &products);
            }
            out[i * n + j] = c;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rdna4Vector {
        m: usize,
        n: usize,
        k: usize,
        a: &'static [u8],
        b: &'static [u8],
        c: Option<&'static [u32]>,
        expected: &'static [u32],
        b200: &'static [u32],
    }

    include!("rdna4_fp8_vectors.rs");

    fn bits(v: &[f32]) -> Vec<u32> {
        v.iter().map(|x| x.to_bits()).collect()
    }

    fn run(v: &Rdna4Vector, f: fn(&[u8], &[u8], Option<&[f32]>, usize, usize, usize) -> Vec<f32>) -> Vec<u32> {
        let c: Option<Vec<f32>> = v.c.map(|c| c.iter().map(|&u| f32::from_bits(u)).collect());
        bits(&f(v.a, v.b, c.as_deref(), v.m, v.n, v.k))
    }

    #[test]
    fn rdna4_matches_silicon_vectors() {
        for (idx, v) in RDNA4_FP8_VECTORS.iter().enumerate() {
            assert_eq!(run(v, rdna4_matmul_fp8), v.expected, "vector {idx}");
        }
    }

    #[test]
    fn b200_model_differs_from_rdna4_silicon() {
        for (idx, v) in RDNA4_FP8_VECTORS.iter().enumerate() {
            let b200 = run(v, b200_matmul_fp8);
            assert_eq!(b200, v.b200, "vector {idx}: B200 contrast column");
            assert_ne!(b200, v.expected, "vector {idx} does not separate the devices");
        }
    }

    /// Place (a, b) code pairs at the given k positions of a 16-deep cell.
    fn cell(pairs: &[(usize, u8, u8)], carry: f32) -> u32 {
        let (mut a, mut b) = (vec![0u8; 16], vec![0u8; 16]);
        for &(k, x, y) in pairs {
            (a[k], b[k]) = (x, y);
        }
        rdna4_matmul_fp8(&a, &b, Some(&[carry]), 1, 1, 16)[0].to_bits()
    }

    // E4M3 codes used below.
    const ONE: u8 = 0x38; // 1.0
    const P128: u8 = 0x70; // 128
    const M112: u8 = 0xEE; // -112
    const P2M9: u8 = 0x01; // 2^-9 (min subnormal)
    const M2M9: u8 = 0x81; // -2^-9

    #[test]
    fn negative_terms_floor_against_the_quad_anchor() {
        // 2^14 and -2^-18 in one quad: the grid is 2^(14-24), so -2^-18 floors
        // to -2^-10 and 16384 - 2^-10 is exactly representable.
        assert_eq!(cell(&[(0, P128, P128), (1, M2M9, P2M9)], 0.0), (16384.0f32 - 2f32.powi(-10)).to_bits());
        // A positive term below the grid is dropped.
        assert_eq!(cell(&[(0, P128, P128), (1, P2M9, P2M9)], 0.0), 16384.0f32.to_bits());
    }

    #[test]
    fn quads_are_anchored_separately() {
        // x = -2^-11 and y = 3 * 2^-11, which a 2^14 anchor would floor to -2^-10
        // and +2^-10 (sum 0). 16384 - 14336 = 2048 keeps every result exact.
        let x = (5, 0x88, 0x10); // -2^-6 * 2^-5 = -2^-11 at k = 5, quad 1
        let y = (6, 0x0C, 0x18); // 1.5 * 2^-6 * 2^-4 = 3 * 2^-11 at k = 6, quad 1
        let big = [(0, P128, P128), (1, M112, P128)]; // quad 0: 16384 - 14336
        // Bigs in the other quad: quad 1 sums exactly to 2^-10, which lies on the grid.
        assert_eq!(cell(&[big[0], big[1], x, y], 0.0), (2048.0f32 + 2f32.powi(-10)).to_bits());
        // Same quad as the bigs: x and y are floored individually and cancel.
        let (x, y) = ((2, x.1, x.2), (3, y.1, y.2));
        assert_eq!(cell(&[big[0], big[1], x, y], 0.0), 2048.0f32.to_bits());
    }

    #[test]
    fn each_instruction_is_two_chained_groups() {
        // +B, -B and a tiny x: x survives only when both B terms sit in an
        // earlier group (k 0-7) than x (k 8-15).
        let tiny = (12, 0x08, 0x08); // 2^-6 * 2^-6 = 2^-12
        assert_eq!(cell(&[(0, P128, P128), (1, P128 | 0x80, P128), tiny], 0.0), 2f32.powi(-12).to_bits());
        assert_eq!(cell(&[(8, P128, P128), (9, P128 | 0x80, P128), (2, 0x08, 0x08)], 0.0), 0);
    }

    #[test]
    fn carry_below_the_grid_is_dropped_not_floored() {
        // Products 36 + 2^-18 (odd last bit), grid 2^(5-24). A carry of
        // -2^-20 lies wholly below the grid and is ignored.
        let pairs = [(0, 0x51, 0x48), (4, P2M9, P2M9)]; // 9 * 4 = 36; 2^-9 * 2^-9
        let base = 36.0f32 + 2f32.powi(-18);
        assert_eq!(cell(&pairs, -2f32.powi(-20)), base.to_bits());
        // -2^-19 is on the grid: 36 + 2^-19 is a tie and rounds to even (36).
        assert_eq!(cell(&pairs, -2f32.powi(-19)), 36.0f32.to_bits());
    }

    #[test]
    fn carry_anchor_is_its_exponent_minus_two() {
        // c = 2^24 dominates a 1.5 product: the grid is 2^(24-2-24) = 2^-2 and
        // 2^24 + 1.5 rounds to nearest-even (16777218).
        assert_eq!(cell(&[(0, 0x3C, ONE)], 16777216.0), 16777218.0f32.to_bits());
    }

    #[test]
    fn signed_zero_rules() {
        // -0 carry with only zero products -> +0.
        assert_eq!(cell(&[(0, 0x80, ONE)], -0.0), 0);
        // Exact cancellation -> +0, even from a -0 carry.
        assert_eq!(cell(&[(0, ONE, ONE), (1, ONE | 0x80, ONE)], -0.0), 0);
        // Nonzero carry with only zero products passes through bit-exactly.
        assert_eq!(cell(&[], -1.5e-30), (-1.5e-30f32).to_bits());
    }
}
