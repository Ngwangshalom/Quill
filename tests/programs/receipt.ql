-- Arithmetic, values inside text, var/set and when.
price = 12.50
qty = 3
var total = price * qty
var visits = 0

set visits = visits + 1
set total = total + 0.1 + 0.2

say "Items: {qty} x {price}"
say "Total: {total}"
say "Visits: {visits}"

when
  total > 100 -> say "Free delivery!"
  total > 30 -> say "Delivery: 5"
  else -> say "Delivery: 10"

check 0.1 + 0.2 == 0.3
check total == 37.80
check visits >= 1
