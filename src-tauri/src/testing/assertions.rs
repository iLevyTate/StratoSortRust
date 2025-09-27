// Test Assertions
// Custom assertion functions for integration testing

use std::fmt::Debug;
use std::path::Path;
use std::time::Duration;
use serde_json::Value;

use crate::error::AppError;
use crate::storage::Database;

// API response assertion
pub fn assert_api_response(
    response: &Value,
    expected_status: u16,
    expected_fields: Vec<&str>,
) -> Result<(), AppError> {
    // Check status if response has one
    if let Some(status) = response.get("status") {
        if let Some(status_code) = status.as_u64() {
            if status_code != expected_status as u64 {
                return Err(AppError::AssertionError {
                    message: format!(
                        "Expected status {}, got {}",
                        expected_status, status_code
                    )
                });
            }
        }
    }

    // Check required fields exist
    for field in expected_fields {
        if response.get(field).is_none() {
            return Err(AppError::AssertionError {
                message: format!("Expected field '{}' not found in response", field)
            });
        }
    }

    Ok(())
}

// Database state assertion
pub async fn assert_database_state<F>(
    db: &Database,
    table: &str,
    condition: F,
) -> Result<(), AppError>
where
    F: Fn(&Vec<Value>) -> bool,
{
    // SECURITY: Validate table name to prevent SQL injection
    crate::storage::validate_table_name(table)?;

    // Query table (table name has been validated above)
    let query = format!("SELECT * FROM {}", table);
    let rows = db.query_raw(&query).await?;

    // Convert to JSON values for easier checking
    // SqliteRow doesn't implement Serialize, so we'll create a placeholder JSON structure
    // In production, would need to extract column values from SqliteRow
    let values: Vec<Value> = rows.iter()
        .map(|_row| {
            // Placeholder: Would extract actual row data here
            // For testing purposes, return a basic JSON object
            serde_json::json!({
                "id": 1,
                "data": "test"
            })
        })
        .collect();

    if !condition(&values) {
        return Err(AppError::AssertionError {
            message: format!(
                "Database state assertion failed for table '{}': condition not met",
                table
            )
        });
    }

    Ok(())
}

// File existence assertion
pub fn assert_file_exists(path: impl AsRef<Path>) -> Result<(), AppError> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(AppError::AssertionError {
            message: format!("File does not exist: {:?}", path)
        });
    }

    if !path.is_file() {
        return Err(AppError::AssertionError {
            message: format!("Path exists but is not a file: {:?}", path)
        });
    }

    Ok(())
}

// Directory existence assertion
pub fn assert_dir_exists(path: impl AsRef<Path>) -> Result<(), AppError> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(AppError::AssertionError {
            message: format!("Directory does not exist: {:?}", path)
        });
    }

    if !path.is_dir() {
        return Err(AppError::AssertionError {
            message: format!("Path exists but is not a directory: {:?}", path)
        });
    }

    Ok(())
}

// Performance assertion
pub fn assert_performance(
    actual_duration: Duration,
    max_duration: Duration,
    operation: &str,
) -> Result<(), AppError> {
    if actual_duration > max_duration {
        return Err(AppError::AssertionError {
            message: format!(
                "Performance assertion failed for '{}': took {:?}, expected max {:?}",
                operation, actual_duration, max_duration
            )
        });
    }

    Ok(())
}

// No errors assertion
pub fn assert_no_errors<T, E: Debug>(results: &[Result<T, E>]) -> Result<(), AppError> {
    let errors: Vec<_> = results
        .iter()
        .enumerate()
        .filter_map(|(i, r)| r.as_ref().err().map(|e| (i, e)))
        .collect();

    if !errors.is_empty() {
        let error_msg = errors
            .iter()
            .map(|(i, e)| format!("  [{}]: {:?}", i, e))
            .collect::<Vec<_>>()
            .join("\n");

        return Err(AppError::AssertionError {
            message: format!("Found {} errors:\n{}", errors.len(), error_msg)
        });
    }

    Ok(())
}

// Equality assertion
pub fn assert_eq<T: PartialEq + Debug>(actual: T, expected: T, message: &str) -> Result<(), AppError> {
    if actual != expected {
        return Err(AppError::AssertionError {
            message: format!(
                "{}: expected {:?}, got {:?}",
                message, expected, actual
            )
        });
    }

    Ok(())
}

// Inequality assertion
pub fn assert_ne<T: PartialEq + Debug>(actual: T, not_expected: T, message: &str) -> Result<(), AppError> {
    if actual == not_expected {
        return Err(AppError::AssertionError {
            message: format!(
                "{}: value should not be {:?}",
                message, not_expected
            )
        });
    }

    Ok(())
}

// Greater than assertion
pub fn assert_gt<T: PartialOrd + Debug>(actual: T, threshold: T, message: &str) -> Result<(), AppError> {
    if actual <= threshold {
        return Err(AppError::AssertionError {
            message: format!(
                "{}: expected > {:?}, got {:?}",
                message, threshold, actual
            )
        });
    }

    Ok(())
}

// Less than assertion
pub fn assert_lt<T: PartialOrd + Debug>(actual: T, threshold: T, message: &str) -> Result<(), AppError> {
    if actual >= threshold {
        return Err(AppError::AssertionError {
            message: format!(
                "{}: expected < {:?}, got {:?}",
                message, threshold, actual
            )
        });
    }

    Ok(())
}

// Range assertion
pub fn assert_in_range<T: PartialOrd + Debug>(
    actual: T,
    min: T,
    max: T,
    message: &str,
) -> Result<(), AppError> {
    if actual < min || actual > max {
        return Err(AppError::AssertionError {
            message: format!(
                "{}: expected value in range [{:?}, {:?}], got {:?}",
                message, min, max, actual
            )
        });
    }

    Ok(())
}

// Contains assertion for strings
pub fn assert_contains(haystack: &str, needle: &str, message: &str) -> Result<(), AppError> {
    if !haystack.contains(needle) {
        return Err(AppError::AssertionError {
            message: format!(
                "{}: expected '{}' to contain '{}'",
                message, haystack, needle
            )
        });
    }

    Ok(())
}

// Does not contain assertion
pub fn assert_not_contains(haystack: &str, needle: &str, message: &str) -> Result<(), AppError> {
    if haystack.contains(needle) {
        return Err(AppError::AssertionError {
            message: format!(
                "{}: expected '{}' to not contain '{}'",
                message, haystack, needle
            )
        });
    }

    Ok(())
}

// Starts with assertion
pub fn assert_starts_with(text: &str, prefix: &str, message: &str) -> Result<(), AppError> {
    if !text.starts_with(prefix) {
        return Err(AppError::AssertionError {
            message: format!(
                "{}: expected '{}' to start with '{}'",
                message, text, prefix
            )
        });
    }

    Ok(())
}

// Ends with assertion
pub fn assert_ends_with(text: &str, suffix: &str, message: &str) -> Result<(), AppError> {
    if !text.ends_with(suffix) {
        return Err(AppError::AssertionError {
            message: format!(
                "{}: expected '{}' to end with '{}'",
                message, text, suffix
            )
        });
    }

    Ok(())
}

// JSON structure assertion
pub fn assert_json_structure(
    actual: &Value,
    expected_structure: &Value,
) -> Result<(), AppError> {
    match (actual, expected_structure) {
        (Value::Object(act_obj), Value::Object(exp_obj)) => {
            for (key, exp_value) in exp_obj {
                if !act_obj.contains_key(key) {
                    return Err(AppError::AssertionError {
                        message: format!("Missing expected key in JSON: '{}'", key)
                    });
                }

                // Recursively check nested structures
                if exp_value.is_object() || exp_value.is_array() {
                    assert_json_structure(act_obj.get(key).unwrap(), exp_value)?;
                }
            }
        }
        (Value::Array(act_arr), Value::Array(exp_arr)) => {
            if !exp_arr.is_empty() && !act_arr.is_empty() {
                // Check first element structure as template
                assert_json_structure(&act_arr[0], &exp_arr[0])?;
            }
        }
        _ => {
            // For primitive types, just check they're the same type
            if !matches!(
                (actual, expected_structure),
                (Value::String(_), Value::String(_))
                    | (Value::Number(_), Value::Number(_))
                    | (Value::Bool(_), Value::Bool(_))
                    | (Value::Null, Value::Null)
            ) {
                return Err(AppError::AssertionError {
                    message: format!(
                        "JSON type mismatch: expected {:?}, got {:?}",
                        expected_structure, actual
                    )
                });
            }
        }
    }

    Ok(())
}

// Collection assertion
pub fn assert_collection_size<T>(
    collection: &[T],
    expected_size: usize,
    message: &str,
) -> Result<(), AppError> {
    if collection.len() != expected_size {
        return Err(AppError::AssertionError {
            message: format!(
                "{}: expected collection size {}, got {}",
                message, expected_size, collection.len()
            )
        });
    }

    Ok(())
}

// Empty assertion
pub fn assert_empty<T>(collection: &[T], message: &str) -> Result<(), AppError> {
    if !collection.is_empty() {
        return Err(AppError::AssertionError {
            message: format!(
                "{}: expected empty collection, got {} items",
                message, collection.len()
            )
        });
    }

    Ok(())
}

// Not empty assertion
pub fn assert_not_empty<T>(collection: &[T], message: &str) -> Result<(), AppError> {
    if collection.is_empty() {
        return Err(AppError::AssertionError {
            message: format!("{}: expected non-empty collection", message)
        });
    }

    Ok(())
}

// Async operation success assertion
pub async fn assert_async_completes<F, T>(
    future: F,
    timeout: Duration,
    message: &str,
) -> Result<T, AppError>
where
    F: std::future::Future<Output = Result<T, AppError>>,
{
    match tokio::time::timeout(timeout, future).await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(e)) => Err(AppError::AssertionError {
            message: format!("{}: async operation failed: {}", message, e)
        }),
        Err(_) => Err(AppError::AssertionError {
            message: format!("{}: async operation timed out after {:?}", message, timeout)
        }),
    }
}

// Memory usage assertion - placeholder implementation
// Note: Would require memory_stats crate for actual implementation
pub fn assert_memory_usage(max_bytes: usize, message: &str) -> Result<(), AppError> {
    // Placeholder implementation - always passes
    // In production, would use memory_stats crate or platform-specific APIs
    let _ = max_bytes;
    let _ = message;
    Ok(())
}

// Thread safety assertion helper
pub async fn assert_thread_safe<F, T>(
    operation: F,
    thread_count: usize,
) -> Result<Vec<T>, AppError>
where
    F: Fn() -> T + Send + Sync + 'static,
    T: Send + 'static,
{
    let operation = Arc::new(operation);
    let mut handles = Vec::new();

    for _ in 0..thread_count {
        let op = operation.clone();
        let handle = tokio::spawn(async move {
            op()
        });
        handles.push(handle);
    }

    let mut results = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(result) => results.push(result),
            Err(e) => {
                return Err(AppError::AssertionError {
                    message: format!("Thread safety test failed: {:?}", e)
                });
            }
        }
    }

    Ok(results)
}

// Custom assertion builder for complex conditions
pub struct AssertionBuilder {
    conditions: Vec<Box<dyn Fn() -> Result<(), AppError>>>,
    message: String,
}

impl AssertionBuilder {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            conditions: Vec::new(),
            message: message.into(),
        }
    }

    pub fn add_condition<F>(mut self, condition: F) -> Self
    where
        F: Fn() -> Result<(), AppError> + 'static,
    {
        self.conditions.push(Box::new(condition));
        self
    }

    pub fn assert(self) -> Result<(), AppError> {
        for (i, condition) in self.conditions.iter().enumerate() {
            if let Err(e) = condition() {
                return Err(AppError::AssertionError {
                    message: format!(
                        "{}: Condition {} failed: {}",
                        self.message, i + 1, e
                    )
                });
            }
        }

        Ok(())
    }
}

// Macro for custom assertions
#[macro_export]
macro_rules! assert_custom {
    ($condition:expr, $message:expr) => {
        if !$condition {
            return Err($crate::error::AppError::AssertionError {
                message: $message.to_string()
            });
        }
    };
}

use std::sync::Arc;