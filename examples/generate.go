package examples

//go:generate cargo build -p example-basic --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-records --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-iface-method-returns-string --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-instructions --target wasm32-unknown-unknown --release
//go:generate sh -c "RUSTFLAGS='--cfg getrandom_backend=\"custom\"' cargo build -q -p example-outlier --target wasm32-unknown-unknown --release"
//go:generate cargo build -p example-resources --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-resources-simple --target wasm32-unknown-unknown --release
//go:generate cargo build -p example-tuples --target wasm32-unknown-unknown --release

//go:generate cargo run --bin gravity -- --world basic --output ./basic/basic.go --wit-file ./basic/wit/basic.wit ../target/wasm32-unknown-unknown/release/example_basic.wasm
//go:generate cargo run --bin gravity -- --world example --output ./iface-method-returns-string/example.go --wit-file ./iface-method-returns-string/wit/example.wit ../target/wasm32-unknown-unknown/release/example_iface_method_returns_string.wasm
//go:generate cargo run --bin gravity -- --world instructions --output ./instructions/bindings.go --wit-file ./instructions/wit/instructions.wit ../target/wasm32-unknown-unknown/release/example_instructions.wasm
//go:generate cargo run --bin gravity -- --world outlier --output ./outlier/outlier.go --wit-file ./outlier/wit/world.wit ../target/wasm32-unknown-unknown/release/example_outlier.wasm
//go:generate cargo run --bin gravity -- --world resources --output ./resources/resources.go --wit-file ./resources/wit/resources.wit ../target/wasm32-unknown-unknown/release/example_resources.wasm
//go:generate cargo run --bin gravity -- --world resources --output ./resources-simple/resources.go --wit-file ./resources-simple/wit/world.wit ../target/wasm32-unknown-unknown/release/example_resources_simple.wasm
//go:generate cargo run --bin gravity -- --world tuples --output ./tuples/tuples.go --wit-file ./tuples/wit/tuple.wit ../target/wasm32-unknown-unknown/release/example_tuples.wasm
