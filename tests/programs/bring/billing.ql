-- Brought into app.ql: functions and types only.
bring "shared/money.ql"

--- example: deposit(1000 USD) == 200 USD
fn deposit(fee: USD) -> USD
  return part(fee, 20)

type Invoice
  to: text
  amount: USD
