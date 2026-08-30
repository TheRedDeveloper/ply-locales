use proc_macro2::TokenStream;
use quote::quote;
use std::collections::BTreeMap;
use syn::Visibility;

use crate::ast_util::{to_rust_ident, ParsedLocale};
use crate::loader::LocalesData;

#[allow(clippy::too_many_arguments)]
pub fn generate_module(
    vis: &Visibility,
    mod_ident: &syn::Ident,
    mod_attrs: &[syn::Attribute],
    manifest_path: &std::path::Path,
    locales_data: &LocalesData,
    parsed_locales: &BTreeMap<String, ParsedLocale>,
    default_locale_id: &str,
    _warnings: &[String],
) -> TokenStream {
    let default_locale = &parsed_locales[default_locale_id];

    #[cfg(not(has_proc_macro_diagnostic))]
    let warning_tokens: Vec<_> = _warnings
        .iter()
        .map(|w| {
            quote! {
                #[allow(non_upper_case_globals, dead_code)]
                const _: () = {
                    #[deprecated(note = #w)]
                    const WARNING: () = ();
                    let _ = WARNING;
                };
            }
        })
        .collect();

    #[cfg(has_proc_macro_diagnostic)]
    let warning_tokens: Vec<proc_macro2::TokenStream> = Vec::new();

    let raw_locale_entries = locales_data.locales.iter().map(|(id, files)| {
        let rel_paths = files.iter().map(|f| f.path.to_string_lossy().to_string());
        quote! {
            (#id, &[
                #(
                    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", #rel_paths))
                ),*
            ])
        }
    });

    let available_locale_ids: Vec<&str> = locales_data.locales.keys().map(|s| s.as_str()).collect();

    let message_functions = default_locale.messages.values().map(|msg| {
        let fn_name = &msg.rust_ident;
        let msg_id = &msg.id;
        let file_str = msg.file_path.display();
        let line_num = msg.line;

        let abs_path = manifest_path.join(&msg.file_path);
        let abs_str = abs_path.to_string_lossy().replace('\\', "/");
        let file_uri = if abs_str.starts_with('/') {
            format!("file://{}#L{}", abs_str, line_num)
        } else {
            format!("file:///{}#L{}", abs_str, line_num)
        };

        let doc_comment = format!(
            "```fluent\n{}\n```\n[{}:{}]({})",
            msg.raw_definition, file_str, line_num, file_uri
        );

        if msg.variables.is_empty() {
            quote! {
                #[doc = #doc_comment]
                pub fn #fn_name() -> String {
                    format_message(#msg_id, None)
                }
            }
        } else {
            let param_idents: Vec<syn::Ident> =
                msg.variables.iter().map(|v| to_rust_ident(v)).collect();
            let var_names = &msg.variables;

            quote! {
                #[doc = #doc_comment]
                pub fn #fn_name<'a>(
                    #( #param_idents: impl Into<::fluent_bundle::FluentValue<'a>> ),*
                ) -> String {
                    let mut args = ::fluent_bundle::FluentArgs::new();
                    #(
                        args.set(#var_names, #param_idents.into());
                    )*
                    format_message(#msg_id, Some(&args))
                }
            }
        }
    });

    quote! {
        #(#mod_attrs)*
        #vis mod #mod_ident {
            #( #warning_tokens )*

            /// A list of all available locale identifiers.
            pub const AVAILABLE_LOCALES: &[&'static str] = &[
                #( #available_locale_ids ),*
            ];

            static RAW_LOCALES: &[(&'static str, &[&'static str])] = &[
                #( #raw_locale_entries ),*
            ];

            static CURRENT_LOCALE: ::std::sync::RwLock<Option<String>> =
                ::std::sync::RwLock::new(None);

            static BUNDLES: ::std::sync::RwLock<
                ::std::collections::BTreeMap<
                    &'static str,
                    ::fluent_bundle::concurrent::FluentBundle<::fluent_bundle::FluentResource>,
                >,
            > = ::std::sync::RwLock::new(::std::collections::BTreeMap::new());

            /// Sets the active locale for formatting messages.
            ///
            /// Returns `false` if `locale` is not in `AVAILABLE_LOCALES` (can't be set)
            pub fn set_locale(locale: &str) -> bool {
                if AVAILABLE_LOCALES.contains(&locale) {
                    let mut lock = CURRENT_LOCALE.write().unwrap();
                    *lock = Some(locale.to_string());
                    true
                } else {
                    false
                }
            }

            /// Returns the currently active locale identifier.
            pub fn current_locale() -> String {
                let lock = CURRENT_LOCALE.read().unwrap();
                match &*lock {
                    Some(loc) => loc.clone(),
                    None => #default_locale_id.to_string(),
                }
            }

            fn ensure_bundle(locale: &'static str) {
                if BUNDLES.read().unwrap().contains_key(locale) {
                    return;
                }
                let mut bundles = BUNDLES.write().unwrap();
                if let ::std::collections::btree_map::Entry::Vacant(entry) = bundles.entry(locale) {
                    if let Some((_, raw_files)) = RAW_LOCALES.iter().find(|(loc, _)| *loc == locale) {
                        let langid = locale.parse().unwrap();
                        let mut bundle = ::fluent_bundle::concurrent::FluentBundle::new_concurrent(vec![langid]);
                        bundle.set_use_isolating(false);
                        let _ = bundle.add_builtins();
                        for ftl in *raw_files {
                            if let Ok(res) = ::fluent_bundle::FluentResource::try_new(ftl.to_string()) {
                                let _ = bundle.add_resource(res);
                            }
                        }
                        entry.insert(bundle);
                    }
                }
            }

            fn format_message(msg_id: &str, args: Option<&::fluent_bundle::FluentArgs>) -> String {
                let cur = current_locale();
                let cur_static = AVAILABLE_LOCALES
                    .iter()
                    .copied()
                    .find(|&loc| loc == cur)
                    .unwrap_or(#default_locale_id);

                for &loc in &[cur_static, #default_locale_id] {
                    ensure_bundle(loc);
                    let bundles = BUNDLES.read().unwrap();
                    if let Some(bundle) = bundles.get(loc) {
                        if let Some(pattern) = bundle.get_message(msg_id).and_then(|m| m.value()) {
                            let mut errors = vec![];
                            return bundle.format_pattern(pattern, args, &mut errors).to_string();
                        }
                    }
                }

                format!("{msg_id}")
            }

            #( #message_functions )*
        }
    }
}
