-- Turns a booking into an invoice, and says plainly when it can't.
bring venues
bring pricing

fn bill(b: Booking) -> Invoice
  fails UnknownVenue, TooManyGuests
  when find_venue(b.venue)
    found v -> when
      b.guests > v.capacity -> fail TooManyGuests
      else -> return invoice_for(b, v)
    missing -> fail UnknownVenue

-- One line of the report. Each failure becomes its own message.
fn report_line(b: Booking) -> text
  invoice = bill(b)
    on UnknownVenue -> return "{b.client}: there is no venue called {b.venue}"
    on TooManyGuests -> return "{b.client}: {b.guests} guests won't fit in {b.venue}"
  return "{b.client} at {invoice.venue}, {b.nights} nights: {invoice.total} (discount {invoice.discount}, tax {invoice.tax})"

-- What the client owes. A booking we can't bill owes nothing.
fn amount_due(b: Booking) -> USD
  invoice = bill(b)
    on UnknownVenue -> return 0 USD
    on TooManyGuests -> return 0 USD
  return invoice.total
