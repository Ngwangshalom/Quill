-- The venues we rent out, and how to look one up by name.
bring types

fn all_venues() -> list of Venue
  hall = Venue name = "Town Hall", capacity = 300, rate = 1200 USD
  garden = Venue name = "Rose Garden", capacity = 120, rate = 800 USD
  loft = Venue name = "Harbour Loft", capacity = 60, rate = 450 USD
  return list hall, garden, loft

-- Gives the venue with this name, or missing when there is none.
fn find_venue(wanted: text) -> maybe Venue
  for v in all_venues()
    when
      v.name == wanted -> return found v
  return missing
