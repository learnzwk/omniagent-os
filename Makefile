.PHONY: build run test clean fmt clippy doc kernel user check

build: kernel user

kernel:
	cargo build -p omniagent-kernel

user:
	cargo build --workspace --exclude omniagent-kernel

run:
	cargo bootimage --run

run-debug:
	cargo bootimage --run -- --s -S

check:
	cargo check --workspace

test:
	cargo test --workspace --exclude omniagent-kernel
	cargo bootimage --test

clean:
	cargo clean && rm -rf target/bootimage

fmt:
	cargo fmt --all

clippy:
	cargo clippy --all-targets -- -D warnings

doc:
	cargo doc --no-deps --all --open
