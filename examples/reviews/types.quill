-- The shapes of data the service reads and sends.

-- What a customer sends us, as JSON:
-- {"product": "Desk lamp", "comment": "Bright and cheap, but it wobbles."}
type Feedback
  product: text
  comment: text

-- What the model gives back. Its answer is checked against every field
-- and range, so stars outside 1..5 never reach our code.
type Rating
  stars: number where 1..5
  summary: text
  angry: true/false

-- What the service answers with, as JSON.
type Review
  product: text
  stars: number
  summary: text
  needs_reply: true/false
