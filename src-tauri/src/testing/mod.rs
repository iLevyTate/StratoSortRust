// Testing Framework Module
// Provides comprehensive integration testing capabilities

pub mod framework;
pub mod fixtures;
pub mod assertions;
pub mod reporting;

pub use framework::{
    IntegrationTest,
    TestContext,
    TestRunner,
    TestResult,
    TestSuite,
    TestCase,
    TestOutcome,
    TestReport,
};

pub use fixtures::{
    TestFixture,
    DatabaseFixture,
    FileSystemFixture,
    ApiFixture,
    MockService,
};

pub use assertions::{
    assert_api_response,
    assert_database_state,
    assert_file_exists,
    assert_performance,
    assert_no_errors,
};

pub use reporting::{
    TestReporter,
    HtmlReporter,
    JsonReporter,
    ConsoleReporter,
    ReportFormat,
};