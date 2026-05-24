#!/bin/bash
# Local mirror of the PR-gating CI run in .github/workflows/ci.yml. Run this
# before pushing to avoid round-tripping through GitHub Actions for trivial
# breakages. Exits non-zero on the first failure — matches CI's gate
# semantics, not a smoke run that swallows errors.

set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> rustfmt --check"
(cd src-tauri && cargo fmt -- --check)

echo "==> clippy"
(cd src-tauri && cargo clippy --no-deps)

echo "==> backend lib tests"
(cd src-tauri && cargo test --lib --no-fail-fast -- --nocapture)

echo "==> backend integration tests"
(cd src-tauri && \
  cargo test --test integration_tests --no-fail-fast -- --nocapture && \
  cargo test --test test_backend_fixes --no-fail-fast -- --nocapture && \
  cargo test --test test_smart_folder_integration --no-fail-fast -- --nocapture && \
  cargo test --test comprehensive_database_test --no-fail-fast -- --nocapture && \
  cargo test --test validate_test_structure --no-fail-fast -- --nocapture --test-threads=1 && \
  cargo test --test test_tauri_plugins --no-fail-fast -- --nocapture)

echo "==> frontend type check"
npm run check

echo "==> frontend unit tests"
npm run test:ci

echo "==> all CI gates passed locally"
