use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement};
use fluent_syntax::parser::parse;
use proc_macro2::Span;
use std::collections::BTreeMap;
use std::path::PathBuf;
use syn::Error;

use crate::loader::LocaleFile;

#[derive(Clone, Debug)]
pub struct ParsedMessage {
    pub id: String,
    pub rust_ident: syn::Ident,
    pub variables: Vec<String>,
    pub raw_definition: String,
    pub file_path: PathBuf,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub struct ParsedLocale {
    pub messages: BTreeMap<String, ParsedMessage>,
}

pub fn to_rust_ident(name: &str) -> syn::Ident {
    let snake = name.replace('-', "_");
    if let Ok(ident) = syn::parse_str::<syn::Ident>(&snake) {
        ident
    } else if let Ok(ident) = syn::parse_str::<syn::Ident>(&format!("r#{}", snake)) {
        ident
    } else {
        quote::format_ident!("_{}", snake)
    }
}

fn offset_to_line_col(text: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, c) in text.char_indices() {
        if i >= offset {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn extract_raw_definition(content: &str, msg_id: &str) -> (String, usize) {
    let lines = content.lines().enumerate();
    let mut def_lines = Vec::new();
    let mut start_line = 1;
    let mut found = false;

    for (idx, line) in lines {
        if !found {
            if (line.starts_with(msg_id) && line[msg_id.len()..].trim_start().starts_with('='))
                || (line.starts_with(&format!("{msg_id} ")) && line.contains('='))
            {
                found = true;
                start_line = idx + 1;
                def_lines.push(line);
            }
        } else if line.starts_with(' ') || line.starts_with('\t') {
            def_lines.push(line);
        } else {
            break;
        }
    }

    if found {
        (def_lines.join("\n"), start_line)
    } else {
        (format!("{} = ...", msg_id), 1)
    }
}

pub fn parse_locale(files: &[LocaleFile], span: Span) -> Result<ParsedLocale, Error> {
    let mut combined_err: Option<Error> = None;
    let mut messages: BTreeMap<String, ParsedMessage> = BTreeMap::new();

    for file in files {
        match parse(file.content.as_str()) {
            Ok(resource) => {
                for item in resource.body {
                    if let Entry::Message(msg) = item {
                        let msg_id = msg.id.name.to_string();
                        let rust_ident = to_rust_ident(&msg_id);
                        let mut variables = Vec::new();

                        if let Some(ref val) = msg.value {
                            collect_pattern_variables(val, &mut variables);
                        }
                        for attr in &msg.attributes {
                            collect_pattern_variables(&attr.value, &mut variables);
                        }

                        let (raw_definition, line) = extract_raw_definition(&file.content, &msg_id);

                        messages.insert(
                            msg_id.clone(),
                            ParsedMessage {
                                id: msg_id,
                                rust_ident,
                                variables,
                                raw_definition,
                                file_path: file.path.clone(),
                                line,
                            },
                        );
                    }
                }
            }
            Err((resource, parser_errors)) => {
                for item in resource.body {
                    if let Entry::Message(msg) = item {
                        let msg_id = msg.id.name.to_string();
                        let rust_ident = to_rust_ident(&msg_id);
                        let mut variables = Vec::new();
                        if let Some(ref val) = msg.value {
                            collect_pattern_variables(val, &mut variables);
                        }
                        for attr in &msg.attributes {
                            collect_pattern_variables(&attr.value, &mut variables);
                        }
                        let (raw_definition, line) = extract_raw_definition(&file.content, &msg_id);
                        messages.insert(
                            msg_id.clone(),
                            ParsedMessage {
                                id: msg_id,
                                rust_ident,
                                variables,
                                raw_definition,
                                file_path: file.path.clone(),
                                line,
                            },
                        );
                    }
                }

                for err in parser_errors {
                    let (line, col) = offset_to_line_col(&file.content, err.pos.start);
                    let error = Error::new(
                        span,
                        format!(
                            "Syntax error in Fluent file '{}:{}:{}': {:?}",
                            file.path.display(),
                            line,
                            col,
                            err.kind
                        ),
                    );
                    match &mut combined_err {
                        Some(e) => e.combine(error),
                        None => combined_err = Some(error),
                    }
                }
            }
        }
    }

    if let Some(err) = combined_err {
        return Err(err);
    }

    Ok(ParsedLocale { messages })
}

fn collect_pattern_variables(pattern: &Pattern<&str>, vars: &mut Vec<String>) {
    for element in &pattern.elements {
        match element {
            PatternElement::Placeable { expression } => {
                collect_expression_variables(expression, vars);
            }
            PatternElement::TextElement { .. } => {}
        }
    }
}

fn collect_expression_variables(expr: &Expression<&str>, vars: &mut Vec<String>) {
    match expr {
        Expression::Inline(inline) => {
            collect_inline_variables(inline, vars);
        }
        Expression::Select { selector, variants } => {
            collect_inline_variables(selector, vars);
            for variant in variants {
                collect_pattern_variables(&variant.value, vars);
            }
        }
    }
}

fn collect_inline_variables(inline: &InlineExpression<&str>, vars: &mut Vec<String>) {
    match inline {
        InlineExpression::VariableReference { id } => {
            let name = id.name.to_string();
            if !vars.contains(&name) {
                vars.push(name);
            }
        }
        InlineExpression::FunctionReference { arguments, .. } => {
            for arg in &arguments.positional {
                collect_inline_variables(arg, vars);
            }
            for named in &arguments.named {
                collect_inline_variables(&named.value, vars);
            }
        }
        InlineExpression::TermReference { arguments, .. } => {
            if let Some(args) = arguments {
                for arg in &args.positional {
                    collect_inline_variables(arg, vars);
                }
                for named in &args.named {
                    collect_inline_variables(&named.value, vars);
                }
            }
        }
        InlineExpression::Placeable { expression } => {
            collect_expression_variables(expression, vars);
        }
        InlineExpression::StringLiteral { .. }
        | InlineExpression::NumberLiteral { .. }
        | InlineExpression::MessageReference { .. } => {}
    }
}
