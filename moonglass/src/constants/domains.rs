//! Signing domain constants.
//!
//! A signing domain separates message kinds and protocol versions before BLS
//! verification. That prevents a signature over one consensus message from
//! being replayed as a different message elsewhere.
//! Domains without container or handler support are intentionally absent.

use crate::primitives::DomainType;

/// BLS12-381 G2 hash-to-curve domain-separation tag for Ethereum consensus
/// signatures. Fixed by the IRTF BLS signature suite (`PoP` variant). Every
/// participant must hash messages with this exact byte string for signatures
/// to interoperate across independent implementations.
pub const BLS_DST: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_";

/// Domain for `BeaconBlock` proposer signatures.
pub const DOMAIN_BEACON_PROPOSER: DomainType = DomainType([0x00, 0x00, 0x00, 0x00]);

/// Domain for attester signatures.
pub const DOMAIN_BEACON_ATTESTER: DomainType = DomainType([0x01, 0x00, 0x00, 0x00]);

/// Domain for RANDAO reveals.
pub const DOMAIN_RANDAO: DomainType = DomainType([0x02, 0x00, 0x00, 0x00]);

/// Domain for deposit signatures.
pub const DOMAIN_DEPOSIT: DomainType = DomainType([0x03, 0x00, 0x00, 0x00]);

/// Domain for voluntary-exit signatures.
pub const DOMAIN_VOLUNTARY_EXIT: DomainType = DomainType([0x04, 0x00, 0x00, 0x00]);

/// Domain for sync-committee signatures.
pub const DOMAIN_SYNC_COMMITTEE: DomainType = DomainType([0x07, 0x00, 0x00, 0x00]);

/// Domain for BLS-to-execution credential-change signatures.
pub const DOMAIN_BLS_TO_EXECUTION_CHANGE: DomainType = DomainType([0x0A, 0x00, 0x00, 0x00]);

/// Domain for builder-bid signatures.
pub const DOMAIN_BEACON_BUILDER: DomainType = DomainType([0x0B, 0x00, 0x00, 0x00]);

/// Domain for payload-timeliness committee attestations.
pub const DOMAIN_PTC_ATTESTER: DomainType = DomainType([0x0C, 0x00, 0x00, 0x00]);

/// Domain for builder-deposit signatures.
///
/// A dedicated domain keeps validator deposit signatures and builder deposit
/// signatures from being replayed against the other deposit path.
pub const DOMAIN_BUILDER_DEPOSIT: DomainType = DomainType([0x0E, 0x00, 0x00, 0x00]);
