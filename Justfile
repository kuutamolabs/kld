# Lint rust
lint:
  cargo clippy --workspace --all-targets --all-features -- -D warnings
  cd ./mgr && cargo clippy --workspace --all-targets --all-features -- -D warnings
fix:
  cargo clippy --workspace --all-targets --all-features --fix --allow-dirty --allow-staged  -- -D warnings
  cd ./mgr && cargo clippy --allow-no-vcs --workspace --all-targets --all-features --fix --allow-dirty --allow-staged  -- -D warnings
  treefmt

test:
  cargo test --workspace --all-features
  cd ./mgr && cargo test --workspace --all-features

# Start up the servers for manual testing
manual:
  cargo test test_manual -- --ignored

# Continuously run cargo check as code changes
watch:
  cargo watch
