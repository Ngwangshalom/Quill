;;; quill.el --- Quill coloring and mistakes for Emacs -*- lexical-binding: t; -*-

;; Each kind of word has its own color.
;; Mistakes come from `quill lsp`, the same check every other editor uses.

(defgroup quill nil "Quill." :group 'languages)

(defface quill-say
  '((((background light)) :foreground "#C45C12")
    (t :foreground "#F0A35E"))
  "say." :group 'quill)
(defface quill-when
  '((((background light)) :foreground "#1D408A")
    (t :foreground "#7EAEF5"))
  "when and else." :group 'quill)
(defface quill-truth
  '((((background light)) :foreground "#157A42")
    (t :foreground "#5ECA8C"))
  "true and false." :group 'quill)
(defface quill-bind
  '((((background light)) :foreground "#6E4A96")
    (t :foreground "#C9A6F2"))
  "var and set." :group 'quill)
(defface quill-check
  '((((background light)) :foreground "#9A3D6A")
    (t :foreground "#F0A0B4"))
  "check and examples." :group 'quill)
(defface quill-logic
  '((((background light)) :foreground "#0F6E68")
    (t :foreground "#5EC4C0"))
  "and, or, and not." :group 'quill)
(defface quill-loop
  '((((background light)) :foreground "#5A6A14")
    (t :foreground "#C6D86A"))
  "for and in." :group 'quill)
(defface quill-list
  '((((background light)) :foreground "#8C4E32")
    (t :foreground "#D4896A"))
  "list." :group 'quill)
(defface quill-record
  '((((background light)) :foreground "#3E5C32")
    (t :foreground "#A8C5A0"))
  "record." :group 'quill)
(defface quill-of
  '((((background light)) :foreground "#8A5A00")
    (t :foreground "#E8C547"))
  "of." :group 'quill)
(defface quill-at
  '((((background light)) :foreground "#5C2E9A")
    (t :foreground "#B07CFF"))
  "at." :group 'quill)
(defface quill-ask
  '((((background light)) :foreground "#15556A")
    (t :foreground "#6EC1D6"))
  "get, sum, and with." :group 'quill)
(defface quill-fn
  '((((background light)) :foreground "#8A3D78")
    (t :foreground "#E8A0D4"))
  "fn and return." :group 'quill)
(defface quill-maybe
  '((((background light)) :foreground "#A85A38")
    (t :foreground "#F0B090"))
  "maybe, found, and missing." :group 'quill)
(defface quill-fail
  '((((background light)) :foreground "#A33B3B")
    (t :foreground "#E08A8A"))
  "fails, fail, on, and try." :group 'quill)
(defface quill-contract
  '((((background light)) :foreground "#7A5A20")
    (t :foreground "#C8A06A"))
  "require, ensure, and result inside ensure." :group 'quill)
(defface quill-rule
  '((((background light)) :foreground "#1A5270")
    (t :foreground "#6AA8C8"))
  "where: a range, or the records to keep." :group 'quill)
(defface quill-string
  '((((background light)) :foreground "#6B3A22")
    (t :foreground "#F3E6D0"))
  "Text." :group 'quill)
(defface quill-number
  '((((background light)) :foreground "#9A4A12")
    (t :foreground "#E7C98A"))
  "Numbers." :group 'quill)
(defface quill-comment
  '((((background light)) :foreground "#8A8175")
    (t :foreground "#9A9186"))
  "Comments." :group 'quill)

(defvar quill-font-lock
  '(("--.*" . 'quill-comment)
    ("---[ \t]+example:" . 'quill-check)
    ("\\b\\(say\\)\\b" . 'quill-say)
    ("\\b\\(when\\|else\\)\\b" . 'quill-when)
    ("\\b\\(var\\|set\\)\\b" . 'quill-bind)
    ("\\b\\(check\\)\\b" . 'quill-check)
    ("\\b\\(and\\|or\\|not\\)\\b" . 'quill-logic)
    ("\\b\\(for\\|in\\)\\b" . 'quill-loop)
    ("\\b\\(list\\)\\b" . 'quill-list)
    ("\\b\\(record\\)\\b" . 'quill-record)
    ("\\b\\(of\\)\\b" . 'quill-of)
    ("\\b\\(at\\)\\b" . 'quill-at)
    ("\\b\\(get\\|sum\\|with\\)\\b" . 'quill-ask)
    ("\\b\\(fn\\|return\\)\\b" . 'quill-fn)
    ("\\b\\(maybe\\|found\\|missing\\)\\b" . 'quill-maybe)
    ("\\b\\(fails\\|fail\\|on\\|try\\)\\b" . 'quill-fail)
    ("\\b\\(require\\|ensure\\)\\b" . 'quill-contract)
    ("\\b\\(where\\)\\b" . 'quill-rule)
    ("\\bensure\\b" ("\\bresult\\b" nil nil (0 'quill-contract)))
    ("\\b\\(true\\|false\\)\\b" . 'quill-truth)
    ("\\b[0-9]+\\(\\.[0-9]+\\)?\\b" . 'quill-number)
    ("\"\\(\\\\.\\|[^\"]\\)*\"" . 'quill-string)))

;;;###autoload
(define-derived-mode quill-mode prog-mode "Quill"
  "A major mode for Quill."
  (setq font-lock-defaults '(quill-font-lock))
  (setq-local comment-start "-- ")
  (setq-local indent-tabs-mode nil)
  (setq-local tab-width 2)
  (when (and (executable-find "quill") (fboundp 'eglot-ensure))
    (eglot-ensure)))

;;;###autoload
(add-to-list 'auto-mode-alist '("\\.ql\\'" . quill-mode))

(with-eval-after-load 'eglot
  (add-to-list 'eglot-server-programs '(quill-mode . ("quill" "lsp"))))

(provide 'quill)
;;; quill.el ends here
