use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QualityRuleKind {
    NotNull,
    NonEmpty,
    Unique,
    NumericRange,
    AllowedValues,
    Regex,
    Dtype,
    UniqueTogether,
    ColumnCompare,
    ReferentialIntegrity,
    Monotonic,
    AggregateCheck,
    AggregateReconciliation,
    DistributionDrift,
    DateRange,
    Conditional,
    SchemaContract,
    RowCount,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QualityComparison {
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QualityMonotonicDirection {
    Increasing,
    Decreasing,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QualityAggregate {
    Count,
    Sum,
    Min,
    Max,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QualityCondition {
    pub(super) column: String,
    pub(super) operator: Option<QualityComparison>,
    pub(super) value: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QualityRule {
    pub(super) column: String,
    pub(super) kind: QualityRuleKind,
    pub(super) max_invalid: Option<usize>,
    pub(super) max_invalid_pct: Option<f64>,
    pub(super) min: Option<f64>,
    pub(super) max: Option<f64>,
    pub(super) values: Option<Vec<String>>,
    pub(super) reference_values: Option<Vec<String>>,
    pub(super) baseline: Option<Vec<String>>,
    pub(super) direction: Option<QualityMonotonicDirection>,
    pub(super) expected: Option<f64>,
    pub(super) aggregate: Option<QualityAggregate>,
    pub(super) tolerance_abs: Option<f64>,
    pub(super) tolerance_rel: Option<f64>,
    pub(super) threshold: Option<f64>,
    pub(super) pattern: Option<String>,
    pub(super) dtype: Option<String>,
    pub(super) columns: Option<Vec<String>>,
    pub(super) operator: Option<QualityComparison>,
    pub(super) min_date: Option<String>,
    pub(super) max_date: Option<String>,
    pub(super) when: Option<QualityCondition>,
    pub(super) then: Option<Box<QualityRule>>,
    pub(super) allow_additional: Option<bool>,
    pub(super) required_order: Option<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QualityRuleResult {
    pub(super) column: String,
    pub(super) kind: QualityRuleKind,
    pub(super) max_invalid: Option<usize>,
    pub(super) max_invalid_pct: Option<f64>,
    pub(super) min: Option<f64>,
    pub(super) max: Option<f64>,
    pub(super) values: Option<Vec<String>>,
    pub(super) reference_values: Option<Vec<String>>,
    pub(super) baseline: Option<Vec<String>>,
    pub(super) direction: Option<QualityMonotonicDirection>,
    pub(super) expected: Option<f64>,
    pub(super) aggregate: Option<QualityAggregate>,
    pub(super) tolerance_abs: Option<f64>,
    pub(super) tolerance_rel: Option<f64>,
    pub(super) threshold: Option<f64>,
    pub(super) pattern: Option<String>,
    pub(super) dtype: Option<String>,
    pub(super) columns: Option<Vec<String>>,
    pub(super) operator: Option<QualityComparison>,
    pub(super) min_date: Option<String>,
    pub(super) max_date: Option<String>,
    pub(super) when: Option<QualityCondition>,
    pub(super) then: Option<Box<QualityRule>>,
    pub(super) allow_additional: Option<bool>,
    pub(super) required_order: Option<Vec<String>>,
    pub(super) checked_count: usize,
    pub(super) invalid_count: usize,
    pub(super) invalid_pct: f64,
    pub(super) passed: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QualityValidationResult {
    pub(crate) passed: bool,
    pub(crate) row_count: usize,
    pub(crate) total_rules: usize,
    pub(crate) failed_rules: usize,
    pub(super) rules: Vec<QualityRuleResult>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QualityMigrationWarning {
    pub(super) rule_index: usize,
    pub(super) source_kind: String,
    pub(super) severity: &'static str,
    pub(super) message: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QualityMigrationReport {
    pub(super) artifact_sha256: Option<String>,
    pub(super) total_items: usize,
    pub(super) converted_items: usize,
    pub(super) omitted_items: usize,
    pub(super) warning_count: usize,
    pub(super) manual_actions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QualityMigrationResult {
    pub(super) source_format: &'static str,
    pub(super) source_version: Option<String>,
    pub(super) converted_rules: Vec<QualityRule>,
    pub(super) warnings: Vec<QualityMigrationWarning>,
    pub(super) omitted_rules: usize,
    pub(super) report: QualityMigrationReport,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QualityRulesDocument {
    pub(super) format: String,
    pub(super) version: u8,
    pub(super) rules: Vec<QualityRule>,
}

impl QualityValidationResult {
    pub(crate) fn total_invalid_count(&self) -> usize {
        self.rules.iter().map(|rule| rule.invalid_count).sum()
    }
}
