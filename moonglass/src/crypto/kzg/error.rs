//! Error types shared by KZG operations.

use thiserror::Error;

/// KZG operation failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum KzgError {
    /// Polynomial has more coefficients than the trusted-setup G1 powers cover.
    #[error(
        "polynomial has {coefficients} coefficients but setup only has {setup_powers} G1 powers"
    )]
    PolynomialTooLarge {
        coefficients: usize,
        setup_powers: usize,
    },

    /// FK opening was requested on an empty polynomial.
    #[error("FK opening requires a non-empty polynomial")]
    EmptyPolynomial,

    /// FK evaluation domain size does not match the polynomial length.
    #[error("FK domain size {domain_size} does not match {coefficients} coefficients")]
    DomainSizeMismatch {
        coefficients: usize,
        domain_size: usize,
    },

    /// Doubling the polynomial length to derive the FK domain overflowed `usize`.
    #[error("FK domain size overflow for {coefficients} coefficients")]
    DomainSizeOverflow { coefficients: usize },

    /// The scalar field does not support a radix-2 domain of the requested size.
    #[error("unsupported radix-2 domain size {0}")]
    UnsupportedDomainSize(usize),
}

pub(super) fn ensure_supported(coefficients: usize, setup_powers: usize) -> Result<(), KzgError> {
    if coefficients > setup_powers {
        return Err(KzgError::PolynomialTooLarge {
            coefficients,
            setup_powers,
        });
    }
    Ok(())
}
