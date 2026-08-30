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

#[proc_macro_attribute]
pub fn ply_locales(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as MacroArgs);
    let item_mod = parse_macro_input!(item as ItemMod);

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

    let expanded = codegen::generate_module(
        &item_mod.vis,
        &item_mod.ident,
        &item_mod.attrs,
        manifest_path,
        &locales_data,
        &parsed_locales,
        &default_locale_id,
        &warnings,
    );

    TokenStream::from(expanded)
}
