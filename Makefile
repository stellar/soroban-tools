all: check build test

export RUSTFLAGS=-Dwarnings -Dclippy::all -Dclippy::pedantic

REPOSITORY_VERSION ?= "$(shell git describe --tags --always --abbrev=0 --match='v[0-9]*.[0-9]*.[0-9]*' 2> /dev/null | sed 's/^.//')"
REPOSITORY_COMMIT_HASH := "$(shell git rev-parse HEAD)"
REPOSITORY_BRANCH := "$(shell git rev-parse --abbrev-ref HEAD)"
BUILD_TIMESTAMP ?= $(shell date '+%Y-%m-%dT%H:%M:%S')
GOLDFLAGS :=	-X 'github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config.Version=${REPOSITORY_VERSION}' \
				-X 'github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config.CommitHash=${REPOSITORY_COMMIT_HASH}' \
				-X 'github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config.BuildTimestamp=${BUILD_TIMESTAMP}' \
				-X 'github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config.Branch=${REPOSITORY_BRANCH}'

# Always specify the build target so that libpreflight.a is always put into
# an architecture subdirectory (i.e. target/$(CARGO_BUILD_TARGET)/release-with-panic-unwind )
# Otherwise it will be much harder for Golang to find the library since
# it would need to distinguish when we are crosscompiling and when we are not
# (libpreflight.a is put at target/release-with-panic-unwind/ when not cross compiling)
CARGO_BUILD_TARGET ?= $(shell rustc -vV | sed -n 's|host: ||p')

# update the Cargo.lock every time the Cargo.toml changes.
Cargo.lock: Cargo.toml
	cargo update --workspace

install_rust: Cargo.lock
	cargo install --path ./cmd/soroban-cli

install: install_rust build-libpreflight
	go install -ldflags="${GOLDFLAGS}" ./...

build_rust: Cargo.lock
	cargo build

build: build_rust build-libpreflight
	go build -ldflags="${GOLDFLAGS}" ./...

build-libpreflight: Cargo.lock
	cd cmd/soroban-rpc/lib/preflight && cargo build --target $(CARGO_BUILD_TARGET) --profile release-with-panic-unwind

build-test-wasms: Cargo.lock
	cargo build --package 'test_*' --profile test-wasms --target wasm32-unknown-unknown

test: build-test-wasms
	cargo test --workspace

e2e-test:
	cargo test --test it -- --ignored

check: Cargo.lock
	cargo clippy --all-targets

watch:
	cargo watch --clear --watch-when-idle --shell '$(MAKE)'

fmt:
	cargo fmt --all

clean:
	cargo clean
	go clean ./...

publish:
	cargo workspaces publish --all --force '*' --from-git --yes

# the build-soroban-rpc build target is an optimized build target used by 
# https://github.com/stellar/pipelines/stellar-horizon/Jenkinsfile-soroban-rpc-package-builder
# as part of the package building.
build-soroban-rpc: build-libpreflight
	go build -ldflags="${GOLDFLAGS}" -o soroban-rpc -trimpath -v ./cmd/soroban-rpc

lint-changes:
	golangci-lint run ./... --new-from-rev $$(git rev-parse HEAD)

lint:
	golangci-lint run ./...

# PHONY lists all the targets that aren't file names, so that make would skip the timestamp based check.
.PHONY: publish clean fmt watch check e2e-test test build-test-wasms install build build-soroban-rpc build-libpreflight lint lint-changes
