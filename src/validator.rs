use proc_macro2::Span;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use syn::Error;

use crate::ast_util::ParsedLocale;

fn format_snippet_and_notes(
    file_path: &Path,
    start_line: usize,
    raw_definition: &str,
    expected_list: &[String],
    found_list: &[String],
) -> String {
    let line_count = raw_definition.lines().count();
    let max_line = start_line + line_count.saturating_sub(1);
    let w = max_line.to_string().len().max(1);
    let arrow_pad = " ".repeat(w);
    let pad = " ".repeat(w + 1);

    let mut out = format!(
        "{}--> {}:{}\n{}|\n",
        arrow_pad,
        file_path.display(),
        start_line,
        pad
    );
    for (i, line) in raw_definition.lines().enumerate() {
        let line_num = start_line + i;
        out.push_str(&format!("{:>w$} | {}\n", line_num, line, w = w));
    }
    out.push_str(&format!("{}|\n", pad));
    out.push_str(&format!(
        "{}= expected: [{}]\n",
        pad,
        expected_list.join(", ")
    ));
    out.push_str(&format!("{}= found:    [{}]", pad, found_list.join(", ")));
    out
}

pub fn validate_locales(
    locales: &BTreeMap<String, ParsedLocale>,
    default_locale_id: &str,
    span: Span,
) -> Result<Vec<String>, Error> {
    let default_locale = match locales.get(default_locale_id) {
        Some(l) => l,
        None => {
            let available = locales.keys().cloned().collect::<Vec<_>>().join(", ");
            return Err(Error::new(
                span,
                format!(
                    "Default locale '{}' not found in locales directory. Available locales: [{}]",
                    default_locale_id, available
                ),
            ));
        }
    };

    let mut combined_err: Option<Error> = None;
    let mut warnings: Vec<String> = Vec::new();

    for (locale_id, locale) in locales {
        if locale_id == default_locale_id {
            continue;
        }

        for (key, default_msg) in &default_locale.messages {
            match locale.messages.get(key) {
                None => {
                    warnings.push(format!(
                        "Locale '{}' is missing translation for key '{}' (defined in default locale '{}'). At runtime this key will fall back to '{}'.",
                        locale_id, key, default_locale_id, default_locale_id
                    ));
                }
                Some(other_msg) => {
                    let def_vars: BTreeSet<&String> = default_msg.variables.iter().collect();
                    let other_vars: BTreeSet<&String> = other_msg.variables.iter().collect();

                    if def_vars != other_vars {
                        let expected_list: Vec<String> = default_msg
                            .variables
                            .iter()
                            .map(|s| format!("${s}"))
                            .collect();
                        let found_list: Vec<String> = other_msg
                            .variables
                            .iter()
                            .map(|s| format!("${s}"))
                            .collect();

                        let details = format_snippet_and_notes(
                            &other_msg.file_path,
                            other_msg.line,
                            &other_msg.raw_definition,
                            &expected_list,
                            &found_list,
                        );

                        let err_msg = format!(
                            "Mismatched Fluent variables in message '{}' for locale '{}'\n{}",
                            key, locale_id, details
                        );

                        let err = Error::new(span, err_msg);
                        match &mut combined_err {
                            Some(e) => e.combine(err),
                            None => combined_err = Some(err),
                        }
                    }
                }
            }
        }

        for (key, extra_msg) in &locale.messages {
            if !default_locale.messages.contains_key(key) {
                warnings.push(format!(
                    "Locale '{}' contains key '{}' at {}:{} which does not exist in default locale '{}'. This key will be ignored.",
                    locale_id,
                    key,
                    extra_msg.file_path.display(),
                    extra_msg.line,
                    default_locale_id
                ));
            }
        }
    }

    if let Some(err) = combined_err {
        return Err(err);
    }

    Ok(warnings)
}
