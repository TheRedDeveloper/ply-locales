#![cfg_attr(has_proc_macro_diagnostic, feature(proc_macro_diagnostic))]

extern crate proc_macro;

use proc_macro::TokenStream;
use std::collections::BTreeMap;
use std::path::Path;
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, Error, ItemMod, LitStr, Token};

mod ast_util;
mod codegen;
mod loader;
mod validator;

struct MacroArgs {
    path: LitStr,
    default: Option<LitStr>,
}

impl Parse for MacroArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let path: LitStr = input.parse()?;
        let mut default = None;

        if input.peek(Token![,]) {
            let _ = input.parse::<Token![,]>()?;
            if !input.is_empty() {
                let ident: syn::Ident = input.parse()?;
                if ident != "default" {
                    return Err(Error::new(
                        ident.span(),
                        format!("Unknown argument '{}'. Expected 'default'", ident),
                    ));
                }
                let _ = input.parse::<Token![=]>()?;
                let def_val: LitStr = input.parse()?;
                default = Some(def_val);
            }
        }

        Ok(MacroArgs { path, default })
    }
}

fn resolve_fluent_bundle_source(
    manifest_path: &Path,
    span: proc_macro2::Span,
) -> Result<proc_macro2::TokenStream, syn::Error> {
    let cargo_toml_path = manifest_path.join("Cargo.toml");
    let content = match std::fs::read_to_string(&cargo_toml_path) {
        Ok(s) => s,
        Err(_) => {
            return Ok(quote::quote!(
                use ::fluent_bundle;
            ))
        }
    };

    let toml_val: toml::Value = match toml::from_str(&content) {
        Ok(v) => v,
        Err(_) => {
            return Ok(quote::quote!(
                use ::fluent_bundle;
            ))
        }
    };

    if let Some(deps) = toml_val.get("dependencies").and_then(|d| d.as_table()) {
        for (dep_name, dep_val) in deps {
            if dep_name == "ply-engine" || dep_name == "ply_engine" {
                let has_locales = dep_val
                    .get("features")
                    .and_then(|f| f.as_array())
                    .is_some_and(|arr| arr.iter().any(|item| item.as_str() == Some("locales")));
                if has_locales {
                    return Ok(quote::quote!(
                        use ::ply_engine::fluent_bundle;
                    ));
                } else {
                    return Err(syn::Error::new(
                        span,
                        "ply-engine was found in dependencies, but the 'locales' feature is not enabled. Please enable it in Cargo.toml: ply-engine = { version = \"...\", features = [\"locales\"] }",
                    ));
                }
            }
        }
    }

    Ok(quote::quote!(
        use ::fluent_bundle;
    ))
}

#[derive(Clone, Debug)]
pub(crate) struct CustomFunction {
    pub(crate) rust_ident: syn::Ident,
    pub(crate) fluent_name: String,
    pub(crate) param_count: usize,
    pub(crate) param_types: Vec<syn::Type>,
    pub(crate) return_type: syn::ReturnType,
}

#[proc_macro_attribute]
pub fn ply_locales(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as MacroArgs);
    let item_mod = parse_macro_input!(item as ItemMod);

    let user_items = match item_mod.content {
        Some((_, items)) => items,
        None => Vec::new(),
    };

    let mut custom_functions = BTreeMap::new();
    for item in &user_items {
        if let syn::Item::Fn(func) = item {
            let ident = &func.sig.ident;
            let fluent_name = ident.to_string().to_uppercase();

            let mut param_types = Vec::new();
            for input in &func.sig.inputs {
                match input {
                    syn::FnArg::Receiver(recv) => {
                        return Error::new(
                            recv.self_token.span,
                            "Custom Fluent functions cannot have a 'self' parameter",
                        )
                        .to_compile_error()
                        .into();
                    }
                    syn::FnArg::Typed(pat_type) => {
                        param_types.push((*pat_type.ty).clone());
                    }
                }
            }

            if fluent_name == "NUMBER" || fluent_name == "DATETIME" || fluent_name == "VOID" {
                return Error::new(
                    ident.span(),
                    format!(
                        "Cannot define custom function '{}': '{}' is a built-in Fluent function",
                        ident, fluent_name
                    ),
                )
                .to_compile_error()
                .into();
            }

            if let Some(existing) = custom_functions.get(&fluent_name) as Option<&CustomFunction> {
                return Error::new(
                    ident.span(),
                    format!(
                        "Duplicate Fluent function name '{}' for Rust functions '{}' and '{}'",
                        fluent_name, existing.rust_ident, ident
                    ),
                )
                .to_compile_error()
                .into();
            }

            custom_functions.insert(
                fluent_name.clone(),
                CustomFunction {
                    rust_ident: ident.clone(),
                    fluent_name,
                    param_count: param_types.len(),
                    param_types,
                    return_type: func.sig.output.clone(),
                },
            );
        }
    }

    let manifest_dir = match std::env::var("CARGO_MANIFEST_DIR") {
        Ok(dir) => dir,
        Err(_) => {
            return Error::new(
                args.path.span(),
                "CARGO_MANIFEST_DIR environment variable is not set",
            )
            .to_compile_error()
            .into();
        }
    };
    let manifest_path = Path::new(&manifest_dir);

    let locales_data =
        match loader::load_locales(&args.path.value(), manifest_path, args.path.span()) {
            Ok(data) => data,
            Err(err) => return err.to_compile_error().into(),
        };

    #[cfg(has_tracked_path)]
    if let Some(s) = locales_data.locales_dir.to_str() {
        proc_macro::tracked_path::path(s);
    }

    let mut parsed_locales = BTreeMap::new();
    let mut combined_err: Option<Error> = None;

    for (loc_id, files) in &locales_data.locales {
        match ast_util::parse_locale(files, args.path.span()) {
            Ok(parsed) => {
                parsed_locales.insert(loc_id.clone(), parsed);
            }
            Err(err) => match &mut combined_err {
                Some(e) => e.combine(err),
                None => combined_err = Some(err),
            },
        }
    }

    if let Some(err) = combined_err {
        return err.to_compile_error().into();
    }

    let default_locale_id = match &args.default {
        Some(lit) => lit.value(),
        None => "en-US".to_string(),
    };

    let warnings = match validator::validate_locales(
        &parsed_locales,
        &default_locale_id,
        args.default.as_ref().map_or(args.path.span(), |d| d.span()),
        &custom_functions,
    ) {
        Ok(w) => w,
        Err(err) => return err.to_compile_error().into(),
    };

    #[cfg(has_proc_macro_diagnostic)]
    {
        for w in &warnings {
            let proc_span = args.path.span().unwrap();
            proc_macro::Diagnostic::spanned(proc_span, proc_macro::Level::Warning, w).emit();
        }
    }

    let fluent_bundle_import = match resolve_fluent_bundle_source(manifest_path, args.path.span()) {
        Ok(import) => import,
        Err(err) => return err.to_compile_error().into(),
    };

    let expanded = codegen::generate_module(
        &item_mod.vis,
        &item_mod.ident,
        &item_mod.attrs,
        manifest_path,
        &locales_data,
        &parsed_locales,
        &default_locale_id,
        &warnings,
        &user_items,
        &custom_functions,
        &fluent_bundle_import,
    );

    TokenStream::from(expanded)
}
