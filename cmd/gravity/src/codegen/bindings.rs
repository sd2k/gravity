use std::collections::BTreeMap;

use genco::{prelude::*, tokens::Tokens};
use wit_bindgen_core::wit_parser::{Resolve, SizeAlign, World};

use crate::{
    codegen::{
        FactoryGenerator,
        factory::FactoryConfig,
        imports::{ImportAnalyzer, ImportCodeGenerator},
        ir::AnalyzedImports,
        wasm::{Wasm, WasmData},
    },
    go::GoIdentifier,
};

/// The WIT bindings for a world.
pub struct Bindings<'a> {
    resolve: &'a Resolve,
    world: &'a World,
    /// The cumulative output tokens containing the Go bindings.
    // TODO(#16): Don't use the internal bindings.out field
    pub out: Tokens<Go>,

    /// The identifier of the Go variable containing the WebAssembly bytes.
    raw_wasm_var: GoIdentifier,

    /// The sizes of the architecture.
    sizes: &'a SizeAlign,
}

impl<'a> Bindings<'a> {
    /// Creates a new bindings generator for the selected world.
    pub fn new(resolve: &'a Resolve, world: &'a World, sizes: &'a SizeAlign) -> Self {
        let world_name = &world.name;
        let wasm_var = GoIdentifier::private(format!("wasm-file-{world_name}"));
        Self {
            resolve,
            world,
            out: Tokens::new(),
            raw_wasm_var: wasm_var,
            sizes,
        }
    }

    /// Adds the given Wasm to the bindings.
    pub fn include_wasm(&mut self, wasm: WasmData) {
        Wasm::new(&self.raw_wasm_var, wasm).format_into(&mut self.out)
    }

    /// Generates the imports for the bindings.
    pub fn generate_imports(&mut self) -> (AnalyzedImports, BTreeMap<String, Tokens<Go>>) {
        let analyzer = ImportAnalyzer::new(self.resolve, self.world);
        let analyzed = analyzer.analyze();

        let generator = ImportCodeGenerator::new(self.resolve, &analyzed, self.sizes);
        let import_chains = generator.import_chains();
        generator.format_into(&mut self.out);
        (analyzed, import_chains)
    }

    /// Generates the factory and instantiate functions, including any
    /// required interfaces.
    pub fn generate_factory(
        &mut self,
        analyzed_imports: &AnalyzedImports,
        import_chains: BTreeMap<String, Tokens<Go>>,
    ) {
        let config = FactoryConfig {
            analyzed_imports,
            import_chains,
            wasm_var_name: &self.raw_wasm_var,
        };
        FactoryGenerator::new(config).format_into(&mut self.out)
    }
}
