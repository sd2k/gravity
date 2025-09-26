package examples

//go:generate cargo build -p example-basic --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-iface-method-returns-string --target wasm32-unknown-unknown --release

//go:generate cargo run --bin gravity -- --world basic --output ./basic/basic.go --wit-file ./basic/wit/basic.wit ../target/wasm32-unknown-unknown/release/example_basic.wasm
//go:generate cargo run --bin gravity -- --world example --output ./iface-method-returns-string/example.go --wit-file ./iface-method-returns-string/wit/example.wit ../target/wasm32-unknown-unknown/release/example_iface_method_returns_string.wasm
