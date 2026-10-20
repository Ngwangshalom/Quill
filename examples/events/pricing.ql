-- How much a booking costs: the rate, a discount for long stays, and tax.
bring types

--- example: discount_for(5, 1000 USD) == 100 USD
--- example: discount_for(3, 1000 USD) == 50 USD
--- example: discount_for(1, 1000 USD) == 0 USD
fn discount_for(nights: number, subtotal: USD) -> USD
  return when
    nights >= 5 -> 10% of subtotal
    nights >= 3 -> 5% of subtotal
    else -> 0 USD

--- example: tax_on(1000 USD) == 80 USD
fn tax_on(amount: USD) -> USD
  require amount >= 0 USD
  eight = 8% of amount
  return round eight to cents

fn invoice_for(b: Booking, v: Venue) -> Invoice
  require b.venue == v.name
  ensure result.total >= 0 USD
  subtotal = v.rate * b.nights
  discount = discount_for(b.nights, subtotal)
  tax = tax_on(subtotal - discount)
  total = subtotal - discount + tax
  return Invoice client = b.client, venue = v.name, subtotal = subtotal, discount = discount, tax = tax, total = total
