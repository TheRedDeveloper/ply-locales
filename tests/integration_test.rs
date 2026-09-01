use ply_locales::ply_locales;

#[ply_locales("tests/locales_working")]
pub mod t {
    pub fn upper(s: &str) -> String {
        s.to_uppercase()
    }

    pub fn custom_calc(a: i64, b: i64) -> i64 {
        a + b
    }
}

#[test]
fn test_locales_working_runtime() {
    assert_eq!(t::AVAILABLE_LOCALES, &["en-US", "es-ES", "fr"]);
    assert_eq!(t::current_locale(), "en-US");

    assert_eq!(t::hello(), "Hello World!");
    assert_eq!(t::menu_file(), "File");
    assert_eq!(t::menu_edit(), "Edit");
    assert_eq!(t::menu_save(), "Save");

    // Custom function tests
    assert_eq!(t::custom_upper("alex"), "Formatted: ALEX");
    assert_eq!(t::sum_result(10, 25), "Sum: 35");
    // Verify direct Rust function invocation also works
    assert_eq!(t::upper("hello"), "HELLO");
    assert_eq!(t::custom_calc(3, 4), 7);

    // Built-in VOID test
    assert_eq!(t::void_var("male"), "He");
    assert_eq!(t::void_var("female"), "They");

    // Terms and Functions test
    assert_eq!(t::app_title(), "Ply Engine");
    assert_eq!(t::sync_notice(), "Backed up by Firefox Account's.");
    assert_eq!(t::items_formatted(15), "You have 15 items.");

    // Multiline & nested select expressions
    assert_eq!(
        t::shared_photos("Alice", 1, "female"),
        "Alice added a new photo to her stream."
    );
    assert_eq!(
        t::shared_photos("Bob", 5, "male"),
        "Bob added 5 new photos to his stream."
    );

    // Format with variables
    assert_eq!(t::welcome_user("Alice"), "Welcome, Alice!");
    assert_eq!(t::items_count(42), "You have 42 items in your cart.");
    assert_eq!(t::user_status("Bob", 5), "User Bob has 5\nunread messages.");

    // Switch to es-ES
    assert!(t::set_locale("es-ES"));
    assert_eq!(t::current_locale(), "es-ES");

    assert_eq!(t::hello(), "¡Hola Mundo!");
    assert_eq!(t::menu_file(), "Archivo");
    assert_eq!(t::menu_save(), "Guardar");

    assert_eq!(
        t::user_status("Carlos", 10),
        "Tienes 10 mensajes sin leer para el usuario Carlos."
    );
    assert_eq!(
        t::shared_photos("Carlos", 2, "male"),
        "Carlos agregó 2 fotos nuevas a su flujo de él."
    );

    assert_eq!(t::custom_upper("alex"), "Formateado: ALEX");
    assert_eq!(t::sum_result(10, 25), "Suma: 35");
    assert_eq!(t::void_var("male"), "Él");
    assert_eq!(t::void_var("female"), "Ellos");

    assert_eq!(t::missing_in_es(), "Only in English");

    // Switch to fr
    assert!(t::set_locale("fr"));
    assert_eq!(t::current_locale(), "fr");
    assert_eq!(t::hello(), "Bonjour le monde!");

    assert_eq!(t::custom_upper("alex"), "Formaté: ALEX");
    assert_eq!(t::sum_result(10, 25), "Somme: 35");
    assert_eq!(t::void_var("male"), "");
    assert_eq!(t::void_var("female"), "");

    assert_eq!(
        t::user_status("Amélie", 3),
        "3 messages non lus pour Amélie."
    );
    assert_eq!(
        t::shared_photos("Amélie", 1, "female"),
        "Amélie a ajouté une nouvelle photo à son flux."
    );

    assert!(!t::set_locale("invalid-locale"));
    assert_eq!(t::current_locale(), "fr");
}
