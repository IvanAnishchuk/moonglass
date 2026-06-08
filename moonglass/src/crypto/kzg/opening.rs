//! Single-point KZG commitments, openings, and verification.

use ark_ec::{AffineRepr, VariableBaseMSM, pairing::Pairing};
use ark_ff::{FftField, Field};
use ark_poly::{DenseUVPolynomial, Polynomial, univariate::DensePolynomial};

use super::{KzgError, KzgSetup, error::ensure_supported};

/// Commit to a dense polynomial in coefficient form.
///
/// The KZG commitment is `C = [p(tau)]_1`.
pub fn commit<E>(
    setup: &KzgSetup<E>,
    polynomial: &DensePolynomial<E::ScalarField>,
) -> Result<E::G1, KzgError>
where
    E: Pairing,
{
    // p(X) = sum_i p_i X^i, so the coefficient slice is the sequence p_i.
    let coefficients = polynomial.coeffs();
    ensure_supported(coefficients.len(), setup.g1_affine_powers.len())?;

    // C = [p(tau)]_1 = sum_i p_i [tau^i]_1.
    Ok(E::G1::msm_unchecked(
        &setup.g1_affine_powers[..coefficients.len()],
        coefficients,
    ))
}

/// Open a polynomial at `point`.
///
/// For `z = point`, the proof is `pi = [q(tau)]_1` where
/// `q(X) = (p(X) - p(z)) / (X - z)`.
pub fn open<E>(
    setup: &KzgSetup<E>,
    polynomial: &DensePolynomial<E::ScalarField>,
    point: &E::ScalarField,
) -> Result<E::G1, KzgError>
where
    E: Pairing,
    E::ScalarField: FftField,
{
    ensure_supported(polynomial.coeffs().len(), setup.g1_affine_powers.len())?;

    // y = p(z).
    let value = polynomial.evaluate(point);

    // The numerator is n(X) = p(X) - y.
    let value_polynomial = DensePolynomial::from_coefficients_slice(&[value]);
    let numerator = polynomial - &value_polynomial;

    // The denominator is d(X) = X - z.
    let denominator = DensePolynomial::from_coefficients_slice(&[-*point, E::ScalarField::ONE]);

    // The witness polynomial is q(X) = n(X) / d(X).
    let quotient = &numerator / &denominator;

    // The opening proof is pi = [q(tau)]_1.
    commit(setup, &quotient)
}

/// Verify an opening proof for a polynomial commitment at `point`.
///
/// Checks `e(C - [y]_1, [1]_2) = e(pi, [tau - z]_2)`.
pub fn verify<E>(
    setup: &KzgSetup<E>,
    commitment: E::G1,
    point: E::ScalarField,
    value: E::ScalarField,
    proof: E::G1,
) -> Result<bool, KzgError>
where
    E: Pairing,
{
    // [y]_1 = y * [1]_1.
    let value_g1 = E::G1Affine::generator() * value;

    // [z]_2 = z * [1]_2.
    let point_g2 = E::G2Affine::generator() * point;

    // Left side: e(C - [y]_1, [1]_2) = e([p(tau) - y]_1, [1]_2).
    let lhs = E::pairing(commitment - value_g1, E::G2Affine::generator());

    // Right side: e(pi, [tau]_2 - [z]_2) = e([q(tau)]_1, [tau - z]_2).
    let rhs = E::pairing(proof, setup.tau_g2 - point_g2);

    Ok(lhs == rhs)
}
