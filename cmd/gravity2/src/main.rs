use std::{fs, process::ExitCode};

use anyhow::{Context, Result};
use clap::{Arg, ArgAction, Command};
use genco::prelude::*;
use gravity_codegen::GenerationContext;
use gravity_go::{Go, GoResult, GoType};
use heck::ToUpperCamelCase;
use wit_bindgen_core::wit_parser::{Resolve, Type, TypeDefKind, WorldItem};

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
        .map(|(module, bindgen)| (module.unwrap_or(wasm.clone()), bindgen))
        .context("File should be a valid WebAssembly module")?;

    // Get the world
    let world_id = bindgen
        .resolve
        .worlds
        .iter()
        .find(|(_, w)| w.name == *selected_world)
        .map(|(id, _)| id)
        .ok_or_else(|| anyhow::anyhow!("World '{}' not found", selected_world))?;

    let world = bindgen.resolve.worlds.get(world_id).unwrap();

    // Create generation context
    let mut context = GenerationContext::new();

    let package_name = selected_world.replace('-', "_");

    // TODO: Generate imports based on what's actually used
    quote_in! { context.out =>
        import (
            $(quoted("context"))
            $(quoted("embed"))
            $(quoted("errors"))
            $(quoted("github.com/tetratelabs/wazero"))
            $(quoted("github.com/tetratelabs/wazero/api"))
        )
        $['\n']
    };

    // Generate embedded WASM if requested
    if inline_wasm {
        quote_in! { context.out =>
            //go:embed $(hex::encode(&module))
            var rawWasm []byte
            $['\n']
        };
    } else {
        quote_in! { context.out =>
            //go:embed $(format!("{}.wasm", selected_world))
            var rawWasm []byte
            $['\n']
        };
    }

    // Generate factory and instance types
    let factory_name = format!("{}Factory", selected_world.to_upper_camel_case());
    let instance_name = format!("{}Instance", selected_world.to_upper_camel_case());
    let factory_name_str = factory_name.as_str();
    let instance_name_str = instance_name.as_str();

    quote_in! { context.out =>
        type $factory_name_str struct {
            runtime wazero.Runtime
            module  wazero.CompiledModule
        }
        $['\n']
        type $instance_name_str struct {
            module api.Module
        }
        $['\n']
    };

    // Process imports (guest imports from host)
    for (_name, import) in world.imports.iter() {
        match import {
            WorldItem::Interface { .. } => {
                // TODO: Generate interface imports
                quote_in! { context.out =>
                    // TODO: Interface import
                    $['\n']
                };
            }
            WorldItem::Function(func) => {
                // Generate import function
                let _func_name = func.name.to_upper_camel_case();

                // TODO: Process function parameters and results
                quote_in! { context.out =>
                    // Import function: $(func.name.as_str())
                    // TODO: Generate import binding
                    $['\n']
                };
            }
            WorldItem::Type(type_id) => {
                // TODO: Generate type definition
                let type_name = bindgen
                    .resolve
                    .types
                    .get(*type_id)
                    .and_then(|t| t.name.as_deref())
                    .unwrap_or("anonymous");
                quote_in! { context.out =>
                    // Type: $type_name
                    // TODO: Generate type binding
                    $['\n']
                };
            }
        }
    }

    // Process exports (guest exports to host)
    for (_name, export) in world.exports.iter() {
        match export {
            WorldItem::Interface { .. } => {
                // TODO: Generate interface exports
                quote_in! { context.out =>
                    // TODO: Interface export
                    $['\n']
                };
            }
            WorldItem::Function(func) => {
                // Generate export function
                let func_name = func.name.to_upper_camel_case();

                // Create a simplified function context
                let result = func
                    .result
                    .as_ref()
                    .map(|t| resolve_type(t, &bindgen.resolve))
                    .map(GoResult::Anon)
                    .unwrap_or(GoResult::Empty);

                // TODO: Use the instruction handler system properly
                let result_str = format_result(&result);
                quote_in! { context.out =>
                    func (i *$instance_name_str) $func_name(ctx context.Context) $result_str {
                        // TODO: Generate proper function body using instruction handlers
                        panic("not implemented")
                    }
                    $['\n']
                };
            }
            WorldItem::Type(type_id) => {
                // TODO: Generate type definition
                let type_name = bindgen
                    .resolve
                    .types
                    .get(*type_id)
                    .and_then(|t| t.name.as_deref())
                    .unwrap_or("anonymous");
                quote_in! { context.out =>
                    // Type: $type_name
                    // TODO: Generate type binding
                    $['\n']
                };
            }
        }
    }

    // Generate factory constructor
    quote_in! { context.out =>
        func New$factory_name_str(ctx context.Context) (*$factory_name_str, error) {
            runtime := wazero.NewRuntime(ctx)
            module, err := runtime.CompileModule(ctx, rawWasm)
            if err != nil {
                return nil, err
            }
            return &$factory_name_str{
                runtime: runtime,
                module:  module,
            }, nil
        }
        $['\n']
        func (f *$factory_name_str) Instantiate(ctx context.Context) (*$instance_name_str, error) {
            module, err := f.runtime.InstantiateModule(ctx, f.module, wazero.NewModuleConfig())
            if err != nil {
                return nil, err
            }
            return &$instance_name_str{module: module}, nil
        }
        $['\n']
        func (f *$factory_name_str) Close(ctx context.Context) error {
            return f.runtime.Close(ctx)
        }
        $['\n']
        func (i *$instance_name_str) Close(ctx context.Context) error {
            return i.module.Close(ctx)
        }
    };

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

// TODO: Move these helper functions to appropriate crates
fn resolve_type(typ: &Type, resolve: &Resolve) -> GoType {
    match typ {
        Type::Bool => GoType::Bool,
        Type::U8 => GoType::Uint8,
        Type::U16 => GoType::Uint16,
        Type::U32 => GoType::Uint32,
        Type::U64 => GoType::Uint64,
        Type::S8 => GoType::Int8,
        Type::S16 => GoType::Int16,
        Type::S32 => GoType::Int32,
        Type::S64 => GoType::Int64,
        Type::F32 => GoType::Float32,
        Type::F64 => GoType::Float64,
        Type::String => GoType::String,
        Type::Char => GoType::Uint32, // Char is represented as uint32
        Type::ErrorContext => GoType::Interface, // TODO: Handle ErrorContext properly
        Type::Id(id) => {
            let typedef = resolve.types.get(*id).unwrap();
            match &typedef.kind {
                TypeDefKind::List(inner) => GoType::Slice(Box::new(resolve_type(inner, resolve))),
                TypeDefKind::Option(inner) => {
                    GoType::ValueOrOk(Box::new(resolve_type(inner, resolve)))
                }
                TypeDefKind::Result(result) => {
                    match (&result.ok, &result.err) {
                        (Some(ok), None) => GoType::ValueOrOk(Box::new(resolve_type(ok, resolve))),
                        (None, Some(err)) => {
                            GoType::ValueOrError(Box::new(resolve_type(err, resolve)))
                        }
                        _ => GoType::Interface, // TODO: Handle other result cases
                    }
                }
                TypeDefKind::Variant(_) => GoType::Interface,
                TypeDefKind::Enum(_) => GoType::Uint32, // Enums are represented as integers
                TypeDefKind::Record(_) | TypeDefKind::Flags(_) | TypeDefKind::Tuple(_) => {
                    GoType::UserDefined(
                        typedef
                            .name
                            .clone()
                            .unwrap_or_else(|| "Anonymous".to_string()),
                    )
                }
                TypeDefKind::Type(t) => resolve_type(t, resolve),
                _ => GoType::Interface,
            }
        }
    }
}

fn format_result(result: &GoResult) -> String {
    match result {
        GoResult::Empty => String::new(),
        GoResult::Anon(typ) => format_type(typ),
    }
}

fn format_type(typ: &GoType) -> String {
    match typ {
        GoType::Bool => "bool".to_string(),
        GoType::Uint8 => "uint8".to_string(),
        GoType::Uint16 => "uint16".to_string(),
        GoType::Uint32 => "uint32".to_string(),
        GoType::Uint64 => "uint64".to_string(),
        GoType::Int8 => "int8".to_string(),
        GoType::Int16 => "int16".to_string(),
        GoType::Int32 => "int32".to_string(),
        GoType::Int64 => "int64".to_string(),
        GoType::Float32 => "float32".to_string(),
        GoType::Float64 => "float64".to_string(),
        GoType::String => "string".to_string(),
        GoType::Error => "error".to_string(),
        GoType::Interface => "interface{}".to_string(),
        GoType::Pointer(inner) => format!("*{}", format_type(inner)),
        GoType::ValueOrOk(inner) => format!("({}, bool)", format_type(inner)),
        GoType::ValueOrError(inner) => format!("({}, error)", format_type(inner)),
        GoType::Slice(inner) => format!("[]{}", format_type(inner)),
        GoType::UserDefined(name) => name.to_upper_camel_case(),
        GoType::Nothing => "".to_string(),
    }
}
