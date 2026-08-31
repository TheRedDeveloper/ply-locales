<p align="center">
  <img src="logo.png" alt="ply-locales" width="600">
</p>

<h3 align="center">Compile-checked Fluent localization for Rust. Dead simple.</h3>

<p align="center">
  <a href="https://plyx.iz.rs/docs/localization">Documentation</a>
</p>

<p align="center">
  <a href="https://crates.io/crates/ply-locales"><img src="https://img.shields.io/crates/v/ply-locales.svg" alt="crates.io"></a>
  <a href="LICENSE.md"><img src="https://img.shields.io/badge/license-0BSD-blue.svg" alt="License: 0BSD"></a>
</p>

---

`ply-locales` is a compile-checked procedural macro for Project Fluent. It checks your `.ftl` translation files at build time and generates typed Rust functions for every message.

## What you get

```fluent
# locales/en-US.ftl
hello = Hello World!
order-summary = Order { $order_id } for { $customer } has { $item_count } items.
```

```rust
#[ply_locales::ply_locales("locales")]
pub mod t {}

fn main() {
    println!("{}", t::hello());
    println!("{}", t::order_summary(1042, "Alice", 3));

    t::set_locale("de-DE");
}
```

- One macro call
- Typed functions with IDE hover docs with direct links to `.ftl` source lines
- Lazy loading
- Automatic fallback

## Quickstart

Add both `ply-locales` and `fluent-bundle` to your `Cargo.toml`:

```toml
[dependencies]
ply-locales = "0.1"
fluent-bundle = "0.16"
```

Because `ply-locales` is a proc macro crate, it cannot provide functions at runtime. The generated code calls `fluent_bundle` directly, so you have to add `fluent-bundle` in `Cargo.toml`.

### Directory structure

Place your translation files in a `locales/` directory:

```text
locales/
├── en-US/
│   └── main.ftl
├── de-DE.ftl
└── es-ES.ftl
```

`ply-locales` supports flat files (`locales/de-DE.ftl`), subdirectories (`locales/en-US/main.ftl`), or a mix of both. File and directory names must be valid language identifiers like `en-US`, `de-DE`, `fr` or `zh-Hant-TW`.

### API

The annotated module exposes:

```rust
pub const AVAILABLE_LOCALES: &[&'static str];
pub fn set_locale(locale: &str) -> bool; // true if available
pub fn current_locale() -> String;
pub fn your_message_name(args...) -> String; // for every message in your .ftl files
```

You can also define custom Rust functions:

```rust
#[ply_locales::ply_locales("locales")]
pub mod t {
    pub fn upper(s: &str) -> String {
        s.to_uppercase()
    }
}
```

And call them in your `.ftl` templates:

```fluent
greet = Hello, { UPPER($name) }!
```

- Rust `snake_case` functions map to uppercase in Fluent
- Calls in Fluent templates are checked at compile time
- Functions remain accessible directly in Rust as well

## Compile errors

Readable errors are always emitted at compile time, such as mismatched variables:

```text
error: Mismatched Fluent variables in message 'order-summary' for locale 'de-DE'
        --> locales/de-DE.ftl:2
         |
       2 | order-summary = Bestellung { $order_id } für { $client } hat { $item_count } Artikel.
         |
         = expected: [$order_id, $customer, $item_count]
         = found:    [$order_id, $client, $item_count]
```

Missing arguments:

```text
error: Missing argument 'hey' in call to term '-variable-inside' in message 'greet' for locale 'en-US'
        --> locales/en-US.ftl:2:28-43
         |
       2 | greet = Hello, { $name } { -variable-inside }!
         |                            ^^^^^^^^^^^^^^^^ Missing argument 'hey'
```

Circular dependencies:

```text
error: Circular dependency detected in locale 'en-US'
        --> locales/en-US.ftl
         |
       1 | -a = { -b }
       2 | -b = { -a }
         |
         = cycle: -a -> -b -> -a
```

and many more! Syntax errors also emit readable compile-time errors:

```text
error: Syntax error in Fluent file 'locales/en-US.ftl'
        --> locales/en-US.ftl:1:16-18
         |
       1 | greet = Hello, { }
         |                ^^^ Expression can't be empty
```

Missing translations emit a compiler warning and fall back to the default language at runtime.

Additional messages in a translated locale emit a compiler warning and are ignored at runtime.

This makes shooting yourself in the foot a lot harder.

## License

[Zero-Clause BSD](LICENSE). Use it for anything. No attribution required.
