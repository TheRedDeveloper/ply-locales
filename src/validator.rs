use proc_macro2::Span;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use syn::Error;

use crate::ast_util::{offset_to_line_col, ParsedLocale, ReferenceSpan};
use crate::CustomFunction;

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
    out.push_str(&format!("{}= found:    [{}]\n", pad, found_list.join(", ")));
    out
}

fn format_span_error(
    file_path: &Path,
    content: &str,
    start: usize,
    end: usize,
    header: &str,
    label: &str,
) -> String {
    let (line, col) = offset_to_line_col(content, start);
    let line_content = content.lines().nth(line.saturating_sub(1)).unwrap_or("");
    let ref_len = end.saturating_sub(start).max(1);
    let end_col = col + ref_len.saturating_sub(1);

    let w = line.to_string().len().max(1);
    let arrow_pad = " ".repeat(w);
    let pad = " ".repeat(w + 1);
    let prefix: String = line_content
        .chars()
        .take(col.saturating_sub(1))
        .map(|c| if c == '\t' { '\t' } else { ' ' })
        .collect();
    let carets = "^".repeat(ref_len);

    let loc_str = if ref_len > 1 {
        format!("{}:{}:{}-{}", file_path.display(), line, col, end_col)
    } else {
        format!("{}:{}:{}", file_path.display(), line, col)
    };

    format!(
        "{header}\n{}--> {loc_str}\n{}|\n{:>w$} | {}\n{}| {prefix}{carets} {label}\n",
        arrow_pad,
        pad,
        line,
        line_content,
        pad,
        w = w
    )
}

const NUMBER_OPTIONS: &[&str] = &[
    "currency",
    "currencyDisplay",
    "useGrouping",
    "minimumIntegerDigits",
    "minimumFractionDigits",
    "maximumFractionDigits",
    "minimumSignificantDigits",
    "maximumSignificantDigits",
    "style",
];

const DATETIME_OPTIONS: &[&str] = &[
    "dateStyle",
    "timeStyle",
    "fractionalSecondDigits",
    "dayPeriod",
    "hour12",
    "weekday",
    "era",
    "year",
    "month",
    "day",
    "hour",
    "minute",
    "second",
    "timeZoneName",
];

#[derive(Clone)]
struct CycleEntryInfo {
    file_path: PathBuf,
    start_line: usize,
    raw_definition: String,
}

fn format_cycle_error(locale_id: &str, cycle: &[String], entries: &[CycleEntryInfo]) -> String {
    let header = format!("Circular dependency detected in locale '{locale_id}'");
    if entries.is_empty() {
        return format!("{header}\n= cycle: {}\n", cycle.join(" -> "));
    }

    let file_path = &entries[0].file_path;

    let max_line = entries
        .iter()
        .map(|e| e.start_line + e.raw_definition.lines().count().saturating_sub(1))
        .max()
        .unwrap_or(1);
    let w = max_line.to_string().len().max(1);
    let arrow_pad = " ".repeat(w);
    let pad = " ".repeat(w + 1);

    let mut out = format!(
        "{header}\n{}--> {}\n{}|\n",
        arrow_pad,
        file_path.display(),
        pad
    );

    let mut sorted_entries = entries.to_vec();
    sorted_entries.sort_by_key(|e| e.start_line);

    let mut prev_end_line: Option<usize> = None;
    for entry in &sorted_entries {
        if let Some(prev_end) = prev_end_line {
            if entry.start_line > prev_end + 1 {
                out.push_str(&format!("{}...\n", pad));
            }
        }

        for (i, line) in entry.raw_definition.lines().enumerate() {
            let line_num = entry.start_line + i;
            out.push_str(&format!("{:>w$} | {}\n", line_num, line, w = w));
        }

        prev_end_line =
            Some(entry.start_line + entry.raw_definition.lines().count().saturating_sub(1));
    }

    out.push_str(&format!("{}|\n", pad));
    out.push_str(&format!("{}= cycle: {}\n", pad, cycle.join(" -> ")));
    out
}

fn detect_cycles(
    nodes: &BTreeSet<String>,
    edges: &BTreeMap<String, Vec<String>>,
) -> Vec<Vec<String>> {
    let mut detected_canonical_cycles = BTreeSet::new();
    let mut result = Vec::new();

    fn dfs(
        current: &str,
        path: &mut Vec<String>,
        on_path: &mut BTreeSet<String>,
        edges: &BTreeMap<String, Vec<String>>,
        detected: &mut BTreeSet<Vec<String>>,
        result: &mut Vec<Vec<String>>,
    ) {
        path.push(current.to_string());
        on_path.insert(current.to_string());

        if let Some(neighbors) = edges.get(current) {
            for neighbor in neighbors {
                if on_path.contains(neighbor) {
                    if let Some(cycle_start_idx) = path.iter().position(|node| node == neighbor) {
                        let cycle = path[cycle_start_idx..].to_vec();
                        if !cycle.is_empty() {
                            let min_idx = cycle
                                .iter()
                                .enumerate()
                                .min_by_key(|(_, v)| *v)
                                .map(|(idx, _)| idx)
                                .unwrap();
                            let mut canonical = cycle[min_idx..].to_vec();
                            canonical.extend_from_slice(&cycle[..min_idx]);
                            if detected.insert(canonical.clone()) {
                                let mut display_cycle = canonical.clone();
                                display_cycle.push(canonical[0].clone());
                                result.push(display_cycle);
                            }
                        }
                    }
                } else {
                    dfs(neighbor, path, on_path, edges, detected, result);
                }
            }
        }

        on_path.remove(current);
        path.pop();
    }

    for start_node in nodes {
        let mut path = Vec::new();
        let mut on_path = BTreeSet::new();
        dfs(
            start_node,
            &mut path,
            &mut on_path,
            edges,
            &mut detected_canonical_cycles,
            &mut result,
        );
    }

    result
}

#[allow(clippy::too_many_arguments)]
fn validate_calls(
    entry_kind: &str,
    entry_id: &str,
    file_path: &Path,
    content: &str,
    term_references: &[ReferenceSpan],
    function_references: &[ReferenceSpan],
    locale_id: &str,
    locale: &ParsedLocale,
    span: Span,
    custom_functions: &BTreeMap<String, CustomFunction>,
    combined_err: &mut Option<Error>,
) {
    // 1. Validate function calls
    for func_ref in function_references {
        if let Some(custom_fn) = custom_functions.get(&func_ref.name) {
            // Check positional arguments count for custom function
            if func_ref.positional_count != custom_fn.param_count {
                let err_msg = format_span_error(
                    file_path,
                    content,
                    func_ref.start,
                    func_ref.end,
                    &format!(
                        "Function '{}' requires exactly {} positional argument{}, found {} in {entry_kind} '{entry_id}' for locale '{locale_id}'",
                        func_ref.name,
                        custom_fn.param_count,
                        if custom_fn.param_count == 1 { "" } else { "s" },
                        func_ref.positional_count
                    ),
                    &format!(
                        "Expected {} argument{}, found {}",
                        custom_fn.param_count,
                        if custom_fn.param_count == 1 { "" } else { "s" },
                        func_ref.positional_count
                    ),
                );
                let err = Error::new(span, err_msg);
                match combined_err {
                    Some(e) => e.combine(err),
                    None => *combined_err = Some(err),
                }
            }

            // Custom functions do not accept named options
            for named in &func_ref.named_args {
                let err_msg = format_span_error(
                    file_path,
                    content,
                    named.start,
                    named.end,
                    &format!(
                        "Unknown argument '{}' in call to function '{}' in {entry_kind} '{entry_id}' for locale '{locale_id}'",
                        named.name, func_ref.name
                    ),
                    &format!(
                        "Function '{}' does not accept option '{}'",
                        func_ref.name, named.name
                    ),
                );
                let err = Error::new(span, err_msg);
                match combined_err {
                    Some(e) => e.combine(err),
                    None => *combined_err = Some(err),
                }
            }
            continue;
        }

        if func_ref.name != "NUMBER" && func_ref.name != "DATETIME" {
            let err_msg = format_span_error(
                file_path,
                content,
                func_ref.start,
                func_ref.end,
                &format!("Unknown function in {entry_kind} '{entry_id}' for locale '{locale_id}'"),
                &format!("Function '{}' is not defined", func_ref.name),
            );
            let err = Error::new(span, err_msg);
            match combined_err {
                Some(e) => e.combine(err),
                None => *combined_err = Some(err),
            }
            continue;
        }

        // Check positional arguments count: builtins NUMBER and DATETIME require exactly 1 positional argument
        if func_ref.positional_count != 1 {
            let err_msg = format_span_error(
                file_path,
                content,
                func_ref.start,
                func_ref.end,
                &format!(
                    "Function '{}' requires exactly 1 positional argument, found {} in {entry_kind} '{entry_id}' for locale '{locale_id}'",
                    func_ref.name, func_ref.positional_count
                ),
                &format!("Expected 1 argument, found {}", func_ref.positional_count),
            );
            let err = Error::new(span, err_msg);
            match combined_err {
                Some(e) => e.combine(err),
                None => *combined_err = Some(err),
            }
        }

        // Check named options
        let allowed_opts = if func_ref.name == "NUMBER" {
            NUMBER_OPTIONS
        } else {
            DATETIME_OPTIONS
        };

        for named in &func_ref.named_args {
            if !allowed_opts.contains(&named.name.as_str()) {
                let err_msg = format_span_error(
                    file_path,
                    content,
                    named.start,
                    named.end,
                    &format!(
                        "Unknown argument '{}' in call to function '{}' in {entry_kind} '{entry_id}' for locale '{locale_id}'",
                        named.name, func_ref.name
                    ),
                    &format!(
                        "Function '{}' does not accept option '{}'",
                        func_ref.name, named.name
                    ),
                );
                let err = Error::new(span, err_msg);
                match combined_err {
                    Some(e) => e.combine(err),
                    None => *combined_err = Some(err),
                }
            }
        }
    }

    // 2. Validate term calls
    for term_ref in term_references {
        let target_term = match locale.terms.get(&term_ref.name) {
            Some(t) => t,
            None => {
                let err_msg = format_span_error(
                    file_path,
                    content,
                    term_ref.start,
                    term_ref.end,
                    &format!(
                        "Missing dependency in {entry_kind} '{entry_id}' for locale '{locale_id}'"
                    ),
                    &format!(
                        "Term '{}' is not defined in locale '{locale_id}'",
                        term_ref.name
                    ),
                );
                let err = Error::new(span, err_msg);
                match combined_err {
                    Some(e) => e.combine(err),
                    None => *combined_err = Some(err),
                }
                continue;
            }
        };

        // Terms cannot accept positional arguments
        if term_ref.positional_count > 0 {
            let err_msg = format_span_error(
                file_path,
                content,
                term_ref.start,
                term_ref.end,
                &format!(
                    "Terms only accept named arguments in call to '{}' in {entry_kind} '{entry_id}' for locale '{locale_id}'",
                    term_ref.name
                ),
                "Terms do not accept positional arguments",
            );
            let err = Error::new(span, err_msg);
            match combined_err {
                Some(e) => e.combine(err),
                None => *combined_err = Some(err),
            }
        }

        // Required arguments: all variables used in target_term must be passed
        let provided_names: BTreeSet<&str> = term_ref
            .named_args
            .iter()
            .map(|a| a.name.as_str())
            .collect();

        for req_var in &target_term.variables {
            if !provided_names.contains(req_var.as_str()) {
                let err_msg = format_span_error(
                    file_path,
                    content,
                    term_ref.start,
                    term_ref.end,
                    &format!(
                        "Missing argument '{req_var}' in call to term '{}' in {entry_kind} '{entry_id}' for locale '{locale_id}'",
                        term_ref.name
                    ),
                    &format!("Missing argument '{req_var}'"),
                );
                let err = Error::new(span, err_msg);
                match combined_err {
                    Some(e) => e.combine(err),
                    None => *combined_err = Some(err),
                }
            }
        }

        // Unknown arguments: arguments passed that are not in target_term.variables
        for named in &term_ref.named_args {
            if !target_term.variables.contains(&named.name) {
                let err_msg = format_span_error(
                    file_path,
                    content,
                    named.start,
                    named.end,
                    &format!(
                        "Unknown argument '{}' in call to term '{}' in {entry_kind} '{entry_id}' for locale '{locale_id}'",
                        named.name, term_ref.name
                    ),
                    &format!(
                        "Term '{}' does not accept argument '{}'",
                        term_ref.name, named.name
                    ),
                );
                let err = Error::new(span, err_msg);
                match combined_err {
                    Some(e) => e.combine(err),
                    None => *combined_err = Some(err),
                }
            }
        }
    }
}

pub fn validate_locales(
    locales: &BTreeMap<String, ParsedLocale>,
    default_locale_id: &str,
    span: Span,
    custom_functions: &BTreeMap<String, CustomFunction>,
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

    // 1. Validate calls and dependencies across all locales
    for (locale_id, locale) in locales {
        // Validate messages
        for (msg_id, msg) in &locale.messages {
            let content = locale
                .file_contents
                .get(&msg.file_path)
                .map(|s| s.as_str())
                .unwrap_or("");

            validate_calls(
                "message",
                msg_id,
                &msg.file_path,
                content,
                &msg.term_references,
                &msg.function_references,
                locale_id,
                locale,
                span,
                custom_functions,
                &mut combined_err,
            );

            // Missing messages
            for msg_ref in &msg.message_references {
                if !locale.messages.contains_key(&msg_ref.name) {
                    let err_msg = format_span_error(
                        &msg.file_path,
                        content,
                        msg_ref.start,
                        msg_ref.end,
                        &format!(
                            "Missing dependency in message '{msg_id}' for locale '{locale_id}'"
                        ),
                        &format!(
                            "Message '{}' is not defined in locale '{locale_id}'",
                            msg_ref.name
                        ),
                    );
                    let err = Error::new(span, err_msg);
                    match &mut combined_err {
                        Some(e) => e.combine(err),
                        None => combined_err = Some(err),
                    }
                }
            }
        }

        // Validate terms
        for (term_id, term) in &locale.terms {
            let content = locale
                .file_contents
                .get(&term.file_path)
                .map(|s| s.as_str())
                .unwrap_or("");

            validate_calls(
                "term",
                term_id,
                &term.file_path,
                content,
                &term.term_references,
                &term.function_references,
                locale_id,
                locale,
                span,
                custom_functions,
                &mut combined_err,
            );

            // Missing messages
            for msg_ref in &term.message_references {
                if !locale.messages.contains_key(&msg_ref.name) {
                    let err_msg = format_span_error(
                        &term.file_path,
                        content,
                        msg_ref.start,
                        msg_ref.end,
                        &format!("Missing dependency in term '{term_id}' for locale '{locale_id}'"),
                        &format!(
                            "Message '{}' is not defined in locale '{locale_id}'",
                            msg_ref.name
                        ),
                    );
                    let err = Error::new(span, err_msg);
                    match &mut combined_err {
                        Some(e) => e.combine(err),
                        None => combined_err = Some(err),
                    }
                }
            }
        }

        // 2. Circular dependency detection
        let mut nodes = BTreeSet::new();
        let mut edges: BTreeMap<String, Vec<String>> = BTreeMap::new();

        for (msg_id, msg) in &locale.messages {
            nodes.insert(msg_id.clone());
            let mut neighbors = Vec::new();
            for r in &msg.term_references {
                if locale.terms.contains_key(&r.name) {
                    neighbors.push(r.name.clone());
                }
            }
            for r in &msg.message_references {
                if locale.messages.contains_key(&r.name) {
                    neighbors.push(r.name.clone());
                }
            }
            edges.insert(msg_id.clone(), neighbors);
        }

        for (term_id, term) in &locale.terms {
            nodes.insert(term_id.clone());
            let mut neighbors = Vec::new();
            for r in &term.term_references {
                if locale.terms.contains_key(&r.name) {
                    neighbors.push(r.name.clone());
                }
            }
            for r in &term.message_references {
                if locale.messages.contains_key(&r.name) {
                    neighbors.push(r.name.clone());
                }
            }
            edges.insert(term_id.clone(), neighbors);
        }

        let cycles = detect_cycles(&nodes, &edges);
        for cycle in cycles {
            let mut cycle_entries = Vec::new();
            let mut seen = BTreeSet::new();
            for node in &cycle {
                if seen.insert(node.clone()) {
                    if let Some(term) = locale.terms.get(node) {
                        cycle_entries.push(CycleEntryInfo {
                            file_path: term.file_path.clone(),
                            start_line: term.line,
                            raw_definition: term.raw_definition.clone(),
                        });
                    } else if let Some(msg) = locale.messages.get(node) {
                        cycle_entries.push(CycleEntryInfo {
                            file_path: msg.file_path.clone(),
                            start_line: msg.line,
                            raw_definition: msg.raw_definition.clone(),
                        });
                    }
                }
            }

            let err_msg = format_cycle_error(locale_id, &cycle, &cycle_entries);
            let err = Error::new(span, err_msg);
            match &mut combined_err {
                Some(e) => e.combine(err),
                None => combined_err = Some(err),
            }
        }
    }

    // 3. Mismatched variables and missing translation warnings
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
