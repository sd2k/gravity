package examples

//go:generate cargo build -p example-basic --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-records --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-iface-method-returns-string --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-instructions --target wasm32-unknown-unknown --release
// This quoting was a nightmare to get right, had to use a subshell in the end.
//go:generate sh -c "RUSTFLAGS='--cfg getrandom_backend=\"custom\"' cargo build -p example-outlier --target wasm32-unknown-unknown --release"

//go:generate cargo run --bin gravity -- --world basic --output ./basic/basic.go ../target/wasm32-unknown-unknown/release/example_basic.wasm
//go:generate cargo run --bin gravity -- --world records --output ./records/records.go ../target/wasm32-unknown-unknown/release/example_records.wasm
//go:generate cargo run --bin gravity -- --world example --output ./iface-method-returns-string/example.go ../target/wasm32-unknown-unknown/release/example_iface_method_returns_string.wasm
//go:generate cargo run --bin gravity -- --world instructions --output ./instructions/bindings.go ../target/wasm32-unknown-unknown/release/example_instructions.wasm
//go:generate cargo run --bin gravity -- --world outlier --output ./outlier/bindings.go ../target/wasm32-unknown-unknown/release/example_outlier.wasm
