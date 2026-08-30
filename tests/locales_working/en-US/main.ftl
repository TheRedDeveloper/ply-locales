hello = Hello World!
welcome-user = Welcome, { $user }!
items-count = You have { $count } items in your cart.
user-status = User { $name } has { $unread }
    unread messages.
missing-in-es = Only in English

-brand-name = Ply Engine
-sync-brand-name = {$case ->
   *[nominative] Firefox Account
    [genitive] Firefox Account's
    [accusative] Firefox Account
}

app-title = { -brand-name }
sync-notice = Backed up by { -sync-brand-name(case: "genitive") }.
items-formatted = You have { NUMBER($count) } items.

shared-photos =
    {$userName} {$photoCount ->
        [one] added a new photo
       *[other] added {$photoCount} new photos
    } to {$userGender ->
        [male] his stream
        [female] her stream
       *[other] their stream
    }.