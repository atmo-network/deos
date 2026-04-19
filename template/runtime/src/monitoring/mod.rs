//! Elegant Monitoring System
//!
//! Provides a unified, type-safe monitoring framework with comprehensive
//! economic metrics, system health tracking, and real-time analytics.

#![allow(dead_code)]

use crate::{Balance, BlockNumber};
use alloc::vec::Vec;
use codec::{Decode, Encode};
use core::{fmt::Debug, marker::PhantomData};
use polkadot_sdk::frame_support::traits::{Currency, Get};
use polkadot_sdk::frame_system;
use polkadot_sdk::sp_runtime::{
  self, DispatchError, DispatchResult, Perbill, Percent,
  traits::{SaturatedConversion, Zero},
};
use scale_info::TypeInfo;

/// Unified monitoring configuration
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub struct MonitoringConfig {
  /// Metrics collection interval in blocks
  pub collection_interval: BlockNumber,
  /// Alert thresholds for critical metrics
  pub alert_thresholds: AlertThresholds,
  /// Retention period for historical data
  pub data_retention_blocks: BlockNumber,
  /// Maximum metrics storage size
  pub max_storage_size: u32,
}

/// Alert thresholds for critical system monitoring
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub struct AlertThresholds {
  /// Minimum capital efficiency threshold
  pub min_capital_efficiency: Perbill,
  /// Maximum transaction failure rate
  pub max_failure_rate: Perbill,
  /// Minimum TOL utilization threshold
  pub min_tol_utilization: Perbill,
  /// Maximum price deviation threshold
  pub max_price_deviation: Perbill,
  /// Minimum system health score
  pub min_health_score: Perbill,
}

impl Default for MonitoringConfig {
  fn default() -> Self {
    Self {
      collection_interval: 10, // Every 10 blocks
      alert_thresholds: AlertThresholds::default(),
      data_retention_blocks: 100_800, // ~1 week in 6s blocks
      max_storage_size: 10_000,
    }
  }
}

impl Default for AlertThresholds {
  fn default() -> Self {
    Self {
      min_capital_efficiency: Perbill::from_percent(80),
      max_failure_rate: Perbill::from_percent(5),
      min_tol_utilization: Perbill::from_percent(70),
      max_price_deviation: Perbill::from_percent(20),
      min_health_score: Perbill::from_percent(90),
    }
  }
}

/// Comprehensive economic metrics
#[derive(Clone, Debug, Default, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub struct EconomicMetrics {
  /// Block number when metrics were collected
  pub block_number: BlockNumber,
  /// Total token supply
  pub total_supply: Balance,
  /// Treasury-owned liquidity
  pub treasury_liquidity: Balance,
  /// TOL-to-supply ratio
  pub tol_supply_ratio: Perbill,
  /// Capital efficiency (TOL utilization)
  pub capital_efficiency: Perbill,
  /// Burn velocity (tokens burned per block)
  pub burn_velocity: Balance,
  /// Total burned tokens
  pub total_burned: Balance,
  /// Transaction volume (last interval)
  pub transaction_volume: Balance,
  /// Fee collection efficiency
  pub fee_collection_efficiency: Perbill,
  /// Price stability metrics
  pub price_stability: PriceStabilityMetrics,
}

/// Price stability metrics
#[derive(Clone, Debug, Default, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub struct PriceStabilityMetrics {
  /// Current price deviation from EMA
  pub current_deviation: Perbill,
  /// Average deviation over period
  pub average_deviation: Perbill,
  /// Maximum observed deviation
  pub max_deviation: Perbill,
  /// Price volatility score
  pub volatility_score: Percent,
}

/// System health metrics
#[derive(Clone, Debug, Default, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub struct SystemHealthMetrics {
  /// Transaction success rate
  pub transaction_success_rate: Perbill,
  /// Block production consistency
  pub block_production_consistency: Perbill,
  /// Network participation rate
  pub network_participation: Perbill,
  /// Storage utilization
  pub storage_utilization: Perbill,
  /// Memory usage efficiency
  pub memory_efficiency: Perbill,
  /// Overall health score (0-100%)
  pub overall_health_score: Perbill,
}

/// Economic coordination metrics
#[derive(Clone, Debug, Default, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub struct EconomicCoordinationMetrics {
  /// Fee buffer accumulation rate
  pub fee_buffer_growth: Balance,
  /// Burn execution efficiency
  pub burn_execution_efficiency: Perbill,
  /// TMC integration effectiveness
  pub tmc_integration_score: Percent,
  /// Router performance metrics
  pub router_performance: RouterMetrics,
  /// Cross-pallet coordination score
  pub cross_pallet_coordination: Percent,
}

/// Router performance metrics
#[derive(Clone, Debug, Default, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub struct RouterMetrics {
  /// Route discovery success rate
  pub route_success_rate: Perbill,
  /// Average route efficiency
  pub average_route_efficiency: Percent,
  /// Price impact minimization
  pub price_impact_reduction: Percent,
  /// Gas efficiency improvement
  pub gas_efficiency: Percent,
}

/// Real-time monitoring dashboard
#[derive(Clone, Debug, Default, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub struct MonitoringDashboard {
  /// Current economic metrics
  pub economic_metrics: EconomicMetrics,
  /// System health status
  pub system_health: SystemHealthMetrics,
  /// Economic coordination status
  pub economic_coordination: EconomicCoordinationMetrics,
  /// Active alerts and warnings
  pub active_alerts: Vec<SystemAlert>,
  /// Performance trends
  pub performance_trends: PerformanceTrends,
}

/// Performance trends over time
#[derive(Clone, Debug, Default, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub struct PerformanceTrends {
  /// Capital efficiency trend
  pub capital_efficiency_trend: TrendDirection,
  /// Burn velocity trend
  pub burn_velocity_trend: TrendDirection,
  /// Transaction volume trend
  pub transaction_volume_trend: TrendDirection,
  /// System health trend
  pub system_health_trend: TrendDirection,
}

/// Trend direction indicators
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub enum TrendDirection {
  /// Strong positive trend
  StrongPositive,
  /// Moderate positive trend
  ModeratePositive,
  /// Stable/no trend
  Stable,
  /// Moderate negative trend
  ModerateNegative,
  /// Strong negative trend
  StrongNegative,
}

impl Default for TrendDirection {
  fn default() -> Self {
    Self::Stable
  }
}

/// System alert types
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub enum SystemAlert {
  /// Critical economic alert
  EconomicCritical {
    metric: EconomicMetricType,
    current_value: Vec<u8>,
    threshold: Vec<u8>,
    severity: AlertSeverity,
  },
  /// System health alert
  HealthCritical {
    metric: HealthMetricType,
    current_value: Percent,
    threshold: Percent,
    severity: AlertSeverity,
  },
  /// Performance degradation
  PerformanceDegradation {
    component: SystemComponent,
    degradation: Percent,
    severity: AlertSeverity,
  },
  /// Security incident
  SecurityIncident {
    incident_type: SecurityIncidentType,
    description: Vec<u8>,
    severity: AlertSeverity,
  },
}

/// Economic metric types for alerts
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub enum EconomicMetricType {
  CapitalEfficiency,
  TOLUtilization,
  BurnVelocity,
  PriceStability,
  FeeCollection,
}

/// Health metric types for alerts
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub enum HealthMetricType {
  TransactionSuccess,
  BlockProduction,
  NetworkParticipation,
  StorageUtilization,
  MemoryEfficiency,
}

/// System components
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub enum SystemComponent {
  Router,
  Tmc,
  Tol,
  FeeManager,
  Governance,
  Xcm,
}

/// Security incident types
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub enum SecurityIncidentType {
  PriceManipulation,
  MEVExtraction,
  GovernanceAttack,
  EconomicExploit,
  SystemFailure,
}

/// Alert severity levels
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub enum AlertSeverity {
  /// Informational only
  Info,
  /// Warning - monitor closely
  Warning,
  /// Critical - immediate action required
  Critical,
  /// Emergency - system at risk
  Emergency,
}

/// Unified monitoring interface
pub trait MonitoringInterface<AccountId, Balance, BlockNumber> {
  /// Collect comprehensive metrics for current block
  fn collect_metrics() -> DispatchResult;

  /// Get current monitoring dashboard
  fn get_dashboard() -> MonitoringDashboard;

  /// Check for active alerts
  fn check_alerts() -> Vec<SystemAlert>;

  /// Get historical metrics for time period
  fn get_historical_metrics(
    start_block: BlockNumber,
    end_block: BlockNumber,
  ) -> Option<Vec<EconomicMetrics>>;

  /// Calculate performance trends
  fn calculate_trends() -> PerformanceTrends;

  /// Emit monitoring event
  fn emit_monitoring_event(event: MonitoringEvent<AccountId, Balance>);

  /// Validate system health
  fn validate_system_health() -> SystemHealthValidation;
}

/// System health validation result
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub struct SystemHealthValidation {
  /// Overall health status
  pub health_status: HealthStatus,
  /// Detailed validation results
  pub validation_results: Vec<ValidationResult>,
  /// Recommended actions
  pub recommended_actions: Vec<RecommendedAction>,
}

/// Health status levels
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub enum HealthStatus {
  /// Optimal performance
  Optimal,
  /// Healthy with minor issues
  Healthy,
  /// Degraded performance
  Degraded,
  /// Critical issues present
  Critical,
  /// System failure
  Failed,
}

/// Individual validation result
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub struct ValidationResult {
  /// Component being validated
  pub component: SystemComponent,
  /// Validation status
  pub status: ValidationStatus,
  /// Detailed message
  pub message: Vec<u8>,
  /// Metric value if applicable
  pub metric_value: Option<Vec<u8>>,
}

/// Validation status
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub enum ValidationStatus {
  /// Validation passed
  Passed,
  /// Validation warning
  Warning,
  /// Validation failed
  Failed,
  /// Validation skipped
  Skipped,
}

/// Recommended actions for system improvement
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub struct RecommendedAction {
  /// Action description
  pub description: Vec<u8>,
  /// Priority level
  pub priority: ActionPriority,
  /// Estimated impact
  pub estimated_impact: ImpactEstimate,
  /// Required resources
  pub required_resources: Vec<ResourceRequirement>,
}

/// Action priority levels
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub enum ActionPriority {
  /// Immediate action required
  Immediate,
  /// High priority
  High,
  /// Medium priority
  Medium,
  /// Low priority
  Low,
}

/// Impact estimates for actions
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub struct ImpactEstimate {
  /// Expected performance improvement
  pub performance_improvement: Percent,
  /// Expected economic improvement
  pub economic_improvement: Percent,
  /// Implementation complexity
  pub implementation_complexity: ImplementationComplexity,
}

/// Implementation complexity levels
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub enum ImplementationComplexity {
  /// Simple implementation
  Simple,
  /// Moderate complexity
  Moderate,
  /// Complex implementation
  Complex,
  /// Very complex implementation
  VeryComplex,
}

/// Resource requirements for actions
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub struct ResourceRequirement {
  /// Resource type
  pub resource_type: ResourceType,
  /// Required amount
  pub amount: Balance,
  /// Duration if applicable
  pub duration: Option<BlockNumber>,
}

/// Resource types
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub enum ResourceType {
  /// Computational resources
  Computation,
  /// Storage resources
  Storage,
  /// Network resources
  Network,
  /// Economic resources (tokens)
  Economic,
  /// Human resources
  Human,
}

/// Monitoring events
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub enum MonitoringEvent<AccountId, Balance> {
  /// Metrics collected
  MetricsCollected {
    block_number: BlockNumber,
    metrics: EconomicMetrics,
  },
  /// Alert triggered
  AlertTriggered {
    alert: SystemAlert,
    triggered_block: BlockNumber,
  },
  /// System health changed
  HealthStatusChanged {
    old_status: HealthStatus,
    new_status: HealthStatus,
    changed_block: BlockNumber,
  },
  /// Performance trend detected
  TrendDetected {
    trend: PerformanceTrends,
    detected_block: BlockNumber,
  },
  #[doc(hidden)]
  _Phantom(PhantomData<(AccountId, Balance)>),
}

/// Monitoring errors
#[derive(Clone, Debug, PartialEq, Eq, TypeInfo, Encode, Decode)]
pub enum MonitoringError {
  /// Metrics collection failed
  CollectionFailed,
  /// Storage limit exceeded
  StorageLimitExceeded,
  /// Invalid metric data
  InvalidMetricData,
  /// Historical data unavailable
  HistoricalDataUnavailable,
  /// System validation failed
  ValidationFailed,
}

impl From<MonitoringError> for DispatchError {
  fn from(error: MonitoringError) -> Self {
    DispatchError::Module(sp_runtime::ModuleError {
      index: 0, // Will be set by pallet
      error: error.encode().try_into().unwrap_or_default(),
      message: Some(match error {
        MonitoringError::CollectionFailed => "CollectionFailed",
        MonitoringError::StorageLimitExceeded => "StorageLimitExceeded",
        MonitoringError::InvalidMetricData => "InvalidMetricData",
        MonitoringError::HistoricalDataUnavailable => "HistoricalDataUnavailable",
        MonitoringError::ValidationFailed => "ValidationFailed",
      }),
    })
  }
}

/// Configuration trait for monitoring system
pub trait Config: frame_system::Config {
  /// The runtime event type
  type RuntimeEvent: From<MonitoringEvent<Self::AccountId, Balance>>
    + Into<<Self as frame_system::Config>::RuntimeEvent>;

  /// Monitoring configuration
  type MonitoringConfig: Get<MonitoringConfig>;

  /// Currency type for economic metrics
  type Currency: Currency<Self::AccountId>;

  /// Maximum historical data points
  type MaxHistoricalPoints: Get<u32>;

  /// Alert notification handlers
  type AlertHandlers: AlertHandler<Self::AccountId>;
}

/// Alert handler trait for notifications
pub trait AlertHandler<AccountId> {
  /// Handle system alert
  fn handle_alert(alert: SystemAlert, block_number: BlockNumber) -> DispatchResult;

  /// Notify emergency contacts
  fn notify_emergency_contacts(alert: SystemAlert, contacts: Vec<AccountId>) -> DispatchResult;
}

// Utility functions for metric calculations
impl EconomicMetrics {
  /// Calculate TOL-to-supply ratio
  pub fn calculate_tol_supply_ratio(&mut self) {
    if self.total_supply > Zero::zero() {
      self.tol_supply_ratio = Perbill::from_rational(self.treasury_liquidity, self.total_supply);
    } else {
      self.tol_supply_ratio = Perbill::zero();
    }
  }

  /// Update capital efficiency
  pub fn update_capital_efficiency(&mut self, utilized_liquidity: Balance) {
    if self.treasury_liquidity > Zero::zero() {
      self.capital_efficiency = Perbill::from_rational(utilized_liquidity, self.treasury_liquidity);
    } else {
      self.capital_efficiency = Perbill::zero();
    }
  }

  /// Calculate burn velocity
  pub fn calculate_burn_velocity(
    &mut self,
    previous_total_burned: Balance,
    blocks_passed: BlockNumber,
  ) {
    if blocks_passed > Zero::zero() && self.total_burned > previous_total_burned {
      let burned_since_last = self.total_burned - previous_total_burned;
      let blocks: Balance = blocks_passed.saturated_into();
      if blocks > Zero::zero() {
        self.burn_velocity = burned_since_last / blocks;
        return;
      }
    }

    self.burn_velocity = Balance::zero();
  }
}

impl SystemHealthMetrics {
  /// Calculate overall health score
  pub fn calculate_overall_score(&mut self) {
    let components = [
      self.transaction_success_rate,
      self.block_production_consistency,
      self.network_participation,
      self.storage_utilization,
      self.memory_efficiency,
    ];

    let total: u64 = components.iter().map(|c| c.deconstruct() as u64).sum();
    let average = (total / components.len() as u64) as u32;

    self.overall_health_score = Perbill::from_parts(average);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_economic_metrics_calculations() {
    let mut metrics = EconomicMetrics {
      block_number: 100,
      total_supply: 1_000_000 * crate::UNIT,
      treasury_liquidity: 500_000 * crate::UNIT,
      ..Default::default()
    };

    metrics.calculate_tol_supply_ratio();
    assert_eq!(metrics.tol_supply_ratio, Perbill::from_percent(50));

    metrics.update_capital_efficiency(400_000 * crate::UNIT);
    assert_eq!(metrics.capital_efficiency, Perbill::from_percent(80));
  }

  #[test]
  fn test_system_health_scoring() {
    let mut health = SystemHealthMetrics {
      transaction_success_rate: Perbill::from_percent(95),
      block_production_consistency: Perbill::from_percent(98),
      network_participation: Perbill::from_percent(85),
      storage_utilization: Perbill::from_percent(75),
      memory_efficiency: Perbill::from_percent(90),
      ..Default::default()
    };

    health.calculate_overall_score();
    assert!(health.overall_health_score >= Perbill::from_percent(88));
  }

  #[test]
  fn test_alert_threshold_validation() {
    let thresholds = AlertThresholds::default();
    let metrics = EconomicMetrics {
      capital_efficiency: Perbill::from_percent(75), // Below threshold
      ..Default::default()
    };

    // This would trigger an alert in real implementation
    assert!(metrics.capital_efficiency < thresholds.min_capital_efficiency);
  }
}
