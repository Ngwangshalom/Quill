" Quill syntax. Each kind of word has its own color.
if exists("b:current_syntax")
  finish
endif

syn keyword quillSay say
syn keyword quillWhen when else
syn keyword quillBind var set
syn keyword quillCheck check
syn keyword quillLogic and or not
syn keyword quillLoop for in
syn keyword quillList list
syn keyword quillRecord record
syn keyword quillOf of
syn keyword quillAt at
syn keyword quillAsk get sum with
syn keyword quillFn fn return
syn keyword quillMaybe maybe found missing
syn keyword quillFail fails fail on try
syn keyword quillTruth true false
syn match quillComment "--.*$" contains=@NoSpell
syn match quillExample "---\s\+example:"
syn region quillString start='"' skip='\\.' end='"' contains=quillEscape,quillInterpolation
syn region quillString start='"""' end='"""' contains=quillEscape,quillInterpolation
syn match quillEscape "\\." contained
syn region quillInterpolation start="{" end="}" contained contains=quillSay,quillWhen,quillBind,quillCheck,quillLogic,quillLoop,quillList,quillRecord,quillOf,quillAt,quillAsk,quillFn,quillMaybe,quillFail,quillTruth,quillNumber,quillOperator
syn match quillNumber "\<\d\+\(\.\d\+\)\?"
syn match quillOperator "->\|==\|!=\|<=\|>=\|[=+\-*/%<>]"

if &background == "light"
  hi def QuillSay guifg=#C45C12 ctermfg=166
  hi def QuillWhen guifg=#1D408A ctermfg=25
  hi def QuillTruth guifg=#157A42 ctermfg=28
  hi def QuillBind guifg=#6E4A96 ctermfg=97
  hi def QuillCheck guifg=#9A3D6A ctermfg=132
  hi def QuillLogic guifg=#0F6E68 ctermfg=23
  hi def QuillLoop guifg=#5A6A14 ctermfg=58
  hi def QuillList guifg=#8C4E32 ctermfg=94
  hi def QuillRecord guifg=#3E5C32 ctermfg=65
  hi def QuillOf guifg=#8A5A00 ctermfg=136
  hi def QuillAt guifg=#5C2E9A ctermfg=55
  hi def QuillAsk guifg=#15556A ctermfg=23
  hi def QuillFn guifg=#8A3D78 ctermfg=90
  hi def QuillMaybe guifg=#A85A38 ctermfg=130
  hi def QuillFail guifg=#A33B3B ctermfg=131
  hi def QuillNumber guifg=#9A4A12 ctermfg=130
  hi def QuillString guifg=#6B3A22 ctermfg=94
  hi def QuillName guifg=#1C1916 ctermfg=234
  hi def QuillComment guifg=#8A8175 ctermfg=244
  hi def QuillOperator guifg=#5C4632 ctermfg=58
  hi def QuillEscape guifg=#C45C12 ctermfg=166
else
  hi def QuillSay guifg=#F0A35E ctermfg=215
  hi def QuillWhen guifg=#7EAEF5 ctermfg=111
  hi def QuillTruth guifg=#5ECA8C ctermfg=78
  hi def QuillBind guifg=#C9A6F2 ctermfg=183
  hi def QuillCheck guifg=#F0A0B4 ctermfg=218
  hi def QuillLogic guifg=#5EC4C0 ctermfg=73
  hi def QuillLoop guifg=#C6D86A ctermfg=149
  hi def QuillList guifg=#D4896A ctermfg=173
  hi def QuillRecord guifg=#A8C5A0 ctermfg=151
  hi def QuillOf guifg=#E8C547 ctermfg=221
  hi def QuillAt guifg=#B07CFF ctermfg=141
  hi def QuillAsk guifg=#6EC1D6 ctermfg=80
  hi def QuillFn guifg=#E8A0D4 ctermfg=182
  hi def QuillMaybe guifg=#F0B090 ctermfg=180
  hi def QuillFail guifg=#E08A8A ctermfg=174
  hi def QuillNumber guifg=#E7C98A ctermfg=222
  hi def QuillString guifg=#F3E6D0 ctermfg=224
  hi def QuillName guifg=#F7F1E8 ctermfg=230
  hi def QuillComment guifg=#9A9186 ctermfg=245
  hi def QuillOperator guifg=#B7A898 ctermfg=144
  hi def QuillEscape guifg=#FFBE7A ctermfg=222
endif

hi def link quillSay QuillSay
hi def link quillWhen QuillWhen
hi def link quillTruth QuillTruth
hi def link quillBind QuillBind
hi def link quillCheck QuillCheck
hi def link quillExample QuillCheck
hi def link quillLogic QuillLogic
hi def link quillLoop QuillLoop
hi def link quillList QuillList
hi def link quillRecord QuillRecord
hi def link quillOf QuillOf
hi def link quillAt QuillAt
hi def link quillAsk QuillAsk
hi def link quillFn QuillFn
hi def link quillMaybe QuillMaybe
hi def link quillFail QuillFail
hi def link quillNumber QuillNumber
hi def link quillString QuillString
hi def link quillEscape QuillEscape
hi def link quillInterpolation QuillOperator
hi def link quillComment QuillComment
hi def link quillOperator QuillOperator

let b:current_syntax = "quill"
