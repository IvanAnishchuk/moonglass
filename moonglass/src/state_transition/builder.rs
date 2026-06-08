//! Builder-market transition phases.
//!
//! Covers payload-bid acceptance, builder-payment quorum accounting,
//! payload-timeliness committee voting, builder exits and slashings, and the
//! per-slot pending-payment settlement.

mod bids;
mod lifecycle;
mod payload_attestations;
mod payments;
