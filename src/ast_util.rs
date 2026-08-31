use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement};
use fluent_syntax::parser::parse;
use proc_macro2::Span;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
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

fn friendly_error_message(
    kind: &fluent_syntax::parser::ErrorKind,
    content: &str,
    pos: usize,
) -> String {
    use fluent_syntax::parser::ErrorKind::*;
    match kind {
        ExpectedToken(c) => match c {
            '}' => "Expected }".to_string(),
            '{' => "Expected {".to_string(),
            '"' => "Expected closing quote '\"'".to_string(),
            '=' => "Expected = after identifier".to_string(),
            ':' => "Expected : after argument name".to_string(),
            ']' => "Expected ]".to_string(),
            '[' => "Expected [".to_string(),
            other => format!("Expected {}", other),
        },
        ExpectedCharRange { range } => {
            let preceding = content[..pos.min(content.len())].trim_end();
            if range.contains('\n') || range.contains('\r') {
                "Expected line break".to_string()
            } else if preceding.ends_with('$') {
                "Expected a variable name".to_string()
            } else if preceding.ends_with('-') {
                "Expected a term name".to_string()
            } else if range == "a-zA-Z" {
                "Expected an identifier".to_string()
            } else if range == "0-9" {
                "Expected a digit (0-9)".to_string()
            } else if range == "a-zA-Z0-9_-" {
                "Expected an identifier (letters, numbers, _, -)".to_string()
            } else if range == "0-9a-fA-F" {
                "Expected a hex digit (0-9, a-f, A-F)".to_string()
            } else {
                let clean: String = range.chars().flat_map(|c| c.escape_default()).collect();
                format!("Expected character in range '{clean}'")
            }
        }
        ExpectedMessageField { entry_id } => {
            format!("Expected a message field for '{entry_id}'")
        }
        ExpectedTermField { entry_id } => {
            format!("Expected a term field for '{entry_id}'")
        }
        ForbiddenCallee => "Functions cannot be called here".to_string(),
        MissingDefaultVariant => {
            "Select expression must have a default variant marked with '*' (e.g. *[other])"
                .to_string()
        }
        MissingValue => "Expected a value or attribute after '='".to_string(),
        MultipleDefaultVariants => {
            "Select expression can only have one default variant ('*')".to_string()
        }
        MessageReferenceAsSelector => {
            "Message references cannot be used as select selectors".to_string()
        }
        TermReferenceAsSelector => "Term references cannot be used as select selectors".to_string(),
        MessageAttributeAsSelector => {
            "Message attributes cannot be used as select selectors".to_string()
        }
        TermAttributeAsPlaceable => "Term attributes cannot be used as placeables".to_string(),
        UnterminatedStringLiteral => {
            "Unterminated string literal, missing closing quote '\"'".to_string()
        }
        PositionalArgumentFollowsNamed => {
            "Positional arguments must come before named arguments".to_string()
        }
        DuplicatedNamedArgument(name) => {
            format!("Argument '{name}' is specified more than once")
        }
        UnknownEscapeSequence(seq) => format!("Unknown escape sequence '\\{seq}'"),
        InvalidUnicodeEscapeSequence(seq) => {
            format!("Invalid Unicode escape sequence '\\{seq}'")
        }
        UnbalancedClosingBrace => {
            let mut depth = 0;
            let mut has_unclosed_select = false;
            for (i, c) in content[..pos.min(content.len())].char_indices().rev() {
                if c == '}' {
                    depth += 1;
                } else if c == '{' {
                    if depth > 0 {
                        depth -= 1;
                    } else {
                        if content[i..pos.min(content.len())].contains("->") {
                            has_unclosed_select = true;
                        }
                        break;
                    }
                }
            }
            if has_unclosed_select {
                "Closing '}' for select expression must be on a new line".to_string()
            } else {
                "Unexpected closing '}', no matching '{' was opened".to_string()
            }
        }
        ExpectedInlineExpression => {
            "Expected an expression (such as a variable $name or literal)".to_string()
        }
        ExpectedSimpleExpressionAsSelector => {
            "Expected a variable or function call as select selector".to_string()
        }
        ExpectedLiteral => "Expected a string or number literal".to_string(),
    }
}

fn format_syntax_error(
    file_path: &Path,
    content: &str,
    err: &fluent_syntax::parser::ParserError,
) -> String {
    use fluent_syntax::parser::ErrorKind::*;

    let (line, col) = offset_to_line_col(content, err.pos.start);
    let line_content = content.lines().nth(line.saturating_sub(1)).unwrap_or("");

    // Check for empty placeable `{}` or `{ }`
    if let ExpectedInlineExpression = &err.kind {
        let before = &content[..err.pos.start.min(content.len())];
        let after = if err.pos.start < content.len() {
            &content[err.pos.start..]
        } else {
            ""
        };

        let before_line_start = before.rfind('\n').map_or(0, |i| i + 1);
        let before_on_line = &before[before_line_start..];

        let after_line_end = after.find('\n').unwrap_or(after.len());
        let after_on_line = &after[..after_line_end];

        if let Some(open_idx) = before_on_line.rfind('{') {
            let between_open = &before_on_line[open_idx + 1..];
            if between_open.chars().all(|c| c.is_whitespace()) {
                if let Some(close_idx) = after_on_line.find('}') {
                    let between_close = &after_on_line[..close_idx];
                    if between_close.chars().all(|c| c.is_whitespace()) {
                        let open_col = open_idx + 1;
                        let close_col = before_on_line.len() + close_idx + 1;
                        let carets_len = close_col.saturating_sub(open_col) + 1;
                        let w = line.to_string().len().max(1);
                        let arrow_pad = " ".repeat(w);
                        let pad = " ".repeat(w + 1);
                        let prefix: String = line_content
                            .chars()
                            .take(open_col.saturating_sub(1))
                            .map(|c| if c == '\t' { '\t' } else { ' ' })
                            .collect();
                        let carets = "^".repeat(carets_len);

                        return format!(
                            "{}--> {}:{}:{}-{}\n{}|\n{:>w$} | {}\n{}| {prefix}{} Expression can't be empty\n",
                            arrow_pad,
                            file_path.display(),
                            line,
                            open_col,
                            close_col,
                            pad,
                            line,
                            line_content,
                            pad,
                            carets,
                            w = w
                        );
                    }
                }
            }
        }
    }

    // Check if MissingValue occurs after a variant like `[one]`
    if let MissingValue = &err.kind {
        let trimmed = content[..err.pos.start.min(content.len())].trim_end();
        if trimmed.ends_with(']') {
            if let Some(open_bracket) = trimmed.rfind('[') {
                let variant_name = &trimmed[open_bracket..];
                let before_bracket = &trimmed[..open_bracket];
                let (var_l, var_c) = offset_to_line_col(content, trimmed.len());
                let var_line_content = content.lines().nth(var_l.saturating_sub(1)).unwrap_or("");

                let mut select_ctx = None;
                if let Some(arrow_idx) = before_bracket.rfind("->") {
                    if let Some(open_brace_idx) = before_bracket[..arrow_idx].rfind('{') {
                        let (open_l, open_c) = offset_to_line_col(content, open_brace_idx);
                        let (arrow_l, arrow_c) = offset_to_line_col(content, arrow_idx);
                        if open_l < var_l && open_l == arrow_l {
                            select_ctx = Some((open_l, open_c, arrow_c + 1));
                        }
                    }
                }

                let max_l = var_l.max(select_ctx.map_or(var_l, |(ol, _, _)| ol));
                let w = max_l.to_string().len().max(1);
                let arrow_pad = " ".repeat(w);
                let pad = " ".repeat(w + 1);
                let prefix: String = var_line_content
                    .chars()
                    .take(var_c)
                    .map(|c| if c == '\t' { '\t' } else { ' ' })
                    .collect();

                let mut snippet = format!(
                    "{}--> {}:{}:{}\n{}|\n",
                    arrow_pad,
                    file_path.display(),
                    var_l,
                    var_c + 1,
                    pad
                );

                if let Some((open_l, open_c, arrow_end)) = select_ctx {
                    let open_line_content =
                        content.lines().nth(open_l.saturating_sub(1)).unwrap_or("");
                    let open_prefix: String = open_line_content
                        .chars()
                        .take(open_c.saturating_sub(1))
                        .map(|c| if c == '\t' { '\t' } else { ' ' })
                        .collect();
                    let dash_count = arrow_end.saturating_sub(open_c) + 1;
                    let dashes = "-".repeat(dash_count);
                    snippet.push_str(&format!("{:>w$} | {}\n", open_l, open_line_content, w = w));
                    snippet.push_str(&format!(
                        "{}| {open_prefix}{dashes} Select expression opened here\n",
                        pad
                    ));
                    if var_l > open_l + 1 {
                        snippet.push_str(&format!("{}...\n", pad));
                    }
                }

                snippet.push_str(&format!("{:>w$} | {}\n", var_l, var_line_content, w = w));
                snippet.push_str(&format!(
                    "{}| {prefix}^ Expected a value for variant '{variant_name}'\n",
                    pad
                ));
                return snippet;
            }
        }
    }

    // Check if error is related to an unclosed select or placeable on an earlier line
    let mut unclosed_ctx = None;
    if matches!(
        &err.kind,
        ExpectedToken('}')
            | MissingDefaultVariant
            | ExpectedInlineExpression
            | UnbalancedClosingBrace
    ) {
        let mut depth = 0;
        for (i, c) in content[..err.pos.start.min(content.len())]
            .char_indices()
            .rev()
        {
            if c == '}' {
                depth += 1;
            } else if c == '{' {
                if depth > 0 {
                    depth -= 1;
                } else {
                    let between = &content[i..err.pos.start.min(content.len())];
                    let (open_l, open_c) = offset_to_line_col(content, i);
                    if open_l < line {
                        if let Some(arrow_rel) = between.find("->") {
                            let arrow_pos = i + arrow_rel;
                            let (arrow_l, arrow_c) = offset_to_line_col(content, arrow_pos);
                            if arrow_l == open_l {
                                unclosed_ctx = Some((open_l, open_c, Some(arrow_c + 1)));
                            } else {
                                unclosed_ctx = Some((open_l, open_c, None));
                            }
                        } else {
                            unclosed_ctx = Some((open_l, open_c, None));
                        }
                    }
                    break;
                }
            }
        }
    }

    if let Some((open_l, open_c, maybe_arrow_end)) = unclosed_ctx {
        let open_line_content = content.lines().nth(open_l.saturating_sub(1)).unwrap_or("");
        let max_l = line.max(open_l);
        let w = max_l.to_string().len().max(1);
        let arrow_pad = " ".repeat(w);
        let pad = " ".repeat(w + 1);
        let prefix: String = line_content
            .chars()
            .take(col.saturating_sub(1))
            .map(|c| if c == '\t' { '\t' } else { ' ' })
            .collect();

        let msg = friendly_error_message(&err.kind, content, err.pos.start);

        let mut snippet = format!(
            "{}--> {}:{}:{}\n{}|\n",
            arrow_pad,
            file_path.display(),
            line,
            col,
            pad
        );

        if let Some(arrow_end) = maybe_arrow_end {
            let open_prefix: String = open_line_content
                .chars()
                .take(open_c.saturating_sub(1))
                .map(|c| if c == '\t' { '\t' } else { ' ' })
                .collect();
            let dash_count = arrow_end.saturating_sub(open_c) + 1;
            let dashes = "-".repeat(dash_count);
            snippet.push_str(&format!("{:>w$} | {}\n", open_l, open_line_content, w = w));
            snippet.push_str(&format!(
                "{}| {open_prefix}{dashes} Select expression opened here\n",
                pad
            ));
        } else {
            snippet.push_str(&format!(
                "{:>w$} | {} <-- Opened {{\n",
                open_l,
                open_line_content,
                w = w
            ));
        }

        if line > open_l + 1 {
            snippet.push_str(&format!("{}...\n", pad));
        }
        snippet.push_str(&format!("{:>w$} | {}\n", line, line_content, w = w));
        snippet.push_str(&format!("{}| {prefix}^ {}\n", pad, msg));
        return snippet;
    }

    let w = line.to_string().len().max(1);
    let arrow_pad = " ".repeat(w);
    let pad = " ".repeat(w + 1);
    let prefix: String = line_content
        .chars()
        .take(col.saturating_sub(1))
        .map(|c| if c == '\t' { '\t' } else { ' ' })
        .collect();

    let msg = friendly_error_message(&err.kind, content, err.pos.start);

    format!(
        "{}--> {}:{}:{}\n{}|\n{:>w$} | {}\n{}| {prefix}^ {}\n",
        arrow_pad,
        file_path.display(),
        line,
        col,
        pad,
        line,
        line_content,
        pad,
        msg,
        w = w
    )
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
                    let snippet = format_syntax_error(&file.path, &file.content, &err);
                    let error = Error::new(
                        span,
                        format!(
                            "Syntax error in Fluent file '{}'\n{}",
                            file.path.display(),
                            snippet
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
