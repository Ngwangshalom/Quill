// Colors Quill examples in the book the way the editors do.
// The patterns follow editors/vscode/syntaxes/quill.tmLanguage.json, and the colors
// in quill.css follow editors/vscode/extension.js. Change all three together.
//
// highlight.js stops at a block whose language it doesn't know, so "output"
// is registered too, as plain text.
(function () {
    if (!window.hljs) {
        return;
    }

    hljs.registerLanguage("output", function () {
        return { name: "Output", contains: [] };
    });

    hljs.registerLanguage("quill", function () {
        function word(kind, pattern) {
            return { className: "quill-" + kind, begin: pattern, relevance: 0 };
        }

        var code = [];
        var ensured = [word("contract", /\b(?:ensure|result)\b/)];
        var escape = { className: "quill-escape", begin: /\\./, relevance: 0 };
        var inside = {
            className: "quill-operator",
            begin: /\{/,
            end: /\}/,
            contains: code,
            relevance: 0
        };

        code.push(
            word("check", /---[ \t]+example:/),
            word("comment", /--.*/),
            {
                className: "quill-string",
                begin: /"""/,
                end: /"""/,
                contains: [escape, inside],
                relevance: 0
            },
            {
                className: "quill-string",
                begin: /"/,
                end: /"/,
                contains: [escape, inside],
                relevance: 0
            },
            word("number", /\b\d+(?:\.\d+)?/),
            word("say", /\bsay\b/),
            word("when", /\b(?:when|else)\b/),
            word("bind", /\b(?:var|set)\b/),
            word("check", /\bcheck\b/),
            word("logic", /\b(?:and|or|not)\b/),
            word("loop", /\b(?:for|in)\b/),
            word("list", /\blist\b/),
            word("record", /\brecord\b/),
            word("of", /\bof\b/),
            word("at", /\bat\b/),
            word("text", /\b(?:trim|uppercase|lowercase|join|replace)\b(?=[ \t]+[A-Za-z_"0-9])/),
            word("text", /\blength\b(?=\s+of\b)/),
            word("text", /\bcontains\b(?=[ \t]+[^\s=])/),
            word("text", /\b(?:starts|ends)[ \t]+with\b/),
            word("ask", /\b(?:get|sum|with)\b/),
            word("ask", /\bcount\b(?=\s+of\b)/),
            word("ask", /\baverage\b(?=\s+[A-Za-z_][A-Za-z0-9_]*\s+of\b)/),
            word("ask", /\bsorted\b(?=\s+by\b)/),
            word("fn", /\b(?:fn|return)\b/),
            word("maybe", /\b(?:maybe|found|missing)\b/),
            word("failure", /\b(?:fails|fail|on|try)\b/),
            {
                begin: /\bensure\b/,
                returnBegin: true,
                end: /$/,
                contains: ensured,
                relevance: 0
            },
            word("contract", /\brequire\b/),
            word("rule", /\b(?:where|type|as)\b/),
            word("money", /\b(?:round|split|convert)\b/),
            word("module", /\b(?:bring|route|respond)\b/),
            word("permit", /\bpermit\b/),
            word("ask-model", /\bask\b/),
            word("truth", /\b(?:true|false)\b/),
            word("operator", /->|==|!=|<=|>=|[=+\-*\/%<>]/),
            word("name", /\b[A-Za-z_][A-Za-z0-9_]*\b/)
        );
        ensured.push.apply(ensured, code);

        return { name: "Quill", aliases: ["ql"], contains: code };
    });
})();
