// Test Reporting
// Generates and formats test reports in various formats

use std::collections::HashMap;
use std::path::Path;

use crate::error::AppError;
use super::framework::{TestResult, TestReport, TestOutcome};

// Report format enumeration
#[derive(Debug, Clone)]
pub enum ReportFormat {
    Console,
    Json,
    Html,
    Xml,
    Markdown,
}

// Base test reporter trait
pub trait TestReporter: Send + Sync {
    fn generate(&self, report: &TestReport) -> Result<String, AppError>;
    fn save(&self, report: &TestReport, path: &Path) -> Result<(), AppError>;
}

// Console reporter for terminal output
pub struct ConsoleReporter {
    config: ConsoleReporterConfig,
}

#[derive(Debug, Clone)]
pub struct ConsoleReporterConfig {
    pub use_colors: bool,
    pub verbose: bool,
    pub show_timings: bool,
    pub show_logs: bool,
}

impl Default for ConsoleReporterConfig {
    fn default() -> Self {
        Self {
            use_colors: true,
            verbose: false,
            show_timings: true,
            show_logs: false,
        }
    }
}

impl ConsoleReporter {
    pub fn new(config: ConsoleReporterConfig) -> Self {
        Self { config }
    }

    fn format_outcome(&self, outcome: &TestOutcome) -> String {
        if self.config.use_colors {
            match outcome {
                TestOutcome::Passed => "\x1b[32m✓ PASSED\x1b[0m".to_string(),
                TestOutcome::Failed { reason } => format!("\x1b[31m✗ FAILED: {}\x1b[0m", reason),
                TestOutcome::Skipped { reason } => format!("\x1b[33m⊘ SKIPPED: {}\x1b[0m", reason),
                TestOutcome::Timeout => "\x1b[31m⏱ TIMEOUT\x1b[0m".to_string(),
                TestOutcome::Panicked { message } => format!("\x1b[31m💥 PANICKED: {}\x1b[0m", message),
            }
        } else {
            match outcome {
                TestOutcome::Passed => "PASSED".to_string(),
                TestOutcome::Failed { reason } => format!("FAILED: {}", reason),
                TestOutcome::Skipped { reason } => format!("SKIPPED: {}", reason),
                TestOutcome::Timeout => "TIMEOUT".to_string(),
                TestOutcome::Panicked { message } => format!("PANICKED: {}", message),
            }
        }
    }

    fn format_duration(&self, duration: &std::time::Duration) -> String {
        if duration.as_secs() > 0 {
            format!("{:.2}s", duration.as_secs_f64())
        } else {
            format!("{}ms", duration.as_millis())
        }
    }
}

impl TestReporter for ConsoleReporter {
    fn generate(&self, report: &TestReport) -> Result<String, AppError> {
        let mut output = Vec::new();

        // Header
        output.push("=".repeat(80));
        output.push(format!("Test Report - Generated at {}", report.generated_at.format("%Y-%m-%d %H:%M:%S UTC")));
        output.push("=".repeat(80));
        output.push(String::new());

        // Summary
        let success_rate = report.success_rate();
        let summary_color = if self.config.use_colors {
            if success_rate == 100.0 {
                "\x1b[32m"
            } else if success_rate >= 80.0 {
                "\x1b[33m"
            } else {
                "\x1b[31m"
            }
        } else {
            ""
        };

        output.push(format!(
            "{}Summary: {} tests, {} passed, {} failed, {} skipped ({:.1}% success rate){}",
            summary_color,
            report.total_tests,
            report.passed,
            report.failed,
            report.skipped,
            success_rate,
            if self.config.use_colors { "\x1b[0m" } else { "" }
        ));

        if self.config.show_timings {
            output.push(format!("Total duration: {}", self.format_duration(&report.duration)));
        }
        output.push(String::new());

        // Group results by suite
        let mut by_suite: HashMap<String, Vec<&TestResult>> = HashMap::new();
        for result in &report.results {
            by_suite.entry(result.suite_name.clone())
                .or_default()
                .push(result);
        }

        // Display results by suite
        for (suite_name, results) in by_suite.iter() {
            output.push(format!("Suite: {}", suite_name));
            output.push("-".repeat(40));

            for result in results {
                let mut line = format!("  {} - {}",
                    result.test_name,
                    self.format_outcome(&result.outcome)
                );

                if self.config.show_timings {
                    line.push_str(&format!(" ({})", self.format_duration(&result.duration)));
                }

                output.push(line);

                // Show verbose details if configured
                if self.config.verbose {
                    // Show assertions
                    if !result.assertions.is_empty() {
                        output.push("    Assertions:".to_string());
                        for assertion in &result.assertions {
                            let marker = if assertion.passed { "✓" } else { "✗" };
                            output.push(format!("      {} {}", marker, assertion.message));
                        }
                    }

                    // Show logs if configured
                    if self.config.show_logs && !result.logs.is_empty() {
                        output.push("    Logs:".to_string());
                        for log in &result.logs {
                            output.push(format!("      {}", log));
                        }
                    }
                }
            }
            output.push(String::new());
        }

        // Failed tests detail
        let failed = report.failed_tests();
        if !failed.is_empty() {
            output.push("Failed Tests:".to_string());
            output.push("-".repeat(40));
            for test in failed {
                output.push(format!("  {}/{}", test.suite_name, test.test_name));
                if let TestOutcome::Failed { reason } = &test.outcome {
                    output.push(format!("    Reason: {}", reason));
                }
            }
            output.push(String::new());
        }

        // Slowest tests
        let slowest = report.slowest_tests(5);
        if !slowest.is_empty() && self.config.show_timings {
            output.push("Slowest Tests:".to_string());
            output.push("-".repeat(40));
            for test in slowest {
                output.push(format!("  {}/{} - {}",
                    test.suite_name,
                    test.test_name,
                    self.format_duration(&test.duration)
                ));
            }
            output.push(String::new());
        }

        Ok(output.join("\n"))
    }

    fn save(&self, report: &TestReport, path: &Path) -> Result<(), AppError> {
        let content = self.generate(report)?;
        std::fs::write(path, content)
            .map_err(|e| AppError::IoError {
                message: format!("Failed to save report: {}", e)
            })
    }
}

// JSON reporter for machine-readable output
pub struct JsonReporter {
    pretty: bool,
}

impl JsonReporter {
    pub fn new(pretty: bool) -> Self {
        Self { pretty }
    }
}

impl TestReporter for JsonReporter {
    fn generate(&self, report: &TestReport) -> Result<String, AppError> {
        let json = if self.pretty {
            serde_json::to_string_pretty(report)
        } else {
            serde_json::to_string(report)
        };

        json.map_err(|e| AppError::SerializationError {
            message: format!("Failed to serialize report: {}", e)
        })
    }

    fn save(&self, report: &TestReport, path: &Path) -> Result<(), AppError> {
        let content = self.generate(report)?;
        std::fs::write(path, content)
            .map_err(|e| AppError::IoError {
                message: format!("Failed to save report: {}", e)
            })
    }
}

// HTML reporter for browser viewing
pub struct HtmlReporter {
    template: HtmlTemplate,
}

#[derive(Debug, Clone)]
pub struct HtmlTemplate {
    pub title: String,
    pub include_charts: bool,
    pub include_logs: bool,
    pub theme: HtmlTheme,
}

#[derive(Debug, Clone)]
pub enum HtmlTheme {
    Light,
    Dark,
    Auto,
}

impl Default for HtmlTemplate {
    fn default() -> Self {
        Self {
            title: "Test Report".to_string(),
            include_charts: true,
            include_logs: false,
            theme: HtmlTheme::Auto,
        }
    }
}

impl HtmlReporter {
    pub fn new(template: HtmlTemplate) -> Self {
        Self { template }
    }

    fn generate_css(&self) -> String {
        let theme_css = match self.template.theme {
            HtmlTheme::Dark => r#"
                :root {
                    --bg-color: #1a1a1a;
                    --text-color: #e0e0e0;
                    --border-color: #333;
                    --success-color: #4caf50;
                    --failure-color: #f44336;
                    --warning-color: #ff9800;
                }
            "#,
            HtmlTheme::Light => r#"
                :root {
                    --bg-color: #ffffff;
                    --text-color: #333333;
                    --border-color: #ddd;
                    --success-color: #4caf50;
                    --failure-color: #f44336;
                    --warning-color: #ff9800;
                }
            "#,
            HtmlTheme::Auto => r#"
                @media (prefers-color-scheme: dark) {
                    :root {
                        --bg-color: #1a1a1a;
                        --text-color: #e0e0e0;
                        --border-color: #333;
                        --success-color: #4caf50;
                        --failure-color: #f44336;
                        --warning-color: #ff9800;
                    }
                }
                @media (prefers-color-scheme: light) {
                    :root {
                        --bg-color: #ffffff;
                        --text-color: #333333;
                        --border-color: #ddd;
                        --success-color: #4caf50;
                        --failure-color: #f44336;
                        --warning-color: #ff9800;
                    }
                }
            "#,
        };

        format!(r#"
            {}
            body {{
                font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
                background-color: var(--bg-color);
                color: var(--text-color);
                margin: 0;
                padding: 20px;
                line-height: 1.6;
            }}
            .container {{
                max-width: 1200px;
                margin: 0 auto;
            }}
            .header {{
                border-bottom: 2px solid var(--border-color);
                padding-bottom: 20px;
                margin-bottom: 30px;
            }}
            .summary {{
                display: grid;
                grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
                gap: 20px;
                margin-bottom: 30px;
            }}
            .summary-card {{
                background: var(--bg-color);
                border: 1px solid var(--border-color);
                border-radius: 8px;
                padding: 15px;
                text-align: center;
            }}
            .test-suite {{
                margin-bottom: 30px;
                border: 1px solid var(--border-color);
                border-radius: 8px;
                overflow: hidden;
            }}
            .suite-header {{
                background: var(--border-color);
                padding: 10px 15px;
                font-weight: bold;
            }}
            .test-result {{
                padding: 10px 15px;
                border-bottom: 1px solid var(--border-color);
                display: flex;
                justify-content: space-between;
                align-items: center;
            }}
            .test-result:last-child {{
                border-bottom: none;
            }}
            .passed {{ color: var(--success-color); }}
            .failed {{ color: var(--failure-color); }}
            .skipped {{ color: var(--warning-color); }}
            .duration {{
                font-size: 0.9em;
                opacity: 0.7;
            }}
            .chart-container {{
                width: 100%;
                max-width: 400px;
                margin: 20px auto;
            }}
        "#, theme_css)
    }

    fn generate_chart_js(&self, report: &TestReport) -> String {
        if !self.template.include_charts {
            return String::new();
        }

        format!(r#"
            <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
            <script>
                const ctx = document.getElementById('resultChart').getContext('2d');
                new Chart(ctx, {{
                    type: 'doughnut',
                    data: {{
                        labels: ['Passed', 'Failed', 'Skipped'],
                        datasets: [{{
                            data: [{}, {}, {}],
                            backgroundColor: ['#4caf50', '#f44336', '#ff9800']
                        }}]
                    }},
                    options: {{
                        responsive: true,
                        maintainAspectRatio: false
                    }}
                }});
            </script>
        "#, report.passed, report.failed, report.skipped)
    }
}

impl TestReporter for HtmlReporter {
    fn generate(&self, report: &TestReport) -> Result<String, AppError> {
        let mut html = String::new();

        // HTML structure
        html.push_str(&format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{}</title>
    <style>{}</style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>{}</h1>
            <p>Generated at {}</p>
        </div>

        <div class="summary">
            <div class="summary-card">
                <h3>Total Tests</h3>
                <div class="stat">{}</div>
            </div>
            <div class="summary-card passed">
                <h3>Passed</h3>
                <div class="stat">{}</div>
            </div>
            <div class="summary-card failed">
                <h3>Failed</h3>
                <div class="stat">{}</div>
            </div>
            <div class="summary-card skipped">
                <h3>Skipped</h3>
                <div class="stat">{}</div>
            </div>
            <div class="summary-card">
                <h3>Success Rate</h3>
                <div class="stat">{:.1}%</div>
            </div>
            <div class="summary-card">
                <h3>Duration</h3>
                <div class="stat">{:?}</div>
            </div>
        </div>
"#,
            self.template.title,
            self.generate_css(),
            self.template.title,
            report.generated_at.format("%Y-%m-%d %H:%M:%S UTC"),
            report.total_tests,
            report.passed,
            report.failed,
            report.skipped,
            report.success_rate(),
            report.duration
        ));

        // Chart if enabled
        if self.template.include_charts {
            html.push_str(r#"
        <div class="chart-container">
            <canvas id="resultChart" width="400" height="400"></canvas>
        </div>
            "#);
        }

        // Group results by suite
        let mut by_suite: HashMap<String, Vec<&TestResult>> = HashMap::new();
        for result in &report.results {
            by_suite.entry(result.suite_name.clone())
                .or_default()
                .push(result);
        }

        // Test results by suite
        for (suite_name, results) in by_suite.iter() {
            html.push_str(&format!(r#"
        <div class="test-suite">
            <div class="suite-header">{}</div>
"#, suite_name));

            for result in results {
                let status_class = match result.outcome {
                    TestOutcome::Passed => "passed",
                    TestOutcome::Failed { .. } => "failed",
                    TestOutcome::Skipped { .. } => "skipped",
                    _ => "failed",
                };

                let status_text = match &result.outcome {
                    TestOutcome::Passed => "✓ Passed",
                    TestOutcome::Failed { reason } => &format!("✗ Failed: {}", reason),
                    TestOutcome::Skipped { reason } => &format!("⊘ Skipped: {}", reason),
                    TestOutcome::Timeout => "⏱ Timeout",
                    TestOutcome::Panicked { message } => &format!("💥 Panicked: {}", message),
                };

                html.push_str(&format!(r#"
            <div class="test-result">
                <div>
                    <span class="test-name">{}</span>
                    <span class="{}">{}</span>
                </div>
                <div class="duration">{:?}</div>
            </div>
"#, result.test_name, status_class, status_text, result.duration));
            }

            html.push_str("        </div>\n");
        }

        // Close HTML
        html.push_str(&format!(r#"
    </div>
    {}
</body>
</html>
"#, self.generate_chart_js(report)));

        Ok(html)
    }

    fn save(&self, report: &TestReport, path: &Path) -> Result<(), AppError> {
        let content = self.generate(report)?;
        std::fs::write(path, content)
            .map_err(|e| AppError::IoError {
                message: format!("Failed to save report: {}", e)
            })
    }
}

// Markdown reporter for documentation
pub struct MarkdownReporter {
    include_toc: bool,
}

impl MarkdownReporter {
    pub fn new(include_toc: bool) -> Self {
        Self { include_toc }
    }
}

impl TestReporter for MarkdownReporter {
    fn generate(&self, report: &TestReport) -> Result<String, AppError> {
        let mut md = Vec::new();

        // Title and metadata
        md.push("# Test Report".to_string());
        md.push(String::new());
        md.push(format!("**Generated:** {}", report.generated_at.format("%Y-%m-%d %H:%M:%S UTC")));
        md.push(format!("**Duration:** {:?}", report.duration));
        md.push(String::new());

        // Table of contents if enabled
        if self.include_toc {
            md.push("## Table of Contents".to_string());
            md.push(String::new());
            md.push("- [Summary](#summary)".to_string());
            md.push("- [Test Results](#test-results)".to_string());
            if report.failed > 0 {
                md.push("- [Failed Tests](#failed-tests)".to_string());
            }
            md.push("- [Performance](#performance)".to_string());
            md.push(String::new());
        }

        // Summary
        md.push("## Summary".to_string());
        md.push(String::new());
        md.push("| Metric | Value |".to_string());
        md.push("|--------|-------|" .to_string());
        md.push(format!("| Total Tests | {} |", report.total_tests));
        md.push(format!("| Passed | {} |", report.passed));
        md.push(format!("| Failed | {} |", report.failed));
        md.push(format!("| Skipped | {} |", report.skipped));
        md.push(format!("| Success Rate | {:.1}% |", report.success_rate()));
        md.push(String::new());

        // Test results
        md.push("## Test Results".to_string());
        md.push(String::new());

        // Group by suite
        let mut by_suite: HashMap<String, Vec<&TestResult>> = HashMap::new();
        for result in &report.results {
            by_suite.entry(result.suite_name.clone())
                .or_default()
                .push(result);
        }

        for (suite_name, results) in by_suite.iter() {
            md.push(format!("### {}", suite_name));
            md.push(String::new());
            md.push("| Test | Status | Duration |".to_string());
            md.push("|------|--------|----------|".to_string());

            for result in results {
                let status = match &result.outcome {
                    TestOutcome::Passed => "✅ Passed",
                    TestOutcome::Failed { .. } => "❌ Failed",
                    TestOutcome::Skipped { .. } => "⚠️ Skipped",
                    TestOutcome::Timeout => "⏱️ Timeout",
                    TestOutcome::Panicked { .. } => "💥 Panicked",
                };

                md.push(format!("| {} | {} | {:?} |", result.test_name, status, result.duration));
            }
            md.push(String::new());
        }

        // Failed tests detail
        if report.failed > 0 {
            md.push("## Failed Tests".to_string());
            md.push(String::new());

            for test in report.failed_tests() {
                md.push(format!("#### {}/{}", test.suite_name, test.test_name));
                if let TestOutcome::Failed { reason } = &test.outcome {
                    md.push(String::new());
                    md.push(format!("**Reason:** {}", reason));
                }
                md.push(String::new());
            }
        }

        // Performance
        md.push("## Performance".to_string());
        md.push(String::new());
        md.push("### Slowest Tests".to_string());
        md.push(String::new());
        md.push("| Test | Duration |".to_string());
        md.push("|------|----------|".to_string());

        for test in report.slowest_tests(10) {
            md.push(format!("| {}/{} | {:?} |",
                test.suite_name,
                test.test_name,
                test.duration
            ));
        }

        Ok(md.join("\n"))
    }

    fn save(&self, report: &TestReport, path: &Path) -> Result<(), AppError> {
        let content = self.generate(report)?;
        std::fs::write(path, content)
            .map_err(|e| AppError::IoError {
                message: format!("Failed to save report: {}", e)
            })
    }
}

// Multi-format reporter that generates multiple formats
pub struct MultiReporter {
    reporters: Vec<Box<dyn TestReporter>>,
}

impl Default for MultiReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiReporter {
    pub fn new() -> Self {
        Self {
            reporters: Vec::new(),
        }
    }

    pub fn add_reporter(mut self, reporter: Box<dyn TestReporter>) -> Self {
        self.reporters.push(reporter);
        self
    }
}

impl TestReporter for MultiReporter {
    fn generate(&self, report: &TestReport) -> Result<String, AppError> {
        // Return console output as default
        if let Some(reporter) = self.reporters.first() {
            reporter.generate(report)
        } else {
            Ok(report.summary())
        }
    }

    fn save(&self, report: &TestReport, base_path: &Path) -> Result<(), AppError> {
        for (i, reporter) in self.reporters.iter().enumerate() {
            let ext = match i {
                0 => "txt",
                1 => "json",
                2 => "html",
                3 => "md",
                _ => &format!("report{}", i),
            };

            let path = base_path.with_extension(ext);
            reporter.save(report, &path)?;
        }

        Ok(())
    }
}