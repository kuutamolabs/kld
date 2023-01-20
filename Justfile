# Lint rust
lint:
  cargo clippy --workspace --all-targets --all-features -- -D warnings
fix:
  treefmt
  cargo clippy --workspace --all-targets --all-features --fix --allow-dirty --allow-staged  -- -D warnings

test:
  cargo test --all --all-features

# Start up the servers for manual testing
manual:
  cargo test test_manual -- --ignored

# Continously run cargo check as code changes
watch:
  cargo watch
