permit read "config"
p = "config/../rates.ql"
x = read_file(p)
  on FileMissing -> "x"
say x
