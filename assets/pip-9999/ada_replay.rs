//! Reference replay for PIP-9999 (Ada FP8 device): `MatMul_ADA` and the
//! replay magnitudes `Z`/`M` the jackpot policy consumes.
//!
//! `GFloat`, `multiply_fp8_to_gfloat`, `windowed_group_sum`,
//! `finite_f32_binade`, `exact_f32_sum_binade` and the H100 replay loop are
//! copied from pearl-research-labs/pearl, zk-pow/src/v4/api/utils.rs
//! (branch fp8-scheme @ 684f7045; the copied arithmetic is unchanged on
//! v4-prover-optis @ 172bbd59). Only `anyhow` is replaced by panics, and
//! `is_multiple_of` by `%`, so the file builds alone. Copied code:
//! Copyright (c) 2025-2026 Pearl Research Labs, ISC License (see
//! LICENSE-PEARL-ISC.txt). Build and run:
//!
//!     rustc --edition 2021 --test ada_replay.rs -o ada_replay_test && ./ada_replay_test
//!
//! The vectors in `ada_fp8_vectors.rs` were measured on an RTX 4070 Ti SUPER.

const FP32_MIN_NONZERO_EXPONENT: i32 = -126;
const MMA_GROUP_PRODUCTS: usize = 32;
const H100_FP8_WIDTH: u16 = 14;
const H100_WINDOW_PRODUCTS: usize = 128;

/// Ada `mma.sync` collapses each 32-product atom in two groups of 16
/// products (k 0-15, then k 16-31), each at the Hopper window width.
const ADA_GROUP_PRODUCTS: usize = 16;

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
        if shift >= 32 {
            significand = 0;
        } else {
            significand >>= shift
        };
        exponent = FP32_MIN_NONZERO_EXPONENT;
    }
    significand = if is_neg_shift { significand << diff } else { significand >> diff };
    if significand == 0 {
        return signed_zero(sign);
    }
    GFloat { sign, exponent, significand }
}

fn finite_f32_binade(value: f32) -> Option<i32> {
    check_not_nan_or_inf_f32(value);
    let bits = value.to_bits() & 0x7FFF_FFFF;
    if bits == 0 {
        return None;
    }
    let exp = ((bits >> 23) & 0xFF) as i32;
    if exp == 0 {
        Some((bits & 0x7F_FFFF).ilog2() as i32 - 149)
    } else {
        Some(exp - 127)
    }
}

/// Exact binade of the real sum of two finite f32 values, before FP32 rounding.
fn exact_f32_sum_binade(a: f32, b: f32) -> Option<i32> {
    let parts = |value: f32| {
        check_not_nan_or_inf_f32(value);
        let bits = value.to_bits();
        let negative = bits >> 31 != 0;
        let exp_field = ((bits >> 23) & 0xFF) as i32;
        let fraction = bits & 0x7F_FFFF;
        let (significand, exponent) = if exp_field == 0 {
            (fraction, -149)
        } else {
            ((1 << 23) | fraction, exp_field - 150)
        };
        let signed = if negative {
            -(significand as i128)
        } else {
            significand as i128
        };
        (signed, exponent)
    };

    let (a_sig, a_exp) = parts(a);
    let (b_sig, b_exp) = parts(b);
    if a_sig == 0 {
        return finite_f32_binade(b);
    }
    if b_sig == 0 {
        return finite_f32_binade(a);
    }
    let exponent = a_exp.min(b_exp);
    let a_shift = (a_exp - exponent) as u32;
    let b_shift = (b_exp - exponent) as u32;
    assert!(
        a_shift < 104 && b_shift < 104,
        "protocol replay exponent gap exceeds the i128 exact-sum frame"
    );
    let sum = (a_sig << a_shift) + (b_sig << b_shift);
    (sum != 0).then(|| exponent + sum.unsigned_abs().ilog2() as i32)
}

/// Exact binades consumed by the jackpot policy for one cell (H100PolicyExponents).
#[derive(Clone, Debug, PartialEq)]
pub struct PolicyExponents {
    /// One `e(Z)` per at-most-128-term window; `None` means zero.
    pub z_windows: Vec<Option<i32>>,
    /// Cell-wide `e(M)`, including every exact unrounded `total + local`.
    pub m_cell: Option<i32>,
}

/// H100::matmul_fp8_replay, Output mode.
fn h100_matmul_fp8(a: &[u8], b: &[u8], m: usize, n: usize, k: usize) -> Vec<f32> {
    replay(a, b, m, n, k, MMA_GROUP_PRODUCTS).0
}

/// H100::matmul_fp8_policy_replay.
fn h100_policy_replay(a: &[u8], b: &[u8], m: usize, n: usize, k: usize) -> (Vec<f32>, Vec<PolicyExponents>) {
    replay(a, b, m, n, k, MMA_GROUP_PRODUCTS)
}

// ---- proposed ------------------------------------------------------------

/// `MatMul_ADA`: the H100 replay (128-product windows, local accumulator
/// reset to +0, one FP32 RNE promotion per window) with every 16 products
/// collapsed as one group, i.e. each 32-product atom in two groups.
pub fn ada_matmul_fp8(a: &[u8], b: &[u8], m: usize, n: usize, k: usize) -> Vec<f32> {
    replay(a, b, m, n, k, ADA_GROUP_PRODUCTS).0
}

/// Ada output plus the policy magnitudes. `Z` and `M` see every group result,
/// including the first-half partial that Hopper never forms.
pub fn ada_policy_replay(a: &[u8], b: &[u8], m: usize, n: usize, k: usize) -> (Vec<f32>, Vec<PolicyExponents>) {
    replay(a, b, m, n, k, ADA_GROUP_PRODUCTS)
}

/// Shared replay loop; `group` = products per accumulator collapse
/// (H100: 32, the whole atom; Ada: 16, half an atom).
fn replay(a: &[u8], b: &[u8], m: usize, n: usize, k: usize, group: usize) -> (Vec<f32>, Vec<PolicyExponents>) {
    assert!(k > 0 && k % MMA_GROUP_PRODUCTS == 0, "replay requires positive k divisible by 32");
    assert_eq!(a.len(), m * k);
    assert_eq!(b.len(), n * k);
    let zero_exp = -139;
    let mut out = vec![0.0f32; m * n];
    let mut policy = Vec::with_capacity(m * n);
    for i in 0..m {
        let a_row = &a[i * k..(i + 1) * k];
        for j in 0..n {
            let b_row = &b[j * k..(j + 1) * k];
            let mut total = 0.0f32;
            let mut m_cell = None;
            let mut z_windows = Vec::with_capacity(k.div_ceil(H100_WINDOW_PRODUCTS));
            for window_start in (0..k).step_by(H100_WINDOW_PRODUCTS) {
                let window_end = (window_start + H100_WINDOW_PRODUCTS).min(k);
                let mut local = 0.0f32;
                let mut z_window = None;
                for group_start in (window_start..window_end).step_by(group) {
                    let mut values = Vec::with_capacity(group + 1);
                    values.push(GFloat::from(local));
                    for (&a, &b) in a_row[group_start..group_start + group].iter().zip(&b_row[group_start..group_start + group]) {
                        let product = multiply_fp8_to_gfloat(a, b, zero_exp);
                        // Products are unnormalized: |p| = significand * 2^(exponent - 23).
                        let exponent = (product.significand != 0)
                            .then(|| product.exponent + product.significand.ilog2() as i32 - 23);
                        z_window = z_window.max(exponent);
                        m_cell = m_cell.max(exponent);
                        values.push(product);
                    }
                    local = windowed_group_sum(&values, zero_exp, H100_FP8_WIDTH).into();
                    z_window = z_window.max(finite_f32_binade(local));
                    m_cell = m_cell.max(exact_f32_sum_binade(total, local));
                }
                z_windows.push(z_window);
                total += local;
                check_not_nan_or_inf_f32(total);
                if total == 0.0 {
                    total = 0.0;
                }
            }
            out[i * n + j] = total;
            policy.push(PolicyExponents { z_windows, m_cell });
        }
    }
    (out, policy)
}

/// Exact E4M3FN decode (codes 0x7F/0xFF are NaN and never passed).
fn fp8_e4m3_to_f32(code: u8) -> f32 {
    let exp = ((code >> 3) & 0x0F) as i32;
    let mant = (code & 0x07) as f32;
    let magnitude = if exp == 0 {
        mant * 2f32.powi(-9)
    } else {
        (8.0 + mant) * 2f32.powi(exp - 10)
    };
    if code & 0x80 != 0 { -magnitude } else { magnitude }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AdaVector {
        m: usize,
        n: usize,
        k: usize,
        a: &'static [u8],
        b: &'static [u8],
        expected: &'static [u32],
        h100: &'static [u32],
    }

    include!("ada_fp8_vectors.rs");

    fn bits(v: Vec<f32>) -> Vec<u32> {
        v.iter().map(|x| x.to_bits()).collect()
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    #[test]
    fn ada_replay_matches_rtx_4070_ti_super_silicon() {
        for (idx, v) in ADA_FP8_VECTORS.iter().enumerate() {
            assert_eq!(bits(ada_matmul_fp8(v.a, v.b, v.m, v.n, v.k)), v.expected, "vector {idx}");
        }
    }

    #[test]
    fn h100_replay_differs_from_ada_silicon() {
        for (idx, v) in ADA_FP8_VECTORS.iter().enumerate() {
            let h100 = bits(h100_matmul_fp8(v.a, v.b, v.m, v.n, v.k));
            assert_eq!(h100, v.h100, "vector {idx}: copied H100 replay");
            assert_ne!(h100, v.expected, "vector {idx} must discriminate Ada from H100");
        }
    }

    /// Pearl's own H100 unit vector (h100_replay_resets_every_four_atoms_and_promotes_in_fp32):
    /// the window reset and FP32 promotion carry over unchanged to the Ada replay.
    #[test]
    fn ada_replay_agrees_on_pearl_h100_promotion_vector() {
        let k = 1024;
        let mut a = vec![0u8; k];
        a[..128].fill(0x78);
        a[128..256].fill(0x38);
        let b = vec![0x38u8; k];
        assert_eq!(h100_matmul_fp8(&a, &b, 1, 1, k), vec![32_896.0]);
        assert_eq!(ada_matmul_fp8(&a, &b, 1, 1, k), vec![32_896.0]);
    }

    /// Pearl's h100_policy_replay_tracks_window_z_and_cell_m, against the copied loop.
    #[test]
    fn h100_policy_port_tracks_window_z_and_cell_m() {
        let k = 256;
        let a = vec![0x38u8; k];
        let b = vec![0x38u8; k];
        let (out, policy) = h100_policy_replay(&a, &b, 1, 1, k);
        assert_eq!(out, vec![256.0]);
        assert_eq!(policy[0].z_windows, vec![Some(7), Some(7)]);
        assert_eq!(policy[0].m_cell, Some(8));
        // Same totals on Ada: 1.0 products sum exactly at 14 bits either way.
        let (out, policy) = ada_policy_replay(&a, &b, 1, 1, k);
        assert_eq!(out, vec![256.0]);
        assert_eq!(policy[0].z_windows, vec![Some(7), Some(7)]);
        assert_eq!(policy[0].m_cell, Some(8));
    }

    /// Pearl's h100_policy_replay_tracks_exact_products_when_atoms_cancel. The
    /// two cancelling products share the first group, so Ada agrees.
    #[test]
    fn policy_port_tracks_exact_products_when_atoms_cancel() {
        let mut a = [0u8; MMA_GROUP_PRODUCTS];
        let mut b = [0u8; MMA_GROUP_PRODUCTS];
        for a_code in 0..=u8::MAX {
            if a_code & 0x7F == 0x7F {
                continue;
            }
            for b_code in 0..=u8::MAX {
                if b_code & 0x7F == 0x7F {
                    continue;
                }
                a[0] = a_code;
                a[1] = a_code;
                b[0] = b_code;
                b[1] = b_code ^ 0x80;
                let expected = finite_f32_binade(fp8_e4m3_to_f32(a_code) * fp8_e4m3_to_f32(b_code));
                for (out, policy) in [
                    h100_policy_replay(&a, &b, 1, 1, MMA_GROUP_PRODUCTS),
                    ada_policy_replay(&a, &b, 1, 1, MMA_GROUP_PRODUCTS),
                ] {
                    assert_eq!(out[0].to_bits(), 0, "a={a_code:#04x}, b={b_code:#04x}");
                    assert_eq!(policy[0].m_cell, expected, "M: a={a_code:#04x}, b={b_code:#04x}");
                    assert_eq!(policy[0].z_windows, vec![expected], "Z: a={a_code:#04x}, b={b_code:#04x}");
                }
            }
        }
    }

    /// Sixteen +1 products then sixteen -1 products: the atom sums to +0 on both
    /// devices, but Ada forms the partial 16 between its two groups. Z and M
    /// must see it (binade 4); Hopper's single collapse never does (binade 0).
    #[test]
    fn ada_policy_sees_the_first_group_partial() {
        let mut a = [0x38u8; MMA_GROUP_PRODUCTS];
        a[16..].fill(0xB8);
        let b = [0x38u8; MMA_GROUP_PRODUCTS];
        let (out, policy) = h100_policy_replay(&a, &b, 1, 1, MMA_GROUP_PRODUCTS);
        assert_eq!(out[0].to_bits(), 0);
        assert_eq!(policy[0], PolicyExponents { z_windows: vec![Some(0)], m_cell: Some(0) });
        let (out, policy) = ada_policy_replay(&a, &b, 1, 1, MMA_GROUP_PRODUCTS);
        assert_eq!(out[0].to_bits(), 0);
        assert_eq!(policy[0], PolicyExponents { z_windows: vec![Some(4)], m_cell: Some(4) });
    }

    /// The rank-32 noise atom of Pearl's device_quantization_pins_noise_alpha_beta_and_code
    /// (quantization.rs): one 32-product dot product from +0. This input does
    /// not separate the devices; Ada gives Hopper's bits.
    #[test]
    fn noise_atom_vector() {
        let e = hex("be3adfead867e55be469dd50e9e85c50dfdc64e2dfe4eae2e8d6596a6746dd65");
        let f = hex("c65268603369d96362e16a5f6ae6615fe6db43eae463e2d3eae55ae3bb5f54e3");
        assert_eq!(h100_matmul_fp8(&e, &f, 1, 1, 32)[0].to_bits(), 1161306112, "copied H100 replay");
        assert_eq!(ada_matmul_fp8(&e, &f, 1, 1, 32)[0].to_bits(), 1161306112);
    }

    /// One 32-product dot product from +0 that separates the devices, measured
    /// on the RTX 4070 Ti SUPER (suite a_gauss_c0, tile 0, row 0, column 2).
    #[test]
    fn single_atom_silicon_vector() {
        let a = hex("3234b828bbc59a38a62a3f1a3bb9a6bc36a9b6b4223a3b88aaba3b3c2b363bab");
        let b = hex("b0382a291c3da4a7b133bcb93c360eab3d39afb5ae123db5289737211d2ab9b5");
        assert_eq!(ada_matmul_fp8(&a, &b, 1, 1, 32)[0].to_bits(), 0xBFC2B000, "silicon");
        assert_eq!(h100_matmul_fp8(&a, &b, 1, 1, 32)[0].to_bits(), 0xBFC2A800);
    }
}
