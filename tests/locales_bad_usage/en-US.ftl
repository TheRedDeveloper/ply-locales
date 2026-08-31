-term-a = { -term-b }
-term-b =
    Term B referencing { -term-a }

-term-c = { -term-d }
greet = Hello, { $name }!
order-summary = Order { $order_id } for { $customer } has { $item_count } items.
-term-d = { -term-c }
balance = Your balance is { $amount } { $currency }.
-param-term = { $case }
bad-term-call = Missing: { -param-term } and extra: { -param-term(case: "nom", extra: "val") }
bad-func-call = Missing arg: { NUMBER() } and bad opt: { NUMBER($amount, badOpt: 1) }
missing-dep = Welcome to { -missing-brand } and { UNKNOWN_FUNC($val) }.
shared-photos =
    {$userName} {$photoCount ->
        [one] added a new photo
       *[other] added {$photoCount} new photos
    } to {$userGender ->
        [male] his stream
        [female] her stream
       *[other] their stream
    }.
