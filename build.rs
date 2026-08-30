fn main() {
    println!("cargo:rustc-check-cfg=cfg(has_tracked_path)");
    println!("cargo:rustc-check-cfg=cfg(has_proc_macro_diagnostic)");
    let ac = autocfg::new();

    let track_path_code = r#"
        #![feature(track_path)]
        extern crate proc_macro;
        pub fn probe() {
            proc_macro::tracked_path::path("foo");
        }
    "#;
    if ac.probe_raw(track_path_code).is_ok() {
        autocfg::emit("has_tracked_path");
    }

    let diag_code = r#"
        #![feature(proc_macro_diagnostic)]
        extern crate proc_macro;
        pub fn probe() {
            proc_macro::Diagnostic::new(proc_macro::Level::Warning, "probe").emit();
        }
    "#;
    if ac.probe_raw(diag_code).is_ok() {
        autocfg::emit("has_proc_macro_diagnostic");
    }

    autocfg::rerun_path("build.rs");
}
