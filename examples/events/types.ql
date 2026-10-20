-- The shapes of data the whole project shares.
-- Every other file brings this one.

type Venue
  name: text
  capacity: number where 1..2000
  rate: USD

type Booking
  client: text
  venue: text
  guests: number where 1..2000
  nights: number where 1..14
  paid: true/false

type Invoice
  client: text
  venue: text
  subtotal: USD
  discount: USD
  tax: USD
  total: USD
