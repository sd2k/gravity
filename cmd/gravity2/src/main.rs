use std::{fs, process::ExitCode};

use anyhow::{Context, Result};
use clap::{Arg, ArgAction, Command};
use genco::prelude::*;
use gravity_codegen::{
    exports::{ExportConfig, ExportGenerator},
    generate_imports_with_chains,
    imports::GoImports,
    FactoryConfig, FactoryGenerator, GenerationContext, TypeGenerator,
};
use gravity_go::{embed, Go, GoIdentifier};

// TODO: Move to gravity-cli crate
const PRIMARY_WORLD_NAME: &str = "root";

fn main() -> Result<ExitCode> {
    let cmd = Command::new("gravity2")
        .about("Generate Go bindings for WebAssembly components (refactored version)")
        .arg(
            Arg::new("world")
                .short('w')
                .long("world")
                .help("Generate host bindings for the specified world")
                .default_value(PRIMARY_WORLD_NAME),
        )
        .arg(
            Arg::new("inline-wasm")
                .long("inline-wasm")
                .help("Include the WebAssembly file as hex bytes in the output code")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("file")
                .help("The WebAssembly file to process")
                .required(true),
        )
        .arg(
            Arg::new("output")
                .help("The file path where output generated code should be written")
                .short('o')
                .long("output"),
        );

    let matches = cmd.get_matches();
    let selected_world = matches
        .get_one::<String>("world")
        .expect("world should have a default value");
    let file = matches.get_one::<String>("file").expect("file is required");
    let inline_wasm = matches.get_flag("inline-wasm");
    let output = matches.get_one::<String>("output");

    // Load the WebAssembly file
    let wasm = fs::read(file).context(format!("Failed to read file: {file}"))?;

    // Decode the component metadata
    let (module, bindgen) = wit_component::metadata::decode(&wasm)
        .map(|(module, bindgen)| (module.unwrap_or(wasm), bindgen))
        .context("File should be a valid WebAssembly module")?;

    // Get the world
    eprintln!("Looking for world: {}", selected_world);
    eprintln!("Available worlds:");
    for (_, world) in bindgen.resolve.worlds.iter() {
        eprintln!("  - {}", world.name);
    }

    let world_id = bindgen
        .resolve
        .worlds
        .iter()
        .find(|(_, w)| w.name == *selected_world)
        .map(|(id, _)| id)
        .or_else(|| {
            // If the requested world is not found, try to find any world
            eprintln!(
                "World '{}' not found. Using first available world.",
                selected_world
            );
            // Use the first available world if the requested one doesn't exist
            bindgen.resolve.worlds.iter().next().map(|(id, _)| id)
        })
        .ok_or_else(|| anyhow::anyhow!("No worlds found in the WebAssembly component"))?;

    let world = bindgen.resolve.worlds.get(world_id).unwrap();
    eprintln!("Using world: {}", world.name);
    eprintln!("World has {} imports", world.imports.len());
    eprintln!("World has {} exports", world.exports.len());

    // Create generation context
    let mut context = GenerationContext::new();

    let package_name = selected_world.replace('-', "_");
    let wasm_file = format!("{}.wasm", &package_name);

    // Generate embedded WASM
    let wasm_var_name = &GoIdentifier::Private {
        name: &format!("wasm-file-{}", &selected_world),
    };
    if inline_wasm {
        let hex_rows = module
            .chunks(16)
            .map(|bytes| {
                quote! {
                    $(for b in bytes join ( ) => $(format!("0x{b:02x},")))
                }
            })
            .collect::<Vec<Tokens<Go>>>();

        // TODO(#16): Don't use the internal bindings.out field
        quote_in! { context.out =>
            var $wasm_var_name = []byte{
                $(for row in hex_rows join ($['\r']) => $row)
            }
        };
    } else {
        // TODO(#16): Don't use the internal bindings.out field
        quote_in! { context.out =>
            import _ "embed"
            $['\n']
            $(embed(wasm_file))
            var $wasm_var_name []byte
            $['\n']
        }
    }

    // Create GoImports first so we can pass it to generate_imports
    let go_imports = GoImports::new();

    // Generate imports using the library function (this generates interface definitions and import chains)
    let import_result = generate_imports_with_chains(
        &mut context,
        &bindgen.resolve,
        &world.name,
        &world.imports,
        &go_imports,
    )
    .context("Failed to generate imports")?;

    let imported_interfaces = import_result.interface_params;
    let import_chains = import_result.import_chains;

    // Generate types (records, variants, enums, etc.)
    let type_generator = TypeGenerator::new(&mut context, &bindgen.resolve);
    type_generator
        .generate_world_types(&world.exports, &world.imports)
        .context("Failed to generate types")?;

    // Generate factory and instance types using the library
    let factory_config = FactoryConfig {
        world_name: &selected_world,
        go_imports: &go_imports,
        interface_params: imported_interfaces,
        import_chains,
        wasm_var_name,
    };

    let factory_generator = FactoryGenerator::new(&mut context, factory_config);
    let instance_name = factory_generator
        .generate()
        .context("Failed to generate factory")?;

    // Generate exports (guest exports to host)
    eprintln!("DEBUG: world.exports.len() = {}", world.exports.len());
    for (key, item) in world.exports.iter() {
        eprintln!(
            "DEBUG: Export key: {:?}, item: {:?}",
            key,
            std::mem::discriminant(item)
        );
    }
    if !world.exports.is_empty() {
        let export_config = ExportConfig {
            world_name: &selected_world,
            instance_name: &instance_name,
            go_imports: &go_imports,
        };

        let export_generator = ExportGenerator::new(&mut context, export_config, &bindgen.resolve);
        export_generator
            .generate(&world.exports)
            .context("Failed to generate exports")?;
    }

    // Format and write output
    let mut writer = genco::fmt::FmtWriter::new(String::new());
    let fmt = genco::fmt::Config::from_lang::<Go>().with_indentation(genco::fmt::Indentation::Tab);
    let config = genco::lang::go::Config::default().with_package(package_name);

    context
        .out
        .format_file(&mut writer.as_formatter(&fmt), &config)
        .context("Failed to format Go code")?;

    let generated_code = writer.into_inner();

    match output {
        Some(outpath) => {
            // Write WASM file if not inlined
            if !inline_wasm {
                let wasm_outpath = std::path::Path::new(outpath)
                    .with_file_name(format!("{}.wasm", selected_world));
                fs::write(&wasm_outpath, &module).context(format!(
                    "Failed to write WASM file: {}",
                    wasm_outpath.display()
                ))?;
            }

            // Write generated Go code
            fs::write(outpath, generated_code)
                .context(format!("Failed to write output file: {outpath}"))?;
        }
        None => {
            println!("{}", generated_code);
        }
    }

    Ok(ExitCode::SUCCESS)
}
