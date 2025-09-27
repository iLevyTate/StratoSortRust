// Feature Flag Definitions
// Core feature flag types and structures

use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

// Feature flag value types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FlagValue {
    Boolean(bool),
    String(String),
    Integer(i64),
    Float(f64),
    Json(serde_json::Value),
}

impl FlagValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Boolean(b) => Some(*b),
            Self::String(s) => match s.to_lowercase().as_str() {
                "true" | "yes" | "on" | "1" => Some(true),
                "false" | "no" | "off" | "0" => Some(false),
                _ => None,
            },
            Self::Integer(i) => Some(*i != 0),
            _ => None,
        }
    }

    pub fn as_string(&self) -> String {
        match self {
            Self::Boolean(b) => b.to_string(),
            Self::String(s) => s.clone(),
            Self::Integer(i) => i.to_string(),
            Self::Float(f) => f.to_string(),
            Self::Json(j) => j.to_string(),
        }
    }

    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Self::Integer(i) => Some(*i),
            Self::Float(f) => Some(*f as i64),
            Self::String(s) => s.parse().ok(),
            Self::Boolean(b) => Some(if *b { 1 } else { 0 }),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(f) => Some(*f),
            Self::Integer(i) => Some(*i as f64),
            Self::String(s) => s.parse().ok(),
            Self::Boolean(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }
}

// Flag status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FlagStatus {
    Active,
    Inactive,
    Scheduled,
    Deprecated,
    Experimental,
}

// Feature flag definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlag {
    // Identification
    pub key: String,
    pub name: String,
    pub description: String,

    // Configuration
    pub default_value: FlagValue,
    pub variants: HashMap<String, FlagValue>,
    pub status: FlagStatus,

    // Metadata
    pub tags: Vec<String>,
    pub owner: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    // Scheduling
    pub enabled_from: Option<DateTime<Utc>>,
    pub enabled_until: Option<DateTime<Utc>>,

    // Targeting
    pub rules: Vec<TargetingRule>,
    pub percentage_rollout: Option<f64>,

    // Dependencies
    pub depends_on: Vec<String>,
    pub conflicts_with: Vec<String>,
}

impl FeatureFlag {
    // Create a simple boolean flag
    pub fn boolean(key: String, default: bool) -> Self {
        Self {
            key: key.clone(),
            name: key.clone(),
            description: String::new(),
            default_value: FlagValue::Boolean(default),
            variants: HashMap::new(),
            status: FlagStatus::Active,
            tags: Vec::new(),
            owner: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            enabled_from: None,
            enabled_until: None,
            rules: Vec::new(),
            percentage_rollout: None,
            depends_on: Vec::new(),
            conflicts_with: Vec::new(),
        }
    }

    // Create a multi-variant flag
    pub fn multivariant(key: String, default: FlagValue, variants: HashMap<String, FlagValue>) -> Self {
        Self {
            key: key.clone(),
            name: key.clone(),
            description: String::new(),
            default_value: default,
            variants,
            status: FlagStatus::Active,
            tags: Vec::new(),
            owner: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            enabled_from: None,
            enabled_until: None,
            rules: Vec::new(),
            percentage_rollout: None,
            depends_on: Vec::new(),
            conflicts_with: Vec::new(),
        }
    }

    // Check if flag is currently active
    pub fn is_active(&self) -> bool {
        let now = Utc::now();

        // Check status
        if !matches!(self.status, FlagStatus::Active | FlagStatus::Experimental) {
            return false;
        }

        // Check schedule
        if let Some(from) = self.enabled_from {
            if now < from {
                return false;
            }
        }

        if let Some(until) = self.enabled_until {
            if now > until {
                return false;
            }
        }

        true
    }

    // Add a targeting rule
    pub fn add_rule(&mut self, rule: TargetingRule) {
        self.rules.push(rule);
        self.updated_at = Utc::now();
    }

    // Set percentage rollout
    pub fn set_rollout(&mut self, percentage: f64) {
        self.percentage_rollout = Some(percentage.clamp(0.0, 100.0));
        self.updated_at = Utc::now();
    }
}

// Targeting rule for flag evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetingRule {
    pub name: String,
    pub conditions: Vec<super::conditions::Condition>,
    pub value: FlagValue,
    pub priority: i32,
}

impl TargetingRule {
    pub fn new(name: String, value: FlagValue) -> Self {
        Self {
            name,
            conditions: Vec::new(),
            value,
            priority: 0,
        }
    }

    pub fn with_condition(mut self, condition: super::conditions::Condition) -> Self {
        self.conditions.push(condition);
        self
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }
}

// Flag collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlagCollection {
    pub flags: HashMap<String, FeatureFlag>,
    pub version: String,
    pub environment: String,
}

impl FlagCollection {
    pub fn new(environment: String) -> Self {
        Self {
            flags: HashMap::new(),
            version: "1.0.0".to_string(),
            environment,
        }
    }

    pub fn add_flag(&mut self, flag: FeatureFlag) {
        self.flags.insert(flag.key.clone(), flag);
    }

    pub fn get_flag(&self, key: &str) -> Option<&FeatureFlag> {
        self.flags.get(key)
    }

    pub fn remove_flag(&mut self, key: &str) -> Option<FeatureFlag> {
        self.flags.remove(key)
    }

    pub fn list_active_flags(&self) -> Vec<&FeatureFlag> {
        self.flags
            .values()
            .filter(|f| f.is_active())
            .collect()
    }
}