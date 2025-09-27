// Feature Flag Evaluator
// Evaluation logic for feature flags

use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use tracing::{debug, warn};

use super::flags::{FeatureFlag, FlagValue};
use crate::error::AppError;

// Evaluation context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationContext {
    // User information
    pub user_id: Option<String>,
    pub user_email: Option<String>,
    pub user_segments: Vec<String>,
    pub user_properties: HashMap<String, serde_json::Value>,

    // System information
    pub platform: String,
    pub version: String,
    pub environment: String,

    // Application state
    pub feature_states: HashMap<String, bool>,
    pub metrics: HashMap<String, f64>,

    // Request information
    pub ip_address: Option<String>,
    pub country: Option<String>,
    pub request_id: Option<String>,
}

impl Default for EvaluationContext {
    fn default() -> Self {
        Self {
            user_id: None,
            user_email: None,
            user_segments: Vec::new(),
            user_properties: HashMap::new(),
            platform: std::env::consts::OS.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            environment: "production".to_string(),
            feature_states: HashMap::new(),
            metrics: HashMap::new(),
            ip_address: None,
            country: None,
            request_id: None,
        }
    }
}

impl EvaluationContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_user(mut self, user_id: String) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn with_segments(mut self, segments: Vec<String>) -> Self {
        self.user_segments = segments;
        self
    }

    pub fn with_property(mut self, key: String, value: serde_json::Value) -> Self {
        self.user_properties.insert(key, value);
        self
    }

    pub fn with_environment(mut self, environment: String) -> Self {
        self.environment = environment;
        self
    }

    // Convert to HashMap for condition evaluation
    pub fn to_map(&self) -> HashMap<String, serde_json::Value> {
        let mut map = HashMap::new();

        // User info
        if let Some(ref id) = self.user_id {
            map.insert("user_id".to_string(), serde_json::json!(id));
        }
        if let Some(ref email) = self.user_email {
            map.insert("user_email".to_string(), serde_json::json!(email));
        }
        map.insert("user_segments".to_string(), serde_json::json!(self.user_segments));

        // System info
        map.insert("platform".to_string(), serde_json::json!(self.platform));
        map.insert("version".to_string(), serde_json::json!(self.version));
        map.insert("environment".to_string(), serde_json::json!(self.environment));

        // User properties
        for (key, value) in &self.user_properties {
            map.insert(format!("user_{}", key), value.clone());
        }

        // Feature states
        for (key, value) in &self.feature_states {
            map.insert(format!("feature_{}", key), serde_json::json!(value));
        }

        // Metrics
        for (key, value) in &self.metrics {
            map.insert(format!("metric_{}", key), serde_json::json!(value));
        }

        map
    }
}

// Evaluation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationResult {
    pub flag_key: String,
    pub value: FlagValue,
    pub reason: EvaluationReason,
    pub variant: Option<String>,
    pub timestamp: DateTime<Utc>,
}

// Reason for evaluation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvaluationReason {
    Default,
    Off,
    TargetingMatch { rule: String },
    PercentageRollout,
    Dependency,
    Schedule,
    Error { message: String },
}

// Flag evaluator
pub struct FlagEvaluator {
    flags: HashMap<String, FeatureFlag>,
    cache: HashMap<String, EvaluationResult>,
}

impl Default for FlagEvaluator {
    fn default() -> Self {
        Self {
            flags: HashMap::new(),
            cache: HashMap::new(),
        }
    }
}

impl FlagEvaluator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_flags(flags: HashMap<String, FeatureFlag>) -> Self {
        Self {
            flags,
            cache: HashMap::new(),
        }
    }

    // Add a flag
    pub fn add_flag(&mut self, flag: FeatureFlag) {
        self.flags.insert(flag.key.clone(), flag);
        self.cache.clear(); // Clear cache when flags change
    }

    // Remove a flag
    pub fn remove_flag(&mut self, key: &str) -> Option<FeatureFlag> {
        self.cache.clear();
        self.flags.remove(key)
    }

    // Evaluate a flag
    pub fn evaluate(
        &mut self,
        flag_key: &str,
        context: &EvaluationContext,
    ) -> Result<EvaluationResult, AppError> {
        // Check cache
        let cache_key = format!("{}:{}", flag_key, context.user_id.as_ref().unwrap_or(&"".to_string()));
        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(cached.clone());
        }

        // Get flag
        let flag = self.flags.get(flag_key)
            .ok_or_else(|| AppError::NotFound {
                message: format!("Feature flag '{}' not found", flag_key),
            })?;

        // Evaluate
        let result = self.evaluate_flag(flag, context)?;

        // Cache result
        self.cache.insert(cache_key, result.clone());

        debug!(
            flag = flag_key,
            value = ?result.value,
            reason = ?result.reason,
            "Evaluated feature flag"
        );

        Ok(result)
    }

    // Evaluate a specific flag
    fn evaluate_flag(
        &self,
        flag: &FeatureFlag,
        context: &EvaluationContext,
    ) -> Result<EvaluationResult, AppError> {
        // Check if flag is active
        if !flag.is_active() {
            return Ok(EvaluationResult {
                flag_key: flag.key.clone(),
                value: FlagValue::Boolean(false),
                reason: EvaluationReason::Off,
                variant: None,
                timestamp: Utc::now(),
            });
        }

        // Check schedule
        let now = Utc::now();
        if let Some(from) = flag.enabled_from {
            if now < from {
                return Ok(EvaluationResult {
                    flag_key: flag.key.clone(),
                    value: flag.default_value.clone(),
                    reason: EvaluationReason::Schedule,
                    variant: None,
                    timestamp: now,
                });
            }
        }
        if let Some(until) = flag.enabled_until {
            if now > until {
                return Ok(EvaluationResult {
                    flag_key: flag.key.clone(),
                    value: FlagValue::Boolean(false),
                    reason: EvaluationReason::Schedule,
                    variant: None,
                    timestamp: now,
                });
            }
        }

        // Check dependencies
        for dep in &flag.depends_on {
            if let Some(dep_flag) = self.flags.get(dep) {
                let dep_result = self.evaluate_flag(dep_flag, context)?;
                if !dep_result.value.as_bool().unwrap_or(false) {
                    return Ok(EvaluationResult {
                        flag_key: flag.key.clone(),
                        value: FlagValue::Boolean(false),
                        reason: EvaluationReason::Dependency,
                        variant: None,
                        timestamp: now,
                    });
                }
            }
        }

        // Check conflicts
        for conflict in &flag.conflicts_with {
            if let Some(conflict_flag) = self.flags.get(conflict) {
                let conflict_result = self.evaluate_flag(conflict_flag, context)?;
                if conflict_result.value.as_bool().unwrap_or(false) {
                    return Ok(EvaluationResult {
                        flag_key: flag.key.clone(),
                        value: FlagValue::Boolean(false),
                        reason: EvaluationReason::Dependency,
                        variant: None,
                        timestamp: now,
                    });
                }
            }
        }

        // Evaluate targeting rules
        let context_map = context.to_map();
        let mut sorted_rules = flag.rules.clone();
        sorted_rules.sort_by(|a, b| b.priority.cmp(&a.priority));

        for rule in sorted_rules {
            let all_match = rule.conditions.iter()
                .all(|condition| condition.evaluate(&context_map));

            if all_match {
                return Ok(EvaluationResult {
                    flag_key: flag.key.clone(),
                    value: rule.value.clone(),
                    reason: EvaluationReason::TargetingMatch { rule: rule.name },
                    variant: None,
                    timestamp: now,
                });
            }
        }

        // Check percentage rollout
        if let Some(percentage) = flag.percentage_rollout {
            if let Some(ref user_id) = context.user_id {
                let hash = Self::hash_user(&flag.key, user_id);
                let bucket = (hash % 100) as f64;

                if bucket < percentage {
                    return Ok(EvaluationResult {
                        flag_key: flag.key.clone(),
                        value: flag.default_value.clone(),
                        reason: EvaluationReason::PercentageRollout,
                        variant: None,
                        timestamp: now,
                    });
                } else {
                    return Ok(EvaluationResult {
                        flag_key: flag.key.clone(),
                        value: FlagValue::Boolean(false),
                        reason: EvaluationReason::PercentageRollout,
                        variant: None,
                        timestamp: now,
                    });
                }
            }
        }

        // Return default value
        Ok(EvaluationResult {
            flag_key: flag.key.clone(),
            value: flag.default_value.clone(),
            reason: EvaluationReason::Default,
            variant: None,
            timestamp: now,
        })
    }

    // Evaluate all flags
    pub fn evaluate_all(
        &mut self,
        context: &EvaluationContext,
    ) -> HashMap<String, EvaluationResult> {
        let mut results = HashMap::new();

        for (key, _) in self.flags.clone() {
            match self.evaluate(&key, context) {
                Ok(result) => {
                    results.insert(key, result);
                }
                Err(e) => {
                    warn!(
                        flag = key,
                        error = %e,
                        "Failed to evaluate flag"
                    );
                }
            }
        }

        results
    }

    // Hash user ID for percentage bucketing
    fn hash_user(flag_key: &str, user_id: &str) -> u32 {
        let combined = format!("{}{}", flag_key, user_id);
        let mut hash: u32 = 0;
        for byte in combined.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(byte as u32);
        }
        hash
    }

    // Clear evaluation cache
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}