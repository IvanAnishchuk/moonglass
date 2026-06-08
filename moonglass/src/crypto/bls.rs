//! Ethereum consensus BLS12-381 signature operations.
//!
//! Ethereum uses the "minimal-pubkey-size" ciphersuite:
//! public keys are in G1, signatures are in G2, and messages are hashed to G2.
//!
//! # Known gaps
//!
//! No `aggregate_verify` (multi-message, multi-key). The state transition
//! does not need it. Reference-test fixtures under `bls/aggregate_verify`
//! are skipped without it.
//!
//! No `sign`, `pop_verify`, or key-validate. Signing belongs to validator-duty
//! tooling, and proof-of-possession is not on the consensus hot path.

use ark_bls12_381::{Bls12_381, G1Affine, G1Projective, G2Affine, G2Projective, g2};
use ark_ec::{
    AffineRepr, CurveGroup,
    hashing::{HashToCurve, curve_maps::wb::WBMap, map_to_curve_hasher::MapToCurveBasedHasher},
    pairing::Pairing,
};
use ark_ff::{Zero, field_hashers::DefaultFieldHasher};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use sha2::Sha256;

use crate::constants::BLS_DST;
use crate::error::{SignatureError, TransitionError};
use crate::primitives::{BLSPubkey, BLSSignature, Root};

type EthG2Hasher =
    MapToCurveBasedHasher<G2Projective, DefaultFieldHasher<Sha256>, WBMap<g2::Config>>;

const G1_POINT_AT_INFINITY: [u8; 48] = {
    let mut bytes = [0u8; 48];
    bytes[0] = 0xC0;
    bytes
};

/// Verify a BLS12-381 signature under the Ethereum consensus scheme.
pub fn verify_signature(
    pubkey: &BLSPubkey,
    signing_root: Root,
    signature: &BLSSignature,
    on_fail: SignatureError,
) -> Result<(), TransitionError> {
    // PK in G1.
    let pk = parse_pubkey(pubkey, on_fail)?;

    // sigma in G2.
    let sig = parse_signature(signature, on_fail)?;

    // H(m) = hash_to_curve(m, DST) in G2.
    let message_hash = hash_to_g2(signing_root, on_fail)?;

    // Check e(PK, H(m)) = e([1]_1, sigma).
    verify_pairing(pk, message_hash, sig, on_fail)
}

/// Aggregate a set of BLS public keys into a single compressed pubkey.
pub fn aggregate_pubkeys(pubkeys: &[BLSPubkey]) -> Result<BLSPubkey, TransitionError> {
    if pubkeys.is_empty() {
        return Err(SignatureError::EmptyAggregatePubkeySet.into());
    }

    // APK = sum_i PK_i.
    let aggregate = aggregate_pubkey(pubkeys, SignatureError::AggregatePubkey)?;
    let mut bytes = [0u8; 48];
    aggregate
        .into_affine()
        .serialize_compressed(&mut bytes[..])
        .map_err(|_| SignatureError::AggregatePubkey)?;
    Ok(BLSPubkey(bytes))
}

/// Verify a single aggregate signature against same-message participant pubkeys.
pub fn fast_aggregate_verify(
    pubkeys: &[BLSPubkey],
    signing_root: Root,
    signature: &BLSSignature,
    on_fail: SignatureError,
) -> Result<(), TransitionError> {
    if pubkeys.is_empty() {
        if signature.is_g2_point_at_infinity() {
            return Ok(());
        }
        return Err(SignatureError::SyncInfinitySignatureRequired.into());
    }

    // APK = sum_i PK_i for all participants that signed the same message.
    let aggregate = aggregate_pubkey(pubkeys, on_fail)?.into_affine();

    // sigma is the aggregate signature in G2.
    let sig = parse_signature(signature, on_fail)?;

    // H(m) = hash_to_curve(m, DST) in G2.
    let message_hash = hash_to_g2(signing_root, on_fail)?;

    // Check e(APK, H(m)) = e([1]_1, sigma).
    verify_pairing(aggregate, message_hash, sig, on_fail)
}

fn parse_pubkey(pubkey: &BLSPubkey, on_fail: SignatureError) -> Result<G1Affine, TransitionError> {
    if pubkey.0 == G1_POINT_AT_INFINITY {
        return Err(on_fail.into());
    }

    let pk = G1Affine::deserialize_compressed(&pubkey.0[..]).map_err(|_| on_fail)?;
    if pk.is_zero() {
        return Err(on_fail.into());
    }
    Ok(pk)
}

fn parse_signature(
    signature: &BLSSignature,
    on_fail: SignatureError,
) -> Result<G2Affine, TransitionError> {
    G2Affine::deserialize_compressed(&signature.0[..]).map_err(|_| on_fail.into())
}

fn hash_to_g2(signing_root: Root, on_fail: SignatureError) -> Result<G2Affine, TransitionError> {
    let hasher = EthG2Hasher::new(BLS_DST).map_err(|_| on_fail)?;
    hasher.hash(&signing_root.0).map_err(|_| on_fail.into())
}

fn aggregate_pubkey(
    pubkeys: &[BLSPubkey],
    on_fail: SignatureError,
) -> Result<G1Projective, TransitionError> {
    // APK = PK_0 + PK_1 + ... + PK_n.
    let mut aggregate = G1Projective::zero();
    for pubkey in pubkeys {
        aggregate += parse_pubkey(pubkey, on_fail)?;
    }
    Ok(aggregate)
}

fn verify_pairing(
    pubkey: G1Affine,
    message_hash: G2Affine,
    signature: G2Affine,
    on_fail: SignatureError,
) -> Result<(), TransitionError> {
    // Left side: e(PK, H(m)).
    let lhs = Bls12_381::pairing(pubkey, message_hash);

    // Right side: e([1]_1, sigma).
    let rhs = Bls12_381::pairing(G1Affine::generator(), signature);

    // The signature is valid exactly when both target-group elements match.
    if lhs == rhs {
        Ok(())
    } else {
        Err(on_fail.into())
    }
}
