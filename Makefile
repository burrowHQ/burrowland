RFLAGS="-C link-arg=-s"

build: build-burrowland build-testoracle build-mock-ref-exchange build-mock-boost-farming build-mock-ft build-mock-pyth build-mock-rated-token build-mock-dcl

build-burrowland: contracts/contract
	rustup target add wasm32-unknown-unknown
	RUSTFLAGS=$(RFLAGS) cargo build -p contract --target wasm32-unknown-unknown --release
	mkdir -p res
	cp target/wasm32-unknown-unknown/release/contract.wasm ./res/burrowland.wasm

build-testoracle: contracts/test-oracle
	rustup target add wasm32-unknown-unknown
	RUSTFLAGS=$(RFLAGS) cargo build -p test-oracle --target wasm32-unknown-unknown --release
	mkdir -p res
	cp target/wasm32-unknown-unknown/release/test_oracle.wasm ./res/

build-mock-boost-farming: contracts/mock-boost-farming
	rustup target add wasm32-unknown-unknown
	RUSTFLAGS=$(RFLAGS) cargo build -p mock-boost-farming --target wasm32-unknown-unknown --release
	mkdir -p res
	cp target/wasm32-unknown-unknown/release/mock_boost_farming.wasm ./res/mock_boost_farming.wasm

build-mock-ref-exchange: contracts/mock-ref-exchange
	rustup target add wasm32-unknown-unknown
	RUSTFLAGS=$(RFLAGS) cargo build -p mock-ref-exchange --target wasm32-unknown-unknown --release
	mkdir -p res
	cp target/wasm32-unknown-unknown/release/mock_ref_exchange.wasm ./res/mock_ref_exchange.wasm

build-mock-ft: contracts/mock-ft
	rustup target add wasm32-unknown-unknown
	RUSTFLAGS=$(RFLAGS) cargo build -p mock-ft --target wasm32-unknown-unknown --release
	mkdir -p res
	cp target/wasm32-unknown-unknown/release/mock_ft.wasm ./res/mock_ft.wasm

build-mock-pyth: contracts/mock-pyth
	rustup target add wasm32-unknown-unknown
	RUSTFLAGS=$(RFLAGS) cargo build -p mock-pyth --target wasm32-unknown-unknown --release
	mkdir -p res
	cp target/wasm32-unknown-unknown/release/mock_pyth.wasm ./res/mock_pyth.wasm

build-mock-rated-token: contracts/mock-rated-token
	rustup target add wasm32-unknown-unknown
	RUSTFLAGS=$(RFLAGS) cargo build -p mock-rated-token --target wasm32-unknown-unknown --release
	mkdir -p res
	cp target/wasm32-unknown-unknown/release/mock_rated_token.wasm ./res/mock_rated_token.wasm

build-mock-dcl: contracts/mock-dcl
	rustup target add wasm32-unknown-unknown
	RUSTFLAGS=$(RFLAGS) cargo build -p mock-dcl --target wasm32-unknown-unknown --release
	mkdir -p res
	cp target/wasm32-unknown-unknown/release/mock_dcl.wasm ./res/mock_dcl.wasm

release:
	$(call docker_build,_rust_setup.sh)
	mkdir -p res
	cp target/wasm32-unknown-unknown/release/contract.wasm res/burrowland_release.wasm
	cp target/wasm32-unknown-unknown/release/test_oracle.wasm res/test_oracle_release.wasm

unittest: build
ifdef TC
	RUSTFLAGS=$(RFLAGS) cargo test $(TC) -p contract --lib -- --nocapture
else
	RUSTFLAGS=$(RFLAGS) cargo test -p contract --lib -- --nocapture
	RUSTFLAGS=$(RFLAGS) cargo test -p contract --lib -- --test-threads=1 --ignored --nocapture
endif

test: build
ifdef TF
	RUSTFLAGS=$(RFLAGS) cargo test -p contract --test $(TF) -- --nocapture
else
	RUSTFLAGS=$(RFLAGS) cargo test -p contract --tests -- --nocapture
endif

clean:
	cargo clean
	rm -rf res/

define docker_build
	docker build -t my-burrow-builder .
	docker run \
		--mount type=bind,source=${PWD},target=/host \
		--cap-add=SYS_PTRACE --security-opt seccomp=unconfined \
		-w /host \
		-e RUSTFLAGS=$(RFLAGS) \
		-i -t my-burrow-builder \
		/bin/bash $(1)
endef