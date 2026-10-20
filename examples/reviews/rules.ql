-- Our own rules about reviews. No model here, so quill test can prove them.
bring types

-- A low or angry review gets a reply from a person.
--- example: needs_reply(Rating stars = 2, summary = "wobbles", angry = false) == true
--- example: needs_reply(Rating stars = 5, summary = "great", angry = true) == true
--- example: needs_reply(Rating stars = 4, summary = "good", angry = false) == false
fn needs_reply(r: Rating) -> true/false
  return r.stars <= 2 or r.angry
