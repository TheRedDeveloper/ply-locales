use proc_macro2::Span;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use syn::Error;
use unic_langid::LanguageIdentifier;

pub struct LocaleFile {
    pub path: PathBuf,
    pub content: String,
}

pub struct LocalesData {
    pub locales: BTreeMap<String, Vec<LocaleFile>>,
    #[allow(dead_code)]
    pub locales_dir: PathBuf,
}

pub fn parse_language_id(s: &str) -> Result<LanguageIdentifier, String> {
    if s.contains('_') {
        return Err("Language tags use hyphens ('-'), not underscores ('_')".to_string());
    }
    let langid: LanguageIdentifier = s.parse().map_err(|e| format!("{e}"))?;

    let lang = langid.language.as_str();
    if lang.len() < 2 || lang.len() > 3 {
        return Err(format!("Invalid language subtag '{lang}'"));
    }

    Ok(langid)
}

pub fn load_locales(
    locales_rel_path: &str,
    manifest_dir: &Path,
    path_span: Span,
) -> Result<LocalesData, Error> {
    let full_locales_path = manifest_dir.join(locales_rel_path);

    if !full_locales_path.exists() {
        return Err(Error::new(
            path_span,
            format!(
                "Locales directory not found: {}",
                full_locales_path.display()
            ),
        ));
    }

    if !full_locales_path.is_dir() {
        return Err(Error::new(
            path_span,
            format!(
                "Locales path is not a directory: {}",
                full_locales_path.display()
            ),
        ));
    }

    let dir_entries = fs::read_dir(&full_locales_path).map_err(|e| {
        Error::new(
            path_span,
            format!(
                "Failed to read locales directory '{}': {}",
                full_locales_path.display(),
                e
            ),
        )
    })?;

    let mut entries: Vec<fs::DirEntry> = Vec::new();
    for entry in dir_entries {
        let entry = entry
            .map_err(|e| Error::new(path_span, format!("Failed to read directory entry: {}", e)))?;
        entries.push(entry);
    }
    entries.sort_by_key(|e| e.file_name());

    let mut combined_err: Option<Error> = None;
    let mut locales: BTreeMap<String, Vec<LocaleFile>> = BTreeMap::new();

    for entry in entries {
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        if file_name_str.starts_with('.') {
            continue;
        }

        let entry_path = entry.path();
        let file_type = entry.file_type().map_err(|e| {
            Error::new(
                path_span,
                format!(
                    "Failed to determine file type for '{}': {}",
                    entry_path.display(),
                    e
                ),
            )
        })?;

        if file_type.is_dir() {
            let dir_name = file_name_str.as_ref();
            if let Err(err) = parse_language_id(dir_name) {
                let error = Error::new(
                    path_span,
                    format!(
                        "Invalid language identifier in directory name '{}' in '{}': {}.",
                        dir_name, locales_rel_path, err
                    ),
                );
                match &mut combined_err {
                    Some(e) => e.combine(error),
                    None => combined_err = Some(error),
                }
                continue;
            }

            let sub_entries = fs::read_dir(&entry_path).map_err(|e| {
                Error::new(
                    path_span,
                    format!(
                        "Failed to read subdirectory '{}': {}",
                        entry_path.display(),
                        e
                    ),
                )
            })?;

            let mut ftl_files: Vec<PathBuf> = Vec::new();
            for sub_entry in sub_entries {
                let sub_entry = sub_entry.map_err(|e| {
                    Error::new(
                        path_span,
                        format!(
                            "Failed to read directory entry in '{}': {}",
                            entry_path.display(),
                            e
                        ),
                    )
                })?;
                let sub_path = sub_entry.path();
                let sub_name = sub_entry.file_name();
                let sub_name_str = sub_name.to_string_lossy();
                if sub_name_str.starts_with('.') {
                    continue;
                }
                if sub_path.is_file() && sub_path.extension().is_some_and(|ext| ext == "ftl") {
                    ftl_files.push(sub_path);
                }
            }

            if ftl_files.is_empty() {
                let error = Error::new(
                    path_span,
                    format!(
                        "Locale directory '{}' contains no .ftl files.",
                        entry_path.display()
                    ),
                );
                match &mut combined_err {
                    Some(e) => e.combine(error),
                    None => combined_err = Some(error),
                }
                continue;
            }

            ftl_files.sort();

            for ftl_path in ftl_files {
                let content = match fs::read_to_string(&ftl_path) {
                    Ok(c) => c,
                    Err(e) => {
                        let error = Error::new(
                            path_span,
                            format!("Failed to read file '{}': {}", ftl_path.display(), e),
                        );
                        match &mut combined_err {
                            Some(ce) => ce.combine(error),
                            None => combined_err = Some(error),
                        }
                        continue;
                    }
                };
                let rel_path = match ftl_path.strip_prefix(manifest_dir) {
                    Ok(p) => p.to_path_buf(),
                    Err(_) => ftl_path.clone(),
                };
                locales
                    .entry(dir_name.to_string())
                    .or_default()
                    .push(LocaleFile {
                        path: rel_path,
                        content,
                    });
            }
        } else if file_type.is_file() {
            if let Some(stem) = file_name_str.strip_suffix(".ftl") {
                if let Err(err) = parse_language_id(stem) {
                    let error = Error::new(
                        path_span,
                        format!(
                            "Invalid language identifier in filename '{}' in '{}': {}.",
                            file_name_str, locales_rel_path, err
                        ),
                    );
                    match &mut combined_err {
                        Some(e) => e.combine(error),
                        None => combined_err = Some(error),
                    }
                    continue;
                }

                let content = match fs::read_to_string(&entry_path) {
                    Ok(c) => c,
                    Err(e) => {
                        let error = Error::new(
                            path_span,
                            format!("Failed to read file '{}': {}", entry_path.display(), e),
                        );
                        match &mut combined_err {
                            Some(ce) => ce.combine(error),
                            None => combined_err = Some(error),
                        }
                        continue;
                    }
                };
                let rel_path = match entry_path.strip_prefix(manifest_dir) {
                    Ok(p) => p.to_path_buf(),
                    Err(_) => entry_path.clone(),
                };

                locales
                    .entry(stem.to_string())
                    .or_default()
                    .push(LocaleFile {
                        path: rel_path,
                        content,
                    });
            } else {
                let error = Error::new(
                    path_span,
                    format!(
                        "Unexpected file '{}' in locales directory '{}'. Expected a .ftl file or a locale directory.",
                        file_name_str, locales_rel_path
                    ),
                );
                match &mut combined_err {
                    Some(e) => e.combine(error),
                    None => combined_err = Some(error),
                }
            }
        }
    }

    if let Some(err) = combined_err {
        return Err(err);
    }

    if locales.is_empty() {
        return Err(Error::new(
            path_span,
            format!(
                "No valid Fluent (.ftl) locales found in '{}'.",
                full_locales_path.display()
            ),
        ));
    }

    Ok(LocalesData {
        locales,
        locales_dir: full_locales_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fluent_tab_support() {
        let with_tab = "msg =\n\thello\n";
        let parse_res = fluent_syntax::parser::parse(with_tab);
        println!("fluent_syntax parse result with tab: {:?}", parse_res);

        let bundle_res = fluent_bundle::FluentResource::try_new(with_tab.to_string());
        println!("fluent_bundle try_new result with tab: {:?}", bundle_res);

        let with_space = "msg =\n    hello\n";
        let parse_space = fluent_syntax::parser::parse(with_space);
        println!(
            "fluent_syntax parse result with space: {:?}",
            parse_space.is_ok()
        );

        let bundle_space = fluent_bundle::FluentResource::try_new(with_space.to_string());
        println!(
            "fluent_bundle try_new result with space: {:?}",
            bundle_space.is_ok()
        );
    }

    #[test]
    fn test_langid_parsing() {
        assert!(parse_language_id("en-US").is_ok());
        assert!(parse_language_id("nl-NL").is_ok());
        assert!(parse_language_id("es-ES").is_ok());
        assert!(parse_language_id("fr").is_ok());
        assert!(parse_language_id("123").is_err());
        assert!(parse_language_id("not_a_locale").is_err());
        assert!(parse_language_id("en_US").is_err());
        assert!(parse_language_id("invalid-language-code").is_err());
    }
}
