greet = Hello, { $name }!
order-summary = Order { $order_id } for { $customer } has { $item_count } items.
balance = Your balance is { $amount } { $currency }.
shared-photos =
    {$userName} {$photoCount ->
        [one] added a new photo
       *[other] added {$photoCount} new photos
    } to {$userGender ->
        [male] his stream
        [female] her stream
       *[other] their stream
    }.
