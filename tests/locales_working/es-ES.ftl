hello = ¡Hola Mundo!
welcome-user = ¡Bienvenido, { $user }!
items-count = Tienes { $count } artículos en tu carrito.
user-status = Tienes { $unread } mensajes sin leer para el usuario { $name }.
menu-file = Archivo
menu-edit = Editar
menu-save = Guardar
extra-key-in-es = Clave extra

-brand-name = Ply Engine
-sync-brand-name = {$case ->
   *[nominative] Cuenta de Firefox
    [genitive] de Cuenta de Firefox
    [accusative] Cuenta de Firefox
}

app-title = { -brand-name }
sync-notice = Respaldado por { -sync-brand-name(case: "genitive") }.
items-formatted = Tienes { NUMBER($count) } artículos.

shared-photos =
    {$userName} {$photoCount ->
        [one] agregó una nueva foto
       *[other] agregó {$photoCount} fotos nuevas
    } a su flujo de {$userGender ->
        [male] él
        [female] ella
       *[other] ellos
    }.
custom-upper = Formateado: { UPPER($text) }
sum-result = Suma: { CUSTOM_CALC($a, $b) }
void-var = { $gender ->
    [male] Él
   *[other] Ellos
}
