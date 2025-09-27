// Feature Flag Conditions
// Condition types for targeting rules

use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use regex::Regex;

// Condition types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Condition {
    // User conditions
    UserProperty {
        property: String,
        operator: ComparisonOperator,
        value: serde_json::Value,
    },
    UserSegment {
        segments: Vec<String>,
        match_any: bool,
    },
    UserPercentage {
        percentage: f64,
        salt: Option<String>,
    },

    // System conditions
    Platform {
        platforms: Vec<String>,
    },
    Version {
        operator: VersionOperator,
        version: String,
    },
    Environment {
        environments: Vec<String>,
    },
    Date {
        operator: DateOperator,
        date: chrono::DateTime<chrono::Utc>,
    },

    // Application conditions
    Feature {
        feature: String,
        enabled: bool,
    },
    Performance {
        metric: String,
        operator: ComparisonOperator,
        threshold: f64,
    },

    // Logical conditions
    And {
        conditions: Vec<Box<Condition>>,
    },
    Or {
        conditions: Vec<Box<Condition>>,
    },
    Not {
        condition: Box<Condition>,
    },
}

// Comparison operators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComparisonOperator {
    Equals,
    NotEquals,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    Contains,
    NotContains,
    StartsWith,
    EndsWith,
    Matches, // Regex match
    In,
    NotIn,
}

// Version operators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VersionOperator {
    Equals,
    NotEquals,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
}

// Date operators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DateOperator {
    Before,
    After,
    Between,
}

// Condition type for external reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConditionType {
    UserProperty,
    UserSegment,
    UserPercentage,
    Platform,
    Version,
    Environment,
    Date,
    Feature,
    Performance,
    And,
    Or,
    Not,
}

impl Condition {
    // Evaluate condition against context
    pub fn evaluate(&self, context: &HashMap<String, serde_json::Value>) -> bool {
        match self {
            Self::UserProperty { property, operator, value } => {
                if let Some(context_value) = context.get(property) {
                    Self::compare_values(context_value, value, operator)
                } else {
                    false
                }
            }

            Self::UserSegment { segments, match_any } => {
                if let Some(user_segments) = context.get("user_segments")
                    .and_then(|v| v.as_array()) {

                    let user_segment_strings: Vec<String> = user_segments
                        .iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect();

                    if *match_any {
                        segments.iter().any(|s| user_segment_strings.contains(s))
                    } else {
                        segments.iter().all(|s| user_segment_strings.contains(s))
                    }
                } else {
                    false
                }
            }

            Self::UserPercentage { percentage, salt } => {
                if let Some(user_id) = context.get("user_id")
                    .and_then(|v| v.as_str()) {

                    let hash_input = if let Some(s) = salt {
                        format!("{}{}", user_id, s)
                    } else {
                        user_id.to_string()
                    };

                    let hash = Self::hash_string(&hash_input);
                    let bucket = (hash % 100) as f64;
                    bucket < *percentage
                } else {
                    false
                }
            }

            Self::Platform { platforms } => {
                if let Some(platform) = context.get("platform")
                    .and_then(|v| v.as_str()) {
                    platforms.iter().any(|p| p == platform)
                } else {
                    false
                }
            }

            Self::Version { operator, version } => {
                if let Some(current_version) = context.get("version")
                    .and_then(|v| v.as_str()) {
                    Self::compare_versions(current_version, version, operator)
                } else {
                    false
                }
            }

            Self::Environment { environments } => {
                if let Some(env) = context.get("environment")
                    .and_then(|v| v.as_str()) {
                    environments.iter().any(|e| e == env)
                } else {
                    false
                }
            }

            Self::Date { operator, date } => {
                let now = chrono::Utc::now();
                match operator {
                    DateOperator::Before => now < *date,
                    DateOperator::After => now > *date,
                    DateOperator::Between => {
                        // For between, we'd need two dates - simplified here
                        false
                    }
                }
            }

            Self::Feature { feature, enabled } => {
                if let Some(feature_enabled) = context.get(&format!("feature_{}", feature))
                    .and_then(|v| v.as_bool()) {
                    feature_enabled == *enabled
                } else {
                    false
                }
            }

            Self::Performance { metric, operator, threshold } => {
                if let Some(metric_value) = context.get(&format!("metric_{}", metric))
                    .and_then(|v| v.as_f64()) {
                    Self::compare_numbers(metric_value, *threshold, operator)
                } else {
                    false
                }
            }

            Self::And { conditions } => {
                conditions.iter().all(|c| c.evaluate(context))
            }

            Self::Or { conditions } => {
                conditions.iter().any(|c| c.evaluate(context))
            }

            Self::Not { condition } => {
                !condition.evaluate(context)
            }
        }
    }

    // Compare two values with operator
    fn compare_values(
        actual: &serde_json::Value,
        expected: &serde_json::Value,
        operator: &ComparisonOperator,
    ) -> bool {
        match operator {
            ComparisonOperator::Equals => actual == expected,
            ComparisonOperator::NotEquals => actual != expected,

            ComparisonOperator::GreaterThan => {
                if let (Some(a), Some(e)) = (actual.as_f64(), expected.as_f64()) {
                    a > e
                } else {
                    false
                }
            }

            ComparisonOperator::GreaterThanOrEqual => {
                if let (Some(a), Some(e)) = (actual.as_f64(), expected.as_f64()) {
                    a >= e
                } else {
                    false
                }
            }

            ComparisonOperator::LessThan => {
                if let (Some(a), Some(e)) = (actual.as_f64(), expected.as_f64()) {
                    a < e
                } else {
                    false
                }
            }

            ComparisonOperator::LessThanOrEqual => {
                if let (Some(a), Some(e)) = (actual.as_f64(), expected.as_f64()) {
                    a <= e
                } else {
                    false
                }
            }

            ComparisonOperator::Contains => {
                if let (Some(a), Some(e)) = (actual.as_str(), expected.as_str()) {
                    a.contains(e)
                } else {
                    false
                }
            }

            ComparisonOperator::NotContains => {
                if let (Some(a), Some(e)) = (actual.as_str(), expected.as_str()) {
                    !a.contains(e)
                } else {
                    false
                }
            }

            ComparisonOperator::StartsWith => {
                if let (Some(a), Some(e)) = (actual.as_str(), expected.as_str()) {
                    a.starts_with(e)
                } else {
                    false
                }
            }

            ComparisonOperator::EndsWith => {
                if let (Some(a), Some(e)) = (actual.as_str(), expected.as_str()) {
                    a.ends_with(e)
                } else {
                    false
                }
            }

            ComparisonOperator::Matches => {
                if let (Some(a), Some(pattern)) = (actual.as_str(), expected.as_str()) {
                    Regex::new(pattern).map(|re| re.is_match(a)).unwrap_or(false)
                } else {
                    false
                }
            }

            ComparisonOperator::In => {
                if let Some(arr) = expected.as_array() {
                    arr.contains(actual)
                } else {
                    false
                }
            }

            ComparisonOperator::NotIn => {
                if let Some(arr) = expected.as_array() {
                    !arr.contains(actual)
                } else {
                    false
                }
            }
        }
    }

    // Compare version strings
    fn compare_versions(actual: &str, expected: &str, operator: &VersionOperator) -> bool {
        let actual_parts: Vec<u32> = actual
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect();

        let expected_parts: Vec<u32> = expected
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect();

        // Compare version parts
        for i in 0..actual_parts.len().max(expected_parts.len()) {
            let a = actual_parts.get(i).unwrap_or(&0);
            let e = expected_parts.get(i).unwrap_or(&0);

            match operator {
                VersionOperator::Equals => {
                    if a != e {
                        return false;
                    }
                }
                VersionOperator::NotEquals => {
                    if a != e {
                        return true;
                    }
                }
                VersionOperator::GreaterThan => {
                    if a > e {
                        return true;
                    } else if a < e {
                        return false;
                    }
                }
                VersionOperator::GreaterThanOrEqual => {
                    if a > e {
                        return true;
                    } else if a < e {
                        return false;
                    }
                }
                VersionOperator::LessThan => {
                    if a < e {
                        return true;
                    } else if a > e {
                        return false;
                    }
                }
                VersionOperator::LessThanOrEqual => {
                    if a < e {
                        return true;
                    } else if a > e {
                        return false;
                    }
                }
            }
        }

        // If we've compared all parts and they're equal
        matches!(operator,
            VersionOperator::Equals |
            VersionOperator::GreaterThanOrEqual |
            VersionOperator::LessThanOrEqual
        )
    }

    // Compare numbers
    fn compare_numbers(actual: f64, expected: f64, operator: &ComparisonOperator) -> bool {
        match operator {
            ComparisonOperator::Equals => (actual - expected).abs() < f64::EPSILON,
            ComparisonOperator::NotEquals => (actual - expected).abs() >= f64::EPSILON,
            ComparisonOperator::GreaterThan => actual > expected,
            ComparisonOperator::GreaterThanOrEqual => actual >= expected,
            ComparisonOperator::LessThan => actual < expected,
            ComparisonOperator::LessThanOrEqual => actual <= expected,
            _ => false,
        }
    }

    // Simple hash function for percentage bucketing
    fn hash_string(s: &str) -> u32 {
        let mut hash: u32 = 0;
        for byte in s.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(byte as u32);
        }
        hash
    }
}