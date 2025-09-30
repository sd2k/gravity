use std::{fs, path::Path, process::ExitCode};

use clap::{Arg, ArgAction, Command};
use genco::{
    lang::{Go, go},
    quote_in,
};
use wit_bindgen_core::{
    abi::{AbiVariant, LiftLower},
    wit_parser::{SizeAlign, WorldItem},
};

use arcjet_gravity::{
    Func,
    codegen::{Bindings, WasmData},
    go::{GoIdentifier, GoResult, GoType},
    resolve_type,
};

// `wit_component::decode` uses `root` as an arbitrary name for the primary
// world name, see
// 1. https://github.com/bytecodealliance/wasm-tools/blob/585a0bdd8f49fc05d076effaa96e63d97f420578/crates/wit-component/src/decoding.rs#L144-L147
// 2. https://github.com/bytecodealliance/wasm-tools/issues/1315
pub const PRIMARY_WORLD_NAME: &str = "root";

fn main() -> Result<ExitCode, ()> {
    let cmd = Command::new("gravity")
        .arg(
            Arg::new("world")
                .short('w')
                .long("world")
                .help("generate host bindings for the specified world")
                .default_value(PRIMARY_WORLD_NAME),
        )
        .arg(
            Arg::new("inline-wasm")
                .long("inline-wasm")
                .help("include the WebAssembly file as hex bytes in the output code")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("file")
                .help("the WebAssembly file to process")
                .required(true),
        )
        .arg(
            Arg::new("output")
                .help("the file path where output generated code should be output")
                .short('o')
                .long("output"),
        );

    let matches = cmd.get_matches();
    let selected_world = matches
        .get_one::<String>("world")
        .expect("should have a world");
    let file = matches
        .get_one::<String>("file")
        .expect("should have a file");
    let inline_wasm = matches.get_flag("inline-wasm");
    let output = matches.get_one::<String>("output");

    // Load the file specified as the `file` arg to clap
    let wasm = match fs::read(file) {
        Ok(wasm) => wasm,
        Err(_) => {
            eprintln!("unable to read file: {file}");
            return Ok(ExitCode::FAILURE);
        }
    };

    let (module, bindgen) = wit_component::metadata::decode(&wasm)
        // If the Wasm doesn't have a custom section, None will be returned so we need to use the original
        .map(|(module, bindgen)| (module.unwrap_or(wasm), bindgen))
        .expect("file should be a valid WebAssembly module");

    let wasm_file = &format!("{}.wasm", selected_world.replace('-', "_"));

    let instance = &GoIdentifier::public(format!("{selected_world}-instance"));

    let context = &go::import("context", "Context");

    let (_, world) = bindgen
        .resolve
        .worlds
        .iter()
        .find(|(_, world)| world.name == *selected_world)
        .expect("world {selected_world} not found");

    let mut sizes = SizeAlign::default();
    sizes.fill(&bindgen.resolve);
    let mut bindings = Bindings::new(&bindgen.resolve, world, &sizes);

    bindings.include_wasm(if inline_wasm {
        WasmData::Inline(&module)
    } else {
        WasmData::Embedded(wasm_file)
    });

    let (imports, chains) = bindings.generate_imports();
    bindings.generate_factory(&imports, chains);

    // TODO: refactor exports into separate generators, too.
    for world_item in world.exports.values() {
        match world_item {
            WorldItem::Function(func) => {
                let mut params: Vec<(GoIdentifier, GoType)> = Vec::with_capacity(func.params.len());
                for (name, wit_type) in func.params.iter() {
                    let go_type = resolve_type(wit_type, &bindgen.resolve);
                    match go_type {
                        // We can't represent this as an argument type so we unwrap the Some type
                        // TODO: Figure out a better way to handle this
                        GoType::ValueOrOk(typ) => params.push((GoIdentifier::local(name), *typ)),
                        typ => params.push((GoIdentifier::local(name), typ)),
                    }
                }

                let mut sizes = SizeAlign::default();
                sizes.fill(&bindgen.resolve);

                let result = match &func.result {
                    Some(wit_type) => {
                        let go_type = resolve_type(wit_type, &bindgen.resolve);
                        GoResult::Anon(go_type)
                    }
                    None => GoResult::Empty,
                };

                let mut f = Func::export(result, &sizes);
                wit_bindgen_core::abi::call(
                    &bindgen.resolve,
                    AbiVariant::GuestExport,
                    LiftLower::LowerArgsLiftResults,
                    func,
                    &mut f,
                    // async is not currently supported
                    false,
                );

                let arg_assignments = f
                    .args()
                    .iter()
                    .zip(params.iter())
                    .map(|(arg, (param, _))| (arg, param))
                    .collect::<Vec<(&String, &GoIdentifier)>>();

                let fn_name = &GoIdentifier::public(&func.name);
                // TODO(#16): Don't use the internal bindings.out field
                quote_in! { bindings.out =>
                    $['\n']
                    func (i *$instance) $fn_name(
                        $['\r']
                        ctx $context,
                        $(for (name, typ) in params.iter() join ($['\r']) => $name $typ,)
                    ) $(f.result()) {
                        $(for (arg, param) in arg_assignments join ($['\r']) => $arg := $param)
                        $(f.body())
                    }
                };
            }
            WorldItem::Interface { .. } => (),
            WorldItem::Type(_) => (),
        }
    }

    let header = "// Code generated by arcjet-gravity; DO NOT EDIT.\n\n".to_string();
    let mut w = genco::fmt::FmtWriter::new(header);
    let fmt = genco::fmt::Config::from_lang::<Go>().with_indentation(genco::fmt::Indentation::Tab);
    let config = go::Config::default().with_package(selected_world.replace('-', "_"));

    // TODO(#16): Don't use the internal bindings.out field
    bindings
        .out
        .format_file(&mut w.as_formatter(&fmt), &config)
        .unwrap();

    match output {
        Some(outpath) => {
            if !inline_wasm {
                let wasm_outpath = Path::new(outpath).with_file_name(wasm_file);
                match fs::write(&wasm_outpath, module) {
                    Ok(_) => (),
                    Err(_) => {
                        eprintln!("failed to create file: {}", wasm_outpath.to_string_lossy());
                        return Ok(ExitCode::FAILURE);
                    }
                }
            }
            match fs::write(outpath, w.into_inner()) {
                Ok(_) => Ok(ExitCode::SUCCESS),
                Err(_) => {
                    eprintln!("failed to create file: {outpath}");
                    Ok(ExitCode::FAILURE)
                }
            }
        }
        None => {
            println!("{}", w.into_inner());
            Ok(ExitCode::SUCCESS)
        }
    }
}
