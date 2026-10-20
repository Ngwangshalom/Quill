permit read "config"
permit env "QUILL_TEST_UNSET_SETTING"

rates = read_file("config/rates.txt")
  on FileMissing -> "none"
say "Rates: {rates}"
other = read_file("config/missing.txt")
  on FileMissing -> "no such file"
say other
say file_exists("config/rates.txt")
when env("QUILL_TEST_UNSET_SETTING")
  found value -> say value
  missing -> say "not set"
