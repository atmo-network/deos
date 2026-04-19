//! Elegant Governance System
//!
//! Provides a unified, type-safe governance framework with clear separation
//! between emergency procedures, parameter management, and community governance.

#![allow(dead_code)]

use crate::{AccountId, Balance, BlockNumber, RuntimeOrigin};
use alloc::vec::Vec;
use codec::{Decode, Encode};
use core::fmt::Debug;
use polkadot_sdk::frame_support::traits::{Contains, Currency, Get};
use polkadot_sdk::frame_system;
use polkadot_sdk::sp_runtime::{
  self, DispatchError, DispatchResult, Perbill, Percent, traits::Zero,
};
use scale_info::TypeInfo;

/// Unified governance configuration
pub struct GovernanceConfig<T: Config> {
  /// Minimum voting period in blocks
  pub min_voting_period: BlockNumber,
  /// Maximum voting period in blocks
  pub max_voting_period: BlockNumber,
  /// Minimum deposit for proposals
  pub min_proposal_deposit: Balance,
  /// Required approval threshold
  pub approval_threshold: Perbill,
  /// Required turnout threshold
  pub turnout_threshold: Perbill,
  /// Emergency timeout in blocks
  pub emergency_timeout: BlockNumber,
  /// Phantom data for type safety
  _phantom: core::marker::PhantomData<T>,
}

impl<T: Config> Default for GovernanceConfig<T> {
  fn default() -> Self {
    Self {
      min_voting_period: 7 * 24 * 60 * 10,  // 7 days in 6s blocks
      max_voting_period: 14 * 24 * 60 * 10, // 14 days
      min_proposal_deposit: 1000 * crate::UNIT,
      approval_threshold: Perbill::from_percent(60),
      turnout_threshold: Perbill::from_percent(15),
      emergency_timeout: 24 * 60 * 10, // 24 hours
      _phantom: core::marker::PhantomData,
    }
  }
}

/// Governance proposal types
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub enum ProposalType {
  /// Parameter change proposal
  ParameterChange {
    pallet: Vec<u8>,
    parameter: Vec<u8>,
    new_value: Vec<u8>,
  },
  /// Treasury spending proposal
  TreasurySpend {
    beneficiary: AccountId,
    amount: Balance,
    purpose: Vec<u8>,
  },
  /// Runtime upgrade proposal
  RuntimeUpgrade {
    code_hash: [u8; 32],
    description: Vec<u8>,
  },
  /// Emergency procedure activation
  EmergencyProcedure {
    procedure: EmergencyProcedure,
    justification: Vec<u8>,
  },
}

/// Emergency procedures for system protection
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub enum EmergencyProcedure {
  /// Pause specific system functions
  SystemPause {
    pallets: Vec<Vec<u8>>,
    duration: BlockNumber,
  },
  /// Adjust economic parameters
  EconomicAdjustment {
    parameter: EconomicParameter,
    new_value: Vec<u8>,
  },
  /// Emergency treasury access
  EmergencyTreasuryAccess { amount: Balance, purpose: Vec<u8> },
}

/// Economic parameters that can be adjusted via governance
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub enum EconomicParameter {
  /// Transaction fees
  TransactionFee,
  /// Collator bonds
  CollatorBond,
  /// Inflation rate
  InflationRate,
  /// Treasury spending limits
  TreasuryLimit,
  /// Governance parameters
  GovernanceParameter,
}

/// Voting information for a proposal
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub struct Vote {
  /// Voter account
  pub voter: AccountId,
  /// Vote weight (based on stake)
  pub weight: Balance,
  /// Vote direction
  pub direction: VoteDirection,
  /// Block number when voted
  pub block_number: BlockNumber,
}

/// Vote directions
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub enum VoteDirection {
  /// Support the proposal
  Aye,
  /// Oppose the proposal
  Nay,
  /// Abstain from voting
  Abstain,
}

/// Proposal status
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub enum ProposalStatus {
  /// Active voting period
  Active {
    start_block: BlockNumber,
    end_block: BlockNumber,
  },
  /// Passed and awaiting execution
  Passed { passed_block: BlockNumber },
  /// Rejected by voters
  Rejected { rejected_block: BlockNumber },
  /// Executed successfully
  Executed { executed_block: BlockNumber },
  /// Cancelled by proposer
  Cancelled { cancelled_block: BlockNumber },
  /// Emergency override
  EmergencyOverride { overridden_block: BlockNumber },
}

/// Unified governance trait
pub trait GovernanceInterface<AccountId, Balance, BlockNumber> {
  /// Submit a new governance proposal
  fn submit_proposal(
    origin: RuntimeOrigin,
    proposal_type: ProposalType,
    deposit: Balance,
  ) -> DispatchResult;

  /// Vote on an active proposal
  fn vote(origin: RuntimeOrigin, proposal_id: u32, direction: VoteDirection) -> DispatchResult;

  /// Execute a passed proposal
  fn execute_proposal(origin: RuntimeOrigin, proposal_id: u32) -> DispatchResult;

  /// Cancel own proposal (with deposit refund)
  fn cancel_proposal(origin: RuntimeOrigin, proposal_id: u32) -> DispatchResult;

  /// Emergency override (sudo/admin only)
  fn emergency_override(
    origin: RuntimeOrigin,
    proposal_id: u32,
    justification: Vec<u8>,
  ) -> DispatchResult;

  /// Get proposal status
  fn get_proposal_status(proposal_id: u32) -> Option<ProposalStatus>;

  /// Get voting results
  fn get_voting_results(proposal_id: u32) -> Option<VotingResults>;
}

/// Voting results structure
#[derive(Clone, Debug, Default, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub struct VotingResults {
  /// Total votes in favor
  pub aye_votes: Balance,
  /// Total votes against
  pub nay_votes: Balance,
  /// Total abstentions
  pub abstain_votes: Balance,
  /// Total voting power
  pub total_voting_power: Balance,
  /// Required approval threshold
  pub approval_threshold: Perbill,
  /// Required turnout threshold
  pub turnout_threshold: Perbill,
}

impl VotingResults {
  /// Check if proposal passed
  pub fn is_passed(&self) -> bool {
    let total_votes = self.aye_votes + self.nay_votes + self.abstain_votes;

    // Check turnout threshold
    let turnout_ratio = if self.total_voting_power > Zero::zero() {
      Perbill::from_rational(total_votes, self.total_voting_power)
    } else {
      Perbill::zero()
    };

    if turnout_ratio < self.turnout_threshold {
      return false;
    }

    // Check approval threshold
    let effective_votes = self.aye_votes + self.nay_votes;
    if effective_votes > Zero::zero() {
      let approval_ratio = Perbill::from_rational(self.aye_votes, effective_votes);
      approval_ratio >= self.approval_threshold
    } else {
      false
    }
  }

  /// Calculate approval percentage
  pub fn approval_percentage(&self) -> Percent {
    let effective_votes = self.aye_votes + self.nay_votes;
    if effective_votes > Zero::zero() {
      Percent::from_rational(self.aye_votes, effective_votes)
    } else {
      Percent::zero()
    }
  }

  /// Calculate turnout percentage
  pub fn turnout_percentage(&self) -> Percent {
    if self.total_voting_power > Zero::zero() {
      let total_votes = self.aye_votes + self.nay_votes + self.abstain_votes;
      Percent::from_rational(total_votes, self.total_voting_power)
    } else {
      Percent::zero()
    }
  }
}

/// Economic parameter manager
pub trait EconomicParameterManager {
  /// Type for parameter values
  type ParameterValue: ParameterValue;

  /// Get current parameter value
  fn get_parameter(parameter: EconomicParameter) -> Option<Self::ParameterValue>;

  /// Set parameter value (governance only)
  fn set_parameter(parameter: EconomicParameter, value: Self::ParameterValue) -> DispatchResult;

  /// Validate parameter change
  fn validate_parameter_change(
    parameter: EconomicParameter,
    new_value: &Self::ParameterValue,
  ) -> DispatchResult;
}

/// Unified parameter value type
pub trait ParameterValue: Clone + Debug + PartialEq + Eq + TypeInfo + Encode + Decode {
  /// Validate the parameter value
  fn validate(&self) -> DispatchResult;

  /// Get default value
  fn default() -> Self;

  /// Convert to bytes for storage
  fn to_bytes(&self) -> Vec<u8>;

  /// Try to create from bytes
  fn try_from_bytes(bytes: &[u8]) -> Result<Self, DispatchError>;
}

// Implement ParameterValue for common types
impl ParameterValue for Balance {
  fn validate(&self) -> DispatchResult {
    if self.is_zero() {
      Err(DispatchError::Other("Parameter value cannot be zero"))
    } else {
      Ok(())
    }
  }

  fn default() -> Self {
    1000 * crate::UNIT
  }

  fn to_bytes(&self) -> Vec<u8> {
    self.encode()
  }

  fn try_from_bytes(bytes: &[u8]) -> Result<Self, DispatchError> {
    Decode::decode(&mut &bytes[..])
      .map_err(|_| DispatchError::Other("Failed to decode Balance parameter"))
  }
}

impl ParameterValue for Perbill {
  fn validate(&self) -> DispatchResult {
    if *self > Perbill::from_percent(100) {
      Err(DispatchError::Other("Parameter value cannot exceed 100%"))
    } else {
      Ok(())
    }
  }

  fn default() -> Self {
    Perbill::from_percent(50)
  }

  fn to_bytes(&self) -> Vec<u8> {
    self.encode()
  }

  fn try_from_bytes(bytes: &[u8]) -> Result<Self, DispatchError> {
    Decode::decode(&mut &bytes[..])
      .map_err(|_| DispatchError::Other("Failed to decode Perbill parameter"))
  }
}

/// Emergency manager for system protection
pub trait EmergencyManager {
  /// Check if emergency mode is active
  fn is_emergency_mode() -> bool;

  /// Activate emergency mode
  fn activate_emergency_mode(
    origin: RuntimeOrigin,
    procedure: EmergencyProcedure,
    justification: Vec<u8>,
  ) -> DispatchResult;

  /// Deactivate emergency mode
  fn deactivate_emergency_mode(origin: RuntimeOrigin) -> DispatchResult;

  /// Execute emergency procedure
  fn execute_emergency_procedure(
    origin: RuntimeOrigin,
    procedure: EmergencyProcedure,
  ) -> DispatchResult;
}

/// Governance events
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub enum GovernanceEvent<AccountId, Balance> {
  /// New proposal submitted
  ProposalSubmitted {
    proposal_id: u32,
    proposer: AccountId,
    proposal_type: ProposalType,
    deposit: Balance,
  },
  /// Vote cast on proposal
  VoteCast {
    proposal_id: u32,
    voter: AccountId,
    direction: VoteDirection,
    weight: Balance,
  },
  /// Proposal passed
  ProposalPassed {
    proposal_id: u32,
    passed_block: BlockNumber,
  },
  /// Proposal rejected
  ProposalRejected {
    proposal_id: u32,
    rejected_block: BlockNumber,
  },
  /// Proposal executed
  ProposalExecuted {
    proposal_id: u32,
    executed_block: BlockNumber,
  },
  /// Emergency mode activated
  EmergencyModeActivated {
    activated_by: AccountId,
    procedure: EmergencyProcedure,
    block_number: BlockNumber,
  },
  /// Emergency mode deactivated
  EmergencyModeDeactivated {
    deactivated_by: AccountId,
    block_number: BlockNumber,
  },
  /// Parameter changed via governance
  ParameterChanged {
    parameter: EconomicParameter,
    old_value: Vec<u8>,
    new_value: Vec<u8>,
    changed_by: AccountId,
  },
}

/// Governance errors
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub enum GovernanceError {
  /// Proposal not found
  ProposalNotFound,
  /// Proposal not active
  ProposalNotActive,
  /// Insufficient deposit
  InsufficientDeposit,
  /// Voting period ended
  VotingPeriodEnded,
  /// Already voted
  AlreadyVoted,
  /// Not proposer
  NotProposer,
  /// Proposal not passed
  ProposalNotPassed,
  /// Execution failed
  ExecutionFailed,
  /// Invalid parameter value
  InvalidParameterValue,
  /// Emergency mode active
  EmergencyModeActive,
  /// Not authorized for emergency
  NotEmergencyAuthorized,
  /// Invalid emergency justification
  InvalidEmergencyJustification,
}

impl From<GovernanceError> for DispatchError {
  fn from(error: GovernanceError) -> Self {
    DispatchError::Module(sp_runtime::ModuleError {
      index: 0, // Will be set by pallet
      error: error.encode().try_into().unwrap_or_default(),
      message: Some(match error {
        GovernanceError::ProposalNotFound => "ProposalNotFound",
        GovernanceError::ProposalNotActive => "ProposalNotActive",
        GovernanceError::InsufficientDeposit => "InsufficientDeposit",
        GovernanceError::VotingPeriodEnded => "VotingPeriodEnded",
        GovernanceError::AlreadyVoted => "AlreadyVoted",
        GovernanceError::NotProposer => "NotProposer",
        GovernanceError::ProposalNotPassed => "ProposalNotPassed",
        GovernanceError::ExecutionFailed => "ExecutionFailed",
        GovernanceError::InvalidParameterValue => "InvalidParameterValue",
        GovernanceError::EmergencyModeActive => "EmergencyModeActive",
        GovernanceError::NotEmergencyAuthorized => "NotEmergencyAuthorized",
        GovernanceError::InvalidEmergencyJustification => "InvalidEmergencyJustification",
      }),
    })
  }
}

/// Configuration trait for governance system
pub trait Config: frame_system::Config {
  /// The runtime event type
  type RuntimeEvent: From<GovernanceEvent<Self::AccountId, Balance>>;

  /// The currency type for deposits and voting
  type Currency: Currency<Self::AccountId>;

  /// Governance configuration
  type GovernanceConfig: Get<GovernanceConfig<Self>>;

  /// Maximum number of active proposals
  type MaxActiveProposals: Get<u32>;

  /// Maximum proposal description length
  type MaxDescriptionLength: Get<u32>;

  /// Emergency authorities
  type EmergencyAuthorities: Contains<Self::AccountId>;
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_voting_results_pass_conditions() {
    let results = VotingResults {
      aye_votes: 600,
      nay_votes: 400,
      abstain_votes: 100,
      total_voting_power: 5000,
      approval_threshold: Perbill::from_percent(60),
      turnout_threshold: Perbill::from_percent(10),
    };

    assert!(results.is_passed());
    assert_eq!(results.approval_percentage(), Percent::from_percent(60));
    assert_eq!(results.turnout_percentage(), Percent::from_percent(22));
  }

  #[test]
  fn test_voting_results_fail_turnout() {
    let results = VotingResults {
      aye_votes: 60,
      nay_votes: 40,
      abstain_votes: 0,
      total_voting_power: 5000,
      approval_threshold: Perbill::from_percent(60),
      turnout_threshold: Perbill::from_percent(10),
    };

    assert!(!results.is_passed()); // Turnout too low
  }

  #[test]
  fn test_voting_results_fail_approval() {
    let results = VotingResults {
      aye_votes: 550,
      nay_votes: 450,
      abstain_votes: 0,
      total_voting_power: 5000,
      approval_threshold: Perbill::from_percent(60),
      turnout_threshold: Perbill::from_percent(10),
    };

    assert!(!results.is_passed()); // Approval too low
  }
}
