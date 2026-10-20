-- Run this file: quill run examples/events/main.ql
-- It bills this week's bookings and writes a report to reports/bookings.txt.
permit write "reports"

bring types
bring billing

amina = Booking client = "Amina", venue = "Town Hall", guests = 250, nights = 3, paid = true
kofi = Booking client = "Kofi", venue = "Rose Garden", guests = 90, nights = 6, paid = false
lena = Booking client = "Lena", venue = "Harbour Loft", guests = 85, nights = 1, paid = false
tom = Booking client = "Tom", venue = "Sky Bar", guests = 40, nights = 2, paid = true
requests = list amina, kofi, lena, tom

var report = "Bookings report\n\n"
var expected = 0 USD
for b in requests
  say report_line(b)
  set report = "{report}{report_line(b)}\n"
  set expected = expected + amount_due(b)

say ""
say "Expected income: {expected}"
set report = "{report}\nExpected income: {expected}\n"

unpaid = requests where paid == false
say "Still to pay: {get client of unpaid}"

-- Kofi pays in two parts.
parts = split amount_due(kofi) into 2
say "Kofi pays {parts at 1} now and {parts at 2} later"

check amount_due(tom) == 0 USD
check amount_due(amina) == 3693.6 USD

write_file("reports/bookings.txt", report)
  on CantWrite -> say "Couldn't write the report."
say "Report saved to reports/bookings.txt"
