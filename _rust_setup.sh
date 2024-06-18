#/bin/bash
VER=1.69.0
rustup toolchain install $VER
rustup default $VER
rustup target add wasm32-unknown-unknown
cargo build -p contract --target wasm32-unknown-unknown --release
cargo build -p test-oracle --target wasm32-unknown-unknown --release
cargo install wasm-opt --locked --version 0.116.0
wasm-opt -Oz -o target/wasm32-unknown-unknown/release/contract_by_wasm_opt.wasm  target/wasm32-unknown-unknown/release/contract.wasm