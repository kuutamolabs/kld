# Lint rust
lint:
  cargo clippy --all-targets --all-features -- -D warnings
fix:
  treefmt
  cargo clippy --all-targets --all-features --fix --allow-dirty --allow-staged  -- -D warnings

test:
  cargo test --all-targets --all-features
# Continously run cargo check as code changes
watch:
  cargo watch
