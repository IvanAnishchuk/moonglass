//! Validator-flow constants: balance thresholds, hysteresis, churn limits,
//! and the withdrawal sweep + credential prefix bytes.

use crate::primitives::Gwei;

/// Minimum deposit accepted by the deposit contract.
pub const MIN_DEPOSIT_AMOUNT: Gwei = Gwei(1_000_000_000);

/// Quantum to which effective balance is rounded.
pub const EFFECTIVE_BALANCE_INCREMENT: Gwei = Gwei(1_000_000_000);

/// Balance required for a validator to activate.
pub const MIN_ACTIVATION_BALANCE: Gwei = Gwei(32_000_000_000);

/// Effective-balance cap for compounding validators.
pub const MAX_EFFECTIVE_BALANCE: Gwei = Gwei(2_048_000_000_000);

/// Balance below which a validator is auto-ejected.
pub const EJECTION_BALANCE: Gwei = Gwei(16_000_000_000);

/// Denominator of the hysteresis window guarding effective-balance updates.
pub const HYSTERESIS_QUOTIENT: u64 = 4;

/// Downward-step multiplier applied past the hysteresis threshold.
pub const HYSTERESIS_DOWNWARD_MULTIPLIER: u64 = 1;

/// Upward-step multiplier applied past the hysteresis threshold.
pub const HYSTERESIS_UPWARD_MULTIPLIER: u64 = 5;

/// Minimum exit churn per epoch, in gwei.
#[cfg(feature = "mainnet")]
pub const MIN_PER_EPOCH_CHURN_LIMIT: Gwei = Gwei(128_000_000_000);

/// Divisor controlling activation/exit churn as a fraction of active stake.
#[cfg(feature = "mainnet")]
pub const CHURN_LIMIT_QUOTIENT: u64 = 32_768;

/// Divisor controlling the consolidation rate.
#[cfg(feature = "mainnet")]
pub const CONSOLIDATION_CHURN_LIMIT_QUOTIENT: u64 = 65_536;

/// Maximum activation churn per epoch, in gwei.
#[cfg(feature = "mainnet")]
pub const MAX_PER_EPOCH_ACTIVATION_CHURN_LIMIT: Gwei = Gwei(256_000_000_000);

/// Maximum withdrawals included in a single execution payload.
#[cfg(feature = "mainnet")]
pub const MAX_WITHDRAWALS_PER_PAYLOAD: usize = 16;

/// Validators scanned per withdrawal sweep.
#[cfg(feature = "mainnet")]
pub const MAX_VALIDATORS_PER_WITHDRAWALS_SWEEP: u64 = 16_384;

/// Pending partial withdrawals dequeued per sweep.
#[cfg(feature = "mainnet")]
pub const MAX_PENDING_PARTIALS_PER_WITHDRAWALS_SWEEP: u64 = 8;

/// Pending deposits processed per epoch.
pub const MAX_PENDING_DEPOSITS_PER_EPOCH: u64 = 16;

/// Builders scanned per withdrawal sweep.
#[cfg(feature = "mainnet")]
pub const MAX_BUILDERS_PER_WITHDRAWALS_SWEEP: u64 = 16_384;

/// `WithdrawalRequest.amount` sentinel for a full validator exit.
pub const FULL_EXIT_REQUEST_AMOUNT: Gwei = Gwei(0);

/// Prefix byte marking a BLS withdrawal credential.
pub const BLS_WITHDRAWAL_PREFIX: u8 = 0x00;

/// Prefix byte marking an execution-address withdrawal credential.
pub const ETH1_ADDRESS_WITHDRAWAL_PREFIX: u8 = 0x01;

/// Prefix byte marking a compounding-validator withdrawal credential.
pub const COMPOUNDING_WITHDRAWAL_PREFIX: u8 = 0x02;

/// Prefix byte marking a builder withdrawal credential.
pub const BUILDER_WITHDRAWAL_PREFIX: u8 = 0x03;

// Minimal preset values used only for testing.

#[cfg(feature = "minimal")]
pub const MIN_PER_EPOCH_CHURN_LIMIT: Gwei = Gwei(64_000_000_000);

#[cfg(feature = "minimal")]
pub const CHURN_LIMIT_QUOTIENT: u64 = 16;

#[cfg(feature = "minimal")]
pub const CONSOLIDATION_CHURN_LIMIT_QUOTIENT: u64 = 32;

#[cfg(feature = "minimal")]
pub const MAX_PER_EPOCH_ACTIVATION_CHURN_LIMIT: Gwei = Gwei(128_000_000_000);

#[cfg(feature = "minimal")]
pub const MAX_WITHDRAWALS_PER_PAYLOAD: usize = 4;

#[cfg(feature = "minimal")]
pub const MAX_VALIDATORS_PER_WITHDRAWALS_SWEEP: u64 = 16;

#[cfg(feature = "minimal")]
pub const MAX_PENDING_PARTIALS_PER_WITHDRAWALS_SWEEP: u64 = 2;

#[cfg(feature = "minimal")]
pub const MAX_BUILDERS_PER_WITHDRAWALS_SWEEP: u64 = 16;
