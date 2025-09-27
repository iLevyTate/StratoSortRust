// Integration Test Framework
// Core testing infrastructure for comprehensive application testing

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::path::PathBuf;
use tokio::sync::{RwLock, Mutex};
use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

use crate::error::AppError;
use crate::state::AppState;
use crate::storage::Database;

// Test outcome enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestOutcome {
    Passed,
    Failed { reason: String },
    Skipped { reason: String },
    Timeout,
    Panicked { message: String },
}

// Test result structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub test_name: String,
    pub suite_name: String,
    pub outcome: TestOutcome,
    pub duration: Duration,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub assertions: Vec<AssertionResult>,
    pub logs: Vec<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

// Assertion result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResult {
    pub passed: bool,
    pub message: String,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub location: String,
}

// Test context for sharing state
pub struct TestContext {
    pub app_state: Arc<AppState>,
    pub test_database: Arc<RwLock<Database>>,
    pub temp_dir: PathBuf,
    pub config: TestConfig,
    pub shared_data: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    logs: Arc<Mutex<Vec<String>>>,
}

impl TestContext {
    pub async fn new(config: TestConfig) -> Result<Self, AppError> {
        // Create temporary directory for test files
        let temp_dir_handle = tempfile::tempdir()
            .map_err(|e| AppError::IoError {
                message: format!("Failed to create temp dir: {}", e)
            })?;

        let temp_dir = temp_dir_handle.keep();

        // Create test database
        let db_path = temp_dir.join("test.db");
        let database = Database::new_with_path(db_path.to_str().unwrap()).await?;

        // Create test app state
        let app_state = AppState::new_for_testing().await?;

        Ok(Self {
            app_state: Arc::new(app_state),
            test_database: Arc::new(RwLock::new(database)),
            temp_dir,
            config,
            shared_data: Arc::new(RwLock::new(HashMap::new())),
            logs: Arc::new(Mutex::new(Vec::new())),
        })
    }

    // Log a message during test execution
    pub async fn log(&self, message: String) {
        let mut logs = self.logs.lock().await;
        logs.push(format!("[{}] {}", Utc::now().format("%H:%M:%S%.3f"), message));
    }

    // Set shared data between test steps
    pub async fn set_data(&self, key: String, value: serde_json::Value) {
        let mut data = self.shared_data.write().await;
        data.insert(key, value);
    }

    // Get shared data
    pub async fn get_data(&self, key: &str) -> Option<serde_json::Value> {
        let data = self.shared_data.read().await;
        data.get(key).cloned()
    }

    // Get all logs
    pub async fn get_logs(&self) -> Vec<String> {
        self.logs.lock().await.clone()
    }

    // Cleanup test resources
    pub async fn cleanup(&self) -> Result<(), AppError> {
        // Close database connection
        // Database cleanup handled by Drop trait

        // Remove temporary files
        if self.temp_dir.exists() {
            std::fs::remove_dir_all(&self.temp_dir)
                .map_err(|e| AppError::IoError {
                    message: format!("Failed to cleanup temp dir: {}", e)
                })?;
        }

        Ok(())
    }
}

// Test configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    pub timeout: Duration,
    pub retry_count: u32,
    pub parallel_execution: bool,
    pub verbose_output: bool,
    pub fail_fast: bool,
    pub filter: Option<String>,
    pub exclude: Option<Vec<String>>,
    pub environment: HashMap<String, String>,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            retry_count: 0,
            parallel_execution: true,
            verbose_output: false,
            fail_fast: false,
            filter: None,
            exclude: None,
            environment: HashMap::new(),
        }
    }
}

// Integration test trait
#[async_trait]
pub trait IntegrationTest: Send + Sync {
    fn name(&self) -> &str;
    fn suite(&self) -> &str;
    fn tags(&self) -> Vec<String> { Vec::new() }

    async fn setup(&self, _ctx: &TestContext) -> Result<(), AppError> {
        Ok(())
    }

    async fn execute(&self, ctx: &TestContext) -> Result<(), AppError>;

    async fn teardown(&self, _ctx: &TestContext) -> Result<(), AppError> {
        Ok(())
    }

    fn should_skip(&self, _config: &TestConfig) -> Option<String> {
        None
    }
}

// Test case implementation
pub struct TestCase {
    pub name: String,
    pub suite: String,
    pub test_fn: Arc<Box<dyn IntegrationTest>>,
}

// Type alias for setup/teardown functions
type TestSetupFn = Box<dyn Fn(&TestContext) -> Result<(), AppError> + Send + Sync>;

// Test suite
pub struct TestSuite {
    pub name: String,
    pub tests: Vec<TestCase>,
    pub setup: Option<TestSetupFn>,
    pub teardown: Option<TestSetupFn>,
}

impl TestSuite {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tests: Vec::new(),
            setup: None,
            teardown: None,
        }
    }

    pub fn add_test(&mut self, test: TestCase) {
        self.tests.push(test);
    }

    pub fn add_test_fn(&mut self, name: String, test_fn: Box<dyn IntegrationTest>) {
        self.tests.push(TestCase {
            name: name.clone(),
            suite: self.name.clone(),
            test_fn: Arc::new(test_fn),
        });
    }

    pub fn set_setup<F>(&mut self, setup: F)
    where
        F: Fn(&TestContext) -> Result<(), AppError> + Send + Sync + 'static,
    {
        self.setup = Some(Box::new(setup));
    }

    pub fn set_teardown<F>(&mut self, teardown: F)
    where
        F: Fn(&TestContext) -> Result<(), AppError> + Send + Sync + 'static,
    {
        self.teardown = Some(Box::new(teardown));
    }
}

// Test runner
pub struct TestRunner {
    suites: Vec<TestSuite>,
    config: TestConfig,
    results: Arc<RwLock<Vec<TestResult>>>,
}

impl TestRunner {
    pub fn new(config: TestConfig) -> Self {
        Self {
            suites: Vec::new(),
            config,
            results: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn add_suite(&mut self, suite: TestSuite) {
        self.suites.push(suite);
    }

    // Run all test suites
    pub async fn run(&self) -> TestReport {
        let start_time = Instant::now();
        let mut all_results = Vec::new();

        for suite in &self.suites {
            // Apply filter if specified
            if let Some(filter) = &self.config.filter {
                if !suite.name.contains(filter) {
                    continue;
                }
            }

            // Run suite
            let suite_results = self.run_suite(suite).await;
            all_results.extend(suite_results);

            // Fail fast if configured
            if self.config.fail_fast {
                let has_failure = all_results.iter().any(|r| matches!(r.outcome, TestOutcome::Failed { .. }));
                if has_failure {
                    break;
                }
            }
        }

        // Store results
        {
            let mut results = self.results.write().await;
            *results = all_results.clone();
        }

        // Generate report
        TestReport {
            total_tests: all_results.len(),
            passed: all_results.iter().filter(|r| matches!(r.outcome, TestOutcome::Passed)).count(),
            failed: all_results.iter().filter(|r| matches!(r.outcome, TestOutcome::Failed { .. })).count(),
            skipped: all_results.iter().filter(|r| matches!(r.outcome, TestOutcome::Skipped { .. })).count(),
            duration: start_time.elapsed(),
            results: all_results,
            generated_at: Utc::now(),
        }
    }

    // Run a single suite
    async fn run_suite(&self, suite: &TestSuite) -> Vec<TestResult> {
        let mut results = Vec::new();

        // Create context for suite (wrapped in Arc for sharing)
        let context = match TestContext::new(self.config.clone()).await {
            Ok(ctx) => Arc::new(ctx),
            Err(e) => {
                // If context creation fails, mark all tests as failed
                for test in &suite.tests {
                    results.push(TestResult {
                        test_name: test.name.clone(),
                        suite_name: suite.name.clone(),
                        outcome: TestOutcome::Failed {
                            reason: format!("Context creation failed: {}", e)
                        },
                        duration: Duration::from_secs(0),
                        started_at: Utc::now(),
                        finished_at: Utc::now(),
                        assertions: Vec::new(),
                        logs: Vec::new(),
                        metadata: HashMap::new(),
                    });
                }
                return results;
            }
        };

        // Run suite setup
        if let Some(setup) = &suite.setup {
            if let Err(e) = setup(&context) {
                // Suite setup failed, skip all tests
                for test in &suite.tests {
                    results.push(TestResult {
                        test_name: test.name.clone(),
                        suite_name: suite.name.clone(),
                        outcome: TestOutcome::Skipped {
                            reason: format!("Suite setup failed: {}", e)
                        },
                        duration: Duration::from_secs(0),
                        started_at: Utc::now(),
                        finished_at: Utc::now(),
                        assertions: Vec::new(),
                        logs: Vec::new(),
                        metadata: HashMap::new(),
                    });
                }
                return results;
            }
        }

        // Run tests
        if self.config.parallel_execution {
            // Parallel execution
            let mut handles = Vec::new();

            for test in &suite.tests {
                let test_name = test.name.clone();
                let suite_name = suite.name.clone();
                // Clone the Box pointer to avoid lifetime issues
                let test_fn = test.test_fn.clone();
                let context = context.clone(); // Already Arc wrapped
                let config = self.config.clone();

                let handle = tokio::spawn(async move {
                    Self::run_single_test(
                        test_name,
                        suite_name,
                        test_fn,
                        context,
                        config,
                    ).await
                });

                handles.push(handle);
            }

            for handle in handles {
                if let Ok(result) = handle.await {
                    results.push(result);
                }
            }
        } else {
            // Sequential execution
            for test in &suite.tests {
                let result = Self::run_single_test(
                    test.name.clone(),
                    suite.name.clone(),
                    test.test_fn.clone(),
                    context.clone(),
                    self.config.clone(),
                ).await;

                // Clone result before checking outcome to avoid move
                let outcome = result.outcome.clone();
                results.push(result);

                // Fail fast if configured
                if self.config.fail_fast && matches!(outcome, TestOutcome::Failed { .. }) {
                    break;
                }
            }
        }

        // Run suite teardown
        if let Some(teardown) = &suite.teardown {
            let _ = teardown(&context);
        }

        // Cleanup context
        let _ = context.cleanup().await;

        results
    }

    // Run a single test
    async fn run_single_test(
        test_name: String,
        suite_name: String,
        test: Arc<Box<dyn IntegrationTest>>,
        context: Arc<TestContext>,
        config: TestConfig,
    ) -> TestResult {
        let started_at = Utc::now();
        let start_time = Instant::now();

        // Check if test should be skipped
        if let Some(reason) = test.should_skip(&config) {
            return TestResult {
                test_name,
                suite_name,
                outcome: TestOutcome::Skipped { reason },
                duration: Duration::from_secs(0),
                started_at,
                finished_at: Utc::now(),
                assertions: Vec::new(),
                logs: Vec::new(),
                metadata: HashMap::new(),
            };
        }

        // Run test with timeout
        let timeout_duration = config.timeout;
        let test_future = async {
            // Setup
            if let Err(e) = test.setup(&context).await {
                return TestOutcome::Failed {
                    reason: format!("Setup failed: {}", e)
                };
            }

            // Execute
            match test.execute(&context).await {
                Ok(_) => TestOutcome::Passed,
                Err(e) => TestOutcome::Failed {
                    reason: format!("Test failed: {}", e)
                },
            }
        };

        let outcome = match tokio::time::timeout(timeout_duration, test_future).await {
            Ok(result) => result,
            Err(_) => TestOutcome::Timeout,
        };

        // Teardown (always run, even on failure)
        let _ = test.teardown(&context).await;

        // Get logs
        let logs = context.get_logs().await;

        TestResult {
            test_name,
            suite_name,
            outcome,
            duration: start_time.elapsed(),
            started_at,
            finished_at: Utc::now(),
            assertions: Vec::new(), // Would be populated by assertion tracking
            logs,
            metadata: HashMap::new(),
        }
    }

    // Get test results
    pub async fn get_results(&self) -> Vec<TestResult> {
        self.results.read().await.clone()
    }

    // Run tests matching a pattern
    pub async fn run_filtered(&mut self, pattern: &str) -> TestReport {
        self.config.filter = Some(pattern.to_string());
        self.run().await
    }

    // Run tests with specific tags
    pub async fn run_tagged(&self, tags: Vec<String>) -> TestReport {
        let start_time = Instant::now();
        let mut all_results = Vec::new();

        for suite in &self.suites {
            let mut suite_tests = Vec::new();

            // Filter tests by tags
            for test in &suite.tests {
                let test_tags = test.test_fn.tags();
                if tags.iter().any(|tag| test_tags.contains(tag)) {
                    suite_tests.push(test);
                }
            }

            if !suite_tests.is_empty() {
                // Create temporary suite with filtered tests
                let mut temp_suite = TestSuite::new(&suite.name);
                for test in suite_tests {
                    temp_suite.add_test(TestCase {
                        name: test.name.clone(),
                        suite: test.suite.clone(),
                        test_fn: test.test_fn.clone(),
                    });
                }

                let suite_results = self.run_suite(&temp_suite).await;
                all_results.extend(suite_results);
            }
        }

        TestReport {
            total_tests: all_results.len(),
            passed: all_results.iter().filter(|r| matches!(r.outcome, TestOutcome::Passed)).count(),
            failed: all_results.iter().filter(|r| matches!(r.outcome, TestOutcome::Failed { .. })).count(),
            skipped: all_results.iter().filter(|r| matches!(r.outcome, TestOutcome::Skipped { .. })).count(),
            duration: start_time.elapsed(),
            results: all_results,
            generated_at: Utc::now(),
        }
    }
}

// Test report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestReport {
    pub total_tests: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub duration: Duration,
    pub results: Vec<TestResult>,
    pub generated_at: DateTime<Utc>,
}

impl TestReport {
    pub fn success_rate(&self) -> f64 {
        if self.total_tests == 0 {
            return 0.0;
        }
        (self.passed as f64 / self.total_tests as f64) * 100.0
    }

    pub fn summary(&self) -> String {
        format!(
            "Test Results: {} total, {} passed, {} failed, {} skipped ({}% success rate) in {:?}",
            self.total_tests,
            self.passed,
            self.failed,
            self.skipped,
            self.success_rate(),
            self.duration
        )
    }

    pub fn failed_tests(&self) -> Vec<&TestResult> {
        self.results
            .iter()
            .filter(|r| matches!(r.outcome, TestOutcome::Failed { .. }))
            .collect()
    }

    pub fn slowest_tests(&self, n: usize) -> Vec<&TestResult> {
        let mut sorted = self.results.iter().collect::<Vec<_>>();
        sorted.sort_by(|a, b| b.duration.cmp(&a.duration));
        sorted.truncate(n);
        sorted
    }
}