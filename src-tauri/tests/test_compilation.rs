// Minimal test to verify crate compilation
use stratosort::error::AppError;

#[test]
fn test_crate_loads() {
    // Just test that we can create an error
    let error = AppError::NotFound {
        message: "Test".to_string(),
    };
    assert!(!error.to_string().is_empty());
}