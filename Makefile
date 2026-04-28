.PHONY: build run test clean fmt clippy doc kernel user check

build: kernel user

kernel:
	cargo build -p omniagent-kernel

user:
	cargo build --workspace --exclude omniagent-kernel --target x86_64-unknown-linux-gnu

run:
	cargo bootimage --run

run-debug:
	cargo bootimage --run -- --s -S

check:
	cargo check --workspace --exclude omniagent-kernel --target x86_64-unknown-linux-gnu

test:
	cargo test --workspace --exclude omniagent-kernel --target x86_64-unknown-linux-gnu
	cargo test --target x86_64-unknown-linux-gnu -p omniagent-kernel -- --test-threads=1

clean:
	cargo clean && rm -rf target/bootimage

fmt:
	cargo fmt --all

clippy:
	cargo clippy --all-targets --target x86_64-unknown-linux-gnu -- -D warnings

doc:
	cargo doc --no-deps --all --open
