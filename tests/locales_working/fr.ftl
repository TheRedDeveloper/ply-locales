hello = Bonjour le monde!
welcome-user = Bienvenue, { $user }!
items-count = Vous avez { $count } articles dans votre panier.
user-status = { $unread } messages non lus pour { $name }.
menu-file = Fichier
menu-edit = Modifier
menu-save = Enregistrer
missing-in-es = Seulement en anglais (mais en francais aussi)

-brand-name = Ply Engine
-sync-brand-name = {$case ->
   *[nominative] Compte Firefox
    [genitive] de Compte Firefox
    [accusative] Compte Firefox
}

app-title = { -brand-name }
sync-notice = Sauvegardé par { -sync-brand-name(case: "genitive") }.
items-formatted = Vous avez { NUMBER($count) } articles.

shared-photos =
    {$userName} a ajouté {$photoCount ->
        [one] une nouvelle photo
       *[other] {$photoCount} nouvelles photos
    } à {$userGender ->
        [male] son flux
        [female] son flux
       *[other] leur flux
    }.
custom-upper = Formaté: { UPPER($text) }
sum-result = Somme: { CUSTOM_CALC($a, $b) }
