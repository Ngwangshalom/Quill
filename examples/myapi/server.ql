-- My first Quill web service.
-- Run it on your computer:  quill run server.ql
-- Then open http://localhost:8080
bring web

type Order
  item: text
  quantity: number where 1..100
  price: USD

fn total(o: Order) -> USD
  return o.price * o.quantity

say "The shop is open."

-- Hosting services call this to see that the server is alive.
route GET "/health"
  respond 200, "ok"

route GET "/"
  respond 200, "Welcome to my shop. POST an order to /orders."

route GET "/hello/{name}"
  respond 200, "Hello, {name}!"

-- Send:  {"item": "Pen", "quantity": 3, "price": "2.50"}
route POST "/orders"
  order = request.body as Order
    on Invalid -> respond 400, "Send an item, a quantity from 1 to 100, and a price."
  respond 201, record item = order.item, quantity = order.quantity, total = total(order)
