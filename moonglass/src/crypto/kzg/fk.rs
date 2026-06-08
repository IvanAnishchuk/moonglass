//! FK23 batch openings for KZG commitments.

use ark_ec::pairing::Pairing;
use ark_ff::{FftField, Zero};
use ark_poly::{EvaluationDomain, Radix2EvaluationDomain};

use super::{KzgError, KzgSetup, error::ensure_supported};

/// Compute all openings at the roots of unity in `domain` using FK23.
///
/// For `d` coefficients and setup powers `[tau^i]_1`, FK turns the witness
/// computation for all roots into one convolution and one domain FFT.
pub fn open_fk<E>(
    setup: &KzgSetup<E>,
    coefficients: &[E::ScalarField],
    domain: &Radix2EvaluationDomain<E::ScalarField>,
) -> Result<Vec<E::G1>, KzgError>
where
    E: Pairing,
    E::ScalarField: FftField,
{
    let d = coefficients.len();
    if d == 0 {
        return Err(KzgError::EmptyPolynomial);
    }
    ensure_supported(d, setup.g1_powers.len())?;
    if domain.size() != d {
        return Err(KzgError::DomainSizeMismatch {
            coefficients: d,
            domain_size: domain.size(),
        });
    }

    let domain_2d_size = d
        .checked_mul(2)
        .ok_or(KzgError::DomainSizeOverflow { coefficients: d })?;
    let domain_2d = Radix2EvaluationDomain::<E::ScalarField>::new(domain_2d_size)
        .ok_or(KzgError::UnsupportedDomainSize(domain_2d_size))?;

    // s = ([tau^(d-1)]_1, [tau^(d-2)]_1, ..., [tau]_1, [1]_1, 0, ..., 0).
    let mut reversed_setup = vec![E::G1::zero(); domain_2d_size];
    reversed_setup[..d].copy_from_slice(
        &setup.g1_powers[..d]
            .iter()
            .rev()
            .copied()
            .collect::<Vec<_>>(),
    );

    // a = (0, ..., 0, f_0, f_1, ..., f_(d-1)).
    let mut padded_coefficients = vec![E::ScalarField::zero(); domain_2d_size];
    padded_coefficients[d..].copy_from_slice(coefficients);

    // S = DFT_2d(s).
    let setup_evaluations = domain_2d.fft(&reversed_setup);

    // A = DFT_2d(a).
    let coefficient_evaluations = domain_2d.fft(&padded_coefficients);

    // H = S o A, where each group element S_i is scaled by field element A_i.
    let convolution_evaluations: Vec<E::G1> = setup_evaluations
        .iter()
        .zip(coefficient_evaluations)
        .map(|(setup_eval, coefficient_eval)| *setup_eval * coefficient_eval)
        .collect();

    // h = IDFT_2d(H). The first d entries encode the FK witness polynomial data.
    let convolution = domain_2d.ifft(&convolution_evaluations);

    // pi_j = DFT_d(h[0..d])_j gives the proof at the j-th root of unity.
    Ok(domain.fft(&convolution[..d]))
}
