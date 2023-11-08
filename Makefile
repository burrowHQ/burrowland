RFLAGS="-C link-arg=-s"

build: build-burrowland build-testoracle build-mock-ft

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

build-mock-ft: contracts/mock-ft
	rustup target add wasm32-unknown-unknown
	RUSTFLAGS=$(RFLAGS) cargo build -p mock-ft --target wasm32-unknown-unknown --release
	mkdir -p res
	cp target/wasm32-unknown-unknown/release/mock_ft.wasm ./res/mock_ft.wasm

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