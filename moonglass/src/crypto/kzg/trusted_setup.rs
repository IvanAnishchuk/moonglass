//! Ethereum c-kzg trusted setup parser.
//!
//! The only supported setup format is `trusted_setup.txt` from
//! `ethereum/c-kzg-4844`.

use std::{path::Path, str};

use ark_ec::{AffineRepr, pairing::Pairing};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize, Compress};
use thiserror::Error;

pub(crate) const ETHEREUM_TRUSTED_SETUP_TEXT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/trusted_setup.txt"
));

const TEXT_HEADER_LINES: usize = 2;

type SetupTextLine<'a> = (usize, &'a str);

/// G1 and G2 monomial powers of tau.
pub type PowersOfTau<E> = (Vec<<E as Pairing>::G1Affine>, Vec<<E as Pairing>::G2Affine>);

/// Ethereum c-kzg `trusted_setup.txt`.
///
/// The file starts with two counts:
/// - `n`, the number of G1 points in both bases
/// - `m`, the number of G2 monomial points
///
/// The remaining lines are ordered as:
/// - `L_i = [ell_i(tau)]_1`, the G1 lagrange-basis points
/// - `T_i = [tau^i]_2`, the G2 monomial powers
/// - `M_i = [tau^i]_1`, the G1 monomial powers
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EthereumTrustedSetup<E: Pairing> {
    g1_lagrange: Vec<E::G1Affine>,
    g2_monomial: Vec<E::G2Affine>,
    g1_monomial: Vec<E::G1Affine>,
}

impl<E: Pairing> EthereumTrustedSetup<E> {
    /// Parse Ethereum c-kzg `trusted_setup.txt` contents.
    pub fn parse(text: &str) -> Result<Self, SetupFileError> {
        let lines = setup_text_lines(text);
        if lines.len() < TEXT_HEADER_LINES {
            return Err(SetupFileError::InvalidTextLineCount {
                expected: TEXT_HEADER_LINES,
                got: lines.len(),
            });
        }

        let (g1_count_line, g1_count_text) = lines[0];
        let (g2_count_line, g2_count_text) = lines[1];
        let g1_count = parse_text_count(g1_count_text, g1_count_line)?;
        let g2_count = parse_text_count(g2_count_text, g2_count_line)?;

        let expected_lines = g1_count
            .checked_mul(2)
            .and_then(|count| count.checked_add(g2_count))
            .and_then(|count| count.checked_add(TEXT_HEADER_LINES))
            .ok_or(SetupFileError::SizeOverflow)?;
        if lines.len() != expected_lines {
            return Err(SetupFileError::InvalidTextLineCount {
                expected: expected_lines,
                got: lines.len(),
            });
        }

        let g1_lagrange_start = TEXT_HEADER_LINES;
        let g2_monomial_start = g1_lagrange_start
            .checked_add(g1_count)
            .ok_or(SetupFileError::SizeOverflow)?;
        let g1_monomial_start = g2_monomial_start
            .checked_add(g2_count)
            .ok_or(SetupFileError::SizeOverflow)?;
        let g1_monomial_end = g1_monomial_start
            .checked_add(g1_count)
            .ok_or(SetupFileError::SizeOverflow)?;

        // L_i = [ell_i(tau)]_1. These are for evaluation-form blob commitments.
        let g1_lagrange = parse_g1_range::<E>(&lines, g1_lagrange_start..g2_monomial_start)?;

        // T_i = [tau^i]_2. In particular, T_1 = [tau]_2.
        let g2_monomial = parse_g2_range::<E>(&lines, g2_monomial_start..g1_monomial_start)?;

        // M_i = [tau^i]_1. Coefficient-form KZG commitments use these powers:
        // C = sum_i p_i M_i.
        let g1_monomial = parse_g1_range::<E>(&lines, g1_monomial_start..g1_monomial_end)?;

        Ok(Self {
            g1_lagrange,
            g2_monomial,
            g1_monomial,
        })
    }

    /// G1 lagrange-basis powers.
    #[must_use]
    pub fn g1_lagrange_powers(&self) -> &[E::G1Affine] {
        &self.g1_lagrange
    }

    /// G1 monomial powers `[tau^i]_1`.
    #[must_use]
    pub fn g1_monomial_powers(&self) -> &[E::G1Affine] {
        &self.g1_monomial
    }

    /// G2 monomial powers `[tau^i]_2`.
    #[must_use]
    pub fn g2_monomial_powers(&self) -> &[E::G2Affine] {
        &self.g2_monomial
    }

    /// Return the monomial powers used by KZG commitment/opening code.
    #[must_use]
    pub fn into_monomial_powers(self) -> PowersOfTau<E> {
        (self.g1_monomial, self.g2_monomial)
    }
}

/// Return G1 and G2 monomial powers from an Ethereum c-kzg setup file.
pub fn get_powers_from_file<E: Pairing>(
    path: impl AsRef<Path>,
) -> Result<PowersOfTau<E>, SetupFileError> {
    let file_data =
        std::fs::read(path.as_ref()).map_err(|e| SetupFileError::FileError(e.to_string()))?;
    get_powers_from_bytes::<E>(&file_data)
}

/// Return G1 and G2 monomial powers from Ethereum c-kzg setup bytes.
pub fn get_powers_from_bytes<E: Pairing>(
    file_data: &[u8],
) -> Result<PowersOfTau<E>, SetupFileError> {
    let text = str::from_utf8(file_data).map_err(|e| SetupFileError::InvalidUtf8(e.to_string()))?;
    get_powers_from_text::<E>(text)
}

/// Return G1 and G2 monomial powers from Ethereum c-kzg setup text.
pub fn get_powers_from_text<E: Pairing>(text: &str) -> Result<PowersOfTau<E>, SetupFileError> {
    Ok(EthereumTrustedSetup::<E>::parse(text)?.into_monomial_powers())
}

fn setup_text_lines(text: &str) -> Vec<SetupTextLine<'_>> {
    text.lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim();
            (!trimmed.is_empty()).then_some((index + 1, trimmed))
        })
        .collect()
}

fn parse_text_count(text: &str, line: usize) -> Result<usize, SetupFileError> {
    text.parse::<usize>()
        .map_err(|e| SetupFileError::InvalidTextCount {
            line,
            value: text.to_owned(),
            reason: e.to_string(),
        })
}

fn parse_g1_range<E: Pairing>(
    lines: &[SetupTextLine<'_>],
    range: std::ops::Range<usize>,
) -> Result<Vec<E::G1Affine>, SetupFileError> {
    let element_size = E::G1Affine::generator().serialized_size(Compress::Yes);
    let mut powers = Vec::with_capacity(range.len());

    for index in range {
        let (line, text) = lines[index];
        let bytes = parse_hex_line(text, line, element_size)?;
        let element = E::G1Affine::deserialize_compressed(bytes.as_slice()).map_err(|e| {
            SetupFileError::TextPointParse {
                line,
                reason: e.to_string(),
            }
        })?;
        powers.push(element);
    }

    Ok(powers)
}

fn parse_g2_range<E: Pairing>(
    lines: &[SetupTextLine<'_>],
    range: std::ops::Range<usize>,
) -> Result<Vec<E::G2Affine>, SetupFileError> {
    let element_size = E::G2Affine::generator().serialized_size(Compress::Yes);
    let mut powers = Vec::with_capacity(range.len());

    for index in range {
        let (line, text) = lines[index];
        let bytes = parse_hex_line(text, line, element_size)?;
        let element = E::G2Affine::deserialize_compressed(bytes.as_slice()).map_err(|e| {
            SetupFileError::TextPointParse {
                line,
                reason: e.to_string(),
            }
        })?;
        powers.push(element);
    }

    Ok(powers)
}

fn parse_hex_line(
    text: &str,
    line: usize,
    expected_bytes: usize,
) -> Result<Vec<u8>, SetupFileError> {
    let text = text.strip_prefix("0x").unwrap_or(text);
    let expected = expected_bytes
        .checked_mul(2)
        .ok_or(SetupFileError::SizeOverflow)?;
    if text.len() != expected {
        return Err(SetupFileError::InvalidHexLength {
            line,
            expected,
            got: text.len(),
        });
    }

    let mut bytes = Vec::with_capacity(expected_bytes);
    for pair in text.as_bytes().chunks_exact(2) {
        let high = hex_value(pair[0], line)?;
        let low = hex_value(pair[1], line)?;
        bytes.push((high << 4) | low);
    }
    Ok(bytes)
}

fn hex_value(byte: u8, line: usize) -> Result<u8, SetupFileError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(SetupFileError::InvalidHexCharacter {
            line,
            value: char::from(byte),
        }),
    }
}

/// Trusted-setup parsing failures.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SetupFileError {
    /// Required `[tau]_2` point is missing.
    #[error("trusted setup does not include [tau]_2")]
    MissingTauG2,

    /// File IO failed.
    #[error("file error: {0}")]
    FileError(String),

    /// Text setup is not UTF-8.
    #[error("trusted setup text is not UTF-8: {0}")]
    InvalidUtf8(String),

    /// Text setup has the wrong number of non-empty lines.
    #[error("trusted setup text has {got} lines, expected {expected}")]
    InvalidTextLineCount { expected: usize, got: usize },

    /// Text setup count line could not be parsed.
    #[error("invalid trusted setup count on line {line}: {value:?}: {reason}")]
    InvalidTextCount {
        line: usize,
        value: String,
        reason: String,
    },

    /// A compressed point hex line has the wrong length.
    #[error("line {line}: expected {expected} hex chars, got {got}")]
    InvalidHexLength {
        line: usize,
        expected: usize,
        got: usize,
    },

    /// A compressed point hex line contains a non-hex character.
    #[error("line {line}: invalid hex character {value:?}")]
    InvalidHexCharacter { line: usize, value: char },

    /// Curve point parsing failed on a setup line.
    #[error("line {line}: point parse error: {reason}")]
    TextPointParse { line: usize, reason: String },

    /// Size arithmetic overflowed.
    #[error("setup size overflow")]
    SizeOverflow,
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fmt::Write;

    use ark_bls12_381::{Bls12_381, Fr};
    use ark_ec::{AffineRepr, CurveGroup, pairing::Pairing};
    use ark_ff::Field;

    #[test]
    fn ethereum_text_setup_returns_monomial_powers() {
        let g1_generator = <Bls12_381 as Pairing>::G1Affine::generator();
        let g2_generator = <Bls12_381 as Pairing>::G2Affine::generator();
        let secret = Fr::from(17_u64);
        let mut tau_power = Fr::ONE;
        let mut g1_monomial = Vec::with_capacity(4);
        let mut g2_monomial = Vec::with_capacity(2);

        for _ in 0..4 {
            g1_monomial.push((g1_generator * tau_power).into_affine());
            tau_power *= secret;
        }

        tau_power = Fr::ONE;
        for _ in 0..2 {
            g2_monomial.push((g2_generator * tau_power).into_affine());
            tau_power *= secret;
        }

        let g1_lagrange: Vec<_> = g1_monomial.iter().rev().copied().collect();
        let mut lines = vec![g1_monomial.len().to_string(), g2_monomial.len().to_string()];
        lines.extend(g1_lagrange.iter().map(compressed_hex));
        lines.extend(g2_monomial.iter().map(compressed_hex));
        lines.extend(g1_monomial.iter().map(compressed_hex));
        let text = format!("{}\n", lines.join("\n"));

        let parsed = EthereumTrustedSetup::<Bls12_381>::parse(&text).unwrap();
        assert_eq!(parsed.g1_lagrange_powers(), g1_lagrange.as_slice());
        assert_eq!(parsed.g2_monomial_powers(), g2_monomial.as_slice());
        assert_eq!(parsed.g1_monomial_powers(), g1_monomial.as_slice());

        let (g1_powers, g2_powers) = get_powers_from_text::<Bls12_381>(&text).unwrap();
        assert_eq!(g1_powers, g1_monomial);
        assert_eq!(g2_powers, g2_monomial);
    }

    #[test]
    fn ethereum_text_setup_rejects_wrong_line_count() {
        assert_eq!(
            EthereumTrustedSetup::<Bls12_381>::parse("1\n1\n"),
            Err(SetupFileError::InvalidTextLineCount {
                expected: 5,
                got: 2,
            })
        );
    }

    fn compressed_hex<T: CanonicalSerialize>(element: &T) -> String {
        let mut bytes = Vec::new();
        element.serialize_compressed(&mut bytes).unwrap();

        let mut out = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            write!(&mut out, "{byte:02x}").unwrap();
        }
        out
    }
}
