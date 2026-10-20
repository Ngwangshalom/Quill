permit write "out"

fn save(text: text)
  fails CantWrite
  try write_file("out/note.txt", text)
