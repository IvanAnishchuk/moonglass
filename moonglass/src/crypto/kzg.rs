//! KZG polynomial commitments and FK batch openings over arkworks pairings.
//!
//! The implementation keeps the equations visible where the terms are built:
//! - commitment: `C = [p(tau)]_1 = sum_i p_i [tau^i]_1`
//! - opening: `q(X) = (p(X) - p(z)) / (X - z)` and `pi = [q(tau)]_1`
//! - verification: `e(C - [p(z)]_1, [1]_2) = e(pi, [tau - z]_2)`
//!
//! Blob wrappers on top of these primitives are not yet exposed:
//! `blob_to_kzg_commitment`, `compute_kzg_proof`, `compute_blob_kzg_proof`,
//! `verify_kzg_proof`, `verify_blob_kzg_proof`, `verify_blob_kzg_proof_batch`.

mod error;
mod fk;
mod opening;
mod setup;
mod trusted_setup;

pub use error::KzgError;
pub use fk::open_fk;
pub use opening::{commit, open, verify};
pub use setup::{EthereumKzgSetup, KzgSetup};
pub use trusted_setup::{
    EthereumTrustedSetup, PowersOfTau, SetupFileError, get_powers_from_bytes, get_powers_from_file,
    get_powers_from_text,
};

#[cfg(test)]
mod tests {
    use super::*;

    use ark_bls12_381::{Bls12_381, Fr, G1Projective};
    use ark_ec::{AffineRepr, pairing::Pairing};
    use ark_ff::{Field, Zero};
    use ark_poly::{
        DenseUVPolynomial, EvaluationDomain, Polynomial, Radix2EvaluationDomain,
        univariate::DensePolynomial,
    };

    use crate::constants::FIELD_ELEMENTS_PER_BLOB;

    #[test]
    fn setup_generates_expected_powers() {
        let secret = Fr::from(17_u64);
        let setup = KzgSetup::<Bls12_381>::setup(secret, 4);
        let generator = <Bls12_381 as Pairing>::G1Affine::generator();
        let mut tau_power = Fr::ONE;

        for i in 0..4 {
            assert_eq!(setup.g1_powers[i], generator * tau_power);
            tau_power *= secret;
        }
        assert_eq!(
            setup.tau_g2,
            <Bls12_381 as Pairing>::G2Affine::generator() * secret
        );
    }

    #[test]
    fn commit_matches_manual_msm() {
        let setup = KzgSetup::<Bls12_381>::setup(Fr::from(17_u64), 4);
        let polynomial =
            DensePolynomial::from_coefficients_slice(&[Fr::from(1), Fr::from(3), Fr::from(2)]);

        let commitment = commit(&setup, &polynomial).unwrap();

        let mut expected = G1Projective::zero();
        for (i, coefficient) in polynomial.coeffs().iter().copied().enumerate() {
            expected += setup.g1_powers[i] * coefficient;
        }
        assert_eq!(commitment, expected);
    }

    #[test]
    fn open_and_verify_round_trip() {
        let setup = KzgSetup::<Bls12_381>::setup(Fr::from(17_u64), 8);
        let polynomial =
            DensePolynomial::from_coefficients_slice(&[Fr::from(1), Fr::from(3), Fr::from(2)]);
        let commitment = commit(&setup, &polynomial).unwrap();
        let point = Fr::from(5_u64);
        let value = polynomial.evaluate(&point);
        let proof = open(&setup, &polynomial, &point).unwrap();

        assert!(verify(&setup, commitment, point, value, proof).unwrap());
        assert!(!verify(&setup, commitment, Fr::from(6_u64), value, proof).unwrap());
    }

    #[test]
    fn polynomial_too_large_is_reported() {
        let setup = KzgSetup::<Bls12_381>::setup(Fr::from(17_u64), 2);
        let polynomial =
            DensePolynomial::from_coefficients_slice(&[Fr::from(1), Fr::from(3), Fr::from(2)]);

        assert_eq!(
            commit(&setup, &polynomial),
            Err(KzgError::PolynomialTooLarge {
                coefficients: 3,
                setup_powers: 2,
            })
        );
    }

    #[test]
    fn fk_openings_match_individual_openings() {
        let setup = KzgSetup::<Bls12_381>::setup(Fr::from(17_u64), 16);
        let polynomial = DensePolynomial::from_coefficients_slice(&[
            Fr::from(1),
            Fr::from(2),
            Fr::from(3),
            Fr::from(4),
        ]);
        let domain = Radix2EvaluationDomain::<Fr>::new(polynomial.coeffs().len()).unwrap();

        let proofs = open_fk(&setup, polynomial.coeffs(), &domain).unwrap();
        let expected: Vec<_> = domain
            .elements()
            .map(|root| open(&setup, &polynomial, &root).unwrap())
            .collect();

        assert_eq!(proofs, expected);
    }

    #[test]
    fn mainnet_setup_loads_vendored_trusted_setup() {
        let setup = EthereumKzgSetup::mainnet().unwrap();

        assert_eq!(setup.g1_affine_powers().len(), FIELD_ELEMENTS_PER_BLOB);
        assert_eq!(setup.g1_powers().len(), FIELD_ELEMENTS_PER_BLOB);
        assert!(!setup.tau_g2().is_zero());
    }
}
