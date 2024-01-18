use std::collections::HashMap;

use wasm_encoder::{ConstExpr, ElementSegment};

fn map_element_items<'a>(
    items: wasmparser::ElementItems,
    functions: &'a mut Vec<u32>,
    const_exprs: &'a mut Vec<wasm_encoder::ConstExpr>,
) -> wasm_encoder::Elements<'a> {
    match items {
        wasmparser::ElementItems::Functions(f) => {
            functions.extend(f.into_iter().map(|x| x.unwrap()));
            wasm_encoder::Elements::Functions(functions)
        }
        wasmparser::ElementItems::Expressions(ref_, e) => {
            const_exprs.extend(e.into_iter().map(|x| x.unwrap().try_into().unwrap()));
            wasm_encoder::Elements::Expressions(ref_.try_into().unwrap(), const_exprs)
        }
    }
}
fn map_element_kind<'a>(
    e: wasmparser::ElementKind,
    offset: &'a mut Option<ConstExpr>, // just for storage. should be None
) -> wasm_encoder::ElementMode<'a> {
    match e {
        wasmparser::ElementKind::Passive => wasm_encoder::ElementMode::Passive,
        wasmparser::ElementKind::Active { table_index, offset_expr } => {
            wasm_encoder::ElementMode::Active {
                table: table_index,
                offset: {
                    offset.replace(offset_expr.try_into().unwrap());
                    offset.as_ref().unwrap()
                },
            }
        }
        wasmparser::ElementKind::Declared => wasm_encoder::ElementMode::Declared,
    }
}

pub async fn minify_symbol(wasm: &mut Vec<u8>, js: &mut Vec<u8>) {
    let parser = wasmparser::Parser::new(0);

    let mut module = wasm_encoder::Module::new();
    let mut imports_ident_map = HashMap::new();
    let mut exports_ident_map = HashMap::new();

    let mut module_ident = MinifiedIdent::new();
    let mut name_ident = MinifiedIdent::new();
    let mut export_ident = MinifiedIdent::new();

    let mut code_section_remaining = 0;
    let mut code_section_encoder = None;

    for payload in parser.parse_all(wasm) {
        let payload = payload.unwrap();
        match payload {
            wasmparser::Payload::TypeSection(section) => {
                let mut encoder = wasm_encoder::TypeSection::new();
                for ty in section {
                    let ty = ty.unwrap();
                    let types = ty.types().iter().cloned().map(|x| x.try_into().unwrap());
                    if ty.is_explicit_rec_group() {
                        encoder.rec(types);
                    } else {
                        let types = types.collect::<Vec<_>>();
                        assert_eq!(types.len(), 1);
                        encoder.subtype(&types[0]);
                    }
                }
                module.section(&encoder);
            }
            wasmparser::Payload::ImportSection(section) => {
                let mut encoder = wasm_encoder::ImportSection::new();
                for import in section {
                    let import = import.unwrap();
                    let (module_name, name_map) = imports_ident_map
                        .entry(import.module)
                        .or_insert_with(|| (module_ident.next().unwrap(), HashMap::new()));
                    let name = name_map
                        .entry(import.name)
                        .or_insert_with(|| name_ident.next().unwrap());
                    let ty: wasm_encoder::EntityType = import.ty.try_into().unwrap();
                    encoder.import(module_name, name, ty);
                }
                module.section(&encoder);
            }
            wasmparser::Payload::FunctionSection(section) => {
                let mut encoder = wasm_encoder::FunctionSection::new();
                for function in section {
                    encoder.function(function.unwrap());
                }
                module.section(&encoder);
            }
            wasmparser::Payload::TableSection(section) => {
                let mut encoder = wasm_encoder::TableSection::new();
                for table in section {
                    let table = table.unwrap();
                    let ty = table.ty.try_into().unwrap();
                    match table.init {
                        wasmparser::TableInit::RefNull => {
                            encoder.table(ty);
                        }
                        wasmparser::TableInit::Expr(exp) => {
                            encoder.table_with_init(ty, &exp.try_into().unwrap());
                        }
                    }
                }
                module.section(&encoder);
            }
            wasmparser::Payload::MemorySection(section) => {
                let mut encoder = wasm_encoder::MemorySection::new();
                for memory in section {
                    encoder.memory(memory.unwrap().into());
                }
                module.section(&encoder);
            }
            wasmparser::Payload::TagSection(section) => {
                let mut encoder = wasm_encoder::TagSection::new();
                for tag in section {
                    encoder.tag(tag.unwrap().into());
                }
                module.section(&encoder);
            }
            wasmparser::Payload::GlobalSection(section) => {
                let mut encoder = wasm_encoder::GlobalSection::new();
                for global in section {
                    let global = global.unwrap();
                    encoder.global(
                        global.ty.try_into().unwrap(),
                        &global.init_expr.try_into().unwrap(),
                    );
                }
                module.section(&encoder);
            }
            wasmparser::Payload::ExportSection(section) => {
                let mut encoder = wasm_encoder::ExportSection::new();
                for export in section {
                    let export = export.unwrap();
                    let export_name = exports_ident_map
                        .entry(export.name)
                        .or_insert_with(|| export_ident.next().unwrap());
                    encoder.export(export_name, export.kind.into(), export.index);
                }
                module.section(&encoder);
            }

            wasmparser::Payload::ElementSection(section) => {
                let mut encoder = wasm_encoder::ElementSection::new();
                for element in section {
                    let element = element.unwrap();
                    let (mut offset, mut functions, mut const_exprs) = (None, vec![], vec![]);
                    let segment = ElementSegment {
                        mode: map_element_kind(element.kind, &mut offset),
                        elements: map_element_items(
                            element.items,
                            &mut functions,
                            &mut const_exprs,
                        ),
                    };
                    encoder.segment(segment);
                }
                module.section(&encoder);
            }

            wasmparser::Payload::DataSection(section) => {
                let mut encoder = wasm_encoder::DataSection::new();
                for data in section {
                    let data = data.unwrap();
                    match data.kind {
                        wasmparser::DataKind::Passive => {
                            encoder.passive(data.data.iter().copied());
                        }
                        wasmparser::DataKind::Active { memory_index, offset_expr } => {
                            encoder.active(
                                memory_index,
                                &offset_expr.try_into().unwrap(),
                                data.data.iter().copied(),
                            );
                        }
                    }
                }
                module.section(&encoder);
            }

            wasmparser::Payload::CustomSection(section) => {
                module.section(&wasm_encoder::CustomSection {
                    name: section.name().into(),
                    data: section.data().into(),
                });
            }

            wasmparser::Payload::CodeSectionStart { count, .. } => {
                assert_eq!(code_section_remaining, 0);
                code_section_remaining = count;
                code_section_encoder = Some(wasm_encoder::CodeSection::new());
            }

            wasmparser::Payload::CodeSectionEntry(f) => {
                let mut reader = f.get_binary_reader();
                let bytes = reader.read_bytes(reader.bytes_remaining()).unwrap();

                let mut function = wasm_encoder::Function::new([]);

                pub struct Function {
                    bytes: Vec<u8>,
                }
                unsafe {
                    (*(&function as *const _ as *const Function as *mut Function))
                        .bytes
                        .clear();
                }
                assert_eq!(function.byte_len(), 0);

                function.raw(bytes.iter().copied());

                let encoder = code_section_encoder.as_mut().unwrap();
                encoder.function(&function);

                code_section_remaining -= 1;
                if code_section_remaining == 0 {
                    module.section(encoder);
                    code_section_encoder = None;
                }
            }

            wasmparser::Payload::Version { .. } | wasmparser::Payload::End(_) => {}

            e @ (wasmparser::Payload::StartSection { .. }
            | wasmparser::Payload::InstanceSection(_)
            | wasmparser::Payload::CoreTypeSection(_)
            | wasmparser::Payload::UnknownSection { .. }
            | wasmparser::Payload::DataCountSection { .. }
            | wasmparser::Payload::ModuleSection { .. }
            | wasmparser::Payload::ComponentSection { .. }
            | wasmparser::Payload::ComponentInstanceSection(_)
            | wasmparser::Payload::ComponentAliasSection(_)
            | wasmparser::Payload::ComponentTypeSection(_)
            | wasmparser::Payload::ComponentCanonicalSection(_)
            | wasmparser::Payload::ComponentStartSection { .. }
            | wasmparser::Payload::ComponentImportSection(_)
            | wasmparser::Payload::ComponentExportSection(_)) => todo!("{e:#?}"),
        }
    }

    assert!(code_section_encoder.is_none());

    let new_wasm = module.finish();
    let mut js_string = String::from_utf8(js.clone()).unwrap();

    // drawback: modifing javascript AST is better
    for (mod_before, (mod_after, fn_idents)) in imports_ident_map {
        js_string = js_string.replace(
            &format!("imports.{mod_before} = {{}};"),
            &format!("imports.{mod_after} = {{}};"),
        );

        for (fn_before, fn_after) in fn_idents {
            js_string = js_string.replace(
                &format!("imports.{mod_before}.{fn_before}"),
                &format!("imports.{mod_after}.{fn_after}"),
            );
        }
    }
    for (export_before, export_after) in exports_ident_map {
        js_string = js_string.replace(
            &format!("wasm.{export_before}"),
            &format!("wasm.{export_after}"),
        );
    }

    *js = js_string.into_bytes();
    *wasm = new_wasm;
}

struct MinifiedIdent {
    n: usize,
}
impl MinifiedIdent {
    fn new() -> Self {
        MinifiedIdent { n: 0 }
    }
}
impl Iterator for MinifiedIdent {
    type Item = String;

    // 123
    // 123 % 10 = 3, 123 /= 10 -> 12
    // 12 % 10 = 2, 12 /= 10 -> 1
    fn next(&mut self) -> Option<Self::Item> {
        let mut ret = String::new();
        let chars = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
        let mut n = self.n;
        loop {
            ret.insert(0, chars[n % chars.len()] as char);
            n /= chars.len();
            if n == 0 {
                break;
            }
        }
        self.n += 1;
        Some(ret)
    }
}
#[test]
fn minified_ident() {
    assert_eq!(
        MinifiedIdent::new().take(60).collect::<Vec<_>>().join(" "),
        "a b c d e f g h i j k l m n o p q r s t u v w x y z A B C D E F G H I J K L M N O P Q R S T U V W X Y Z ba bb bc bd be bf bg bh",
    );
}
