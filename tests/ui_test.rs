#[test]
fn test_ui_errors() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/fail_wrong_format.rs");
    t.compile_fail("tests/ui/fail_bad_variables.rs");
    t.compile_fail("tests/ui/fail_syntax_error.rs");
}
