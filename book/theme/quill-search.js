// The book search uses an English word list that drops short words
// such as "for", "when", and "say". A search for one of those words,
// or for several of them together, opens the chapter that teaches it.
(function () {
    if (!window.elasticlunr || !elasticlunr.Index) {
        return;
    }

    var chapters = {
        "for": "loops.html#repeating-with-for",
        "in": "loops.html#counting-from-one-number-to-another",
        "when": "decisions.html#making-decisions-with-when",
        "else": "decisions.html#acting-on-answers-when",
        "and": "decisions.html#combining-questions-and-or-not",
        "or": "decisions.html#combining-questions-and-or-not",
        "not": "decisions.html#combining-questions-and-or-not",
        "say": "getting-started.html#hello-world",
        "of": "groups.html#of-names-an-empty-list",
        "at": "groups.html#at-picks-one-item",
        "get": "asking.html#asking-for-data",
        "sum": "asking.html#the-total",
        "with": "asking.html#records-inside-other-records",
        "list": "groups.html#a-list",
        "record": "groups.html#a-record",
        "fn": "functions.html#functions",
        "return": "functions.html#why-you-use-return",
        "maybe": "maybe.html#maybe",
        "found": "maybe.html#maybe",
        "missing": "maybe.html#maybe",
        "fails": "failures.html#failures",
        "fail": "failures.html#failures",
        "on": "failures.html#failures",
        "try": "failures.html#failures",
        "require": "contracts.html#contracts",
        "ensure": "contracts.html#contracts",
        "where": "asking.html#only-some-records-where"
    };

    var search = elasticlunr.Index.prototype.search;
    elasticlunr.Index.prototype.search = function (query, options) {
        var results = search.call(this, query, options);
        var text = typeof query === "string" ? query : "";
        var words = text.trim().toLowerCase().split(/\s+/).filter(Boolean);
        var urls = window.search && window.search.doc_urls;
        if (!urls || words.length === 0) {
            return results;
        }

        var onlyQuillWords = words.every(function (word) {
            return Object.prototype.hasOwnProperty.call(chapters, word);
        });
        if (!onlyQuillWords) {
            return results;
        }

        var seen = {};
        results.forEach(function (result) {
            seen[result.ref] = true;
        });

        words.forEach(function (word) {
            var ref = String(urls.indexOf(chapters[word]));
            if (ref === "-1" || seen[ref]) {
                return;
            }
            seen[ref] = true;
            var found = { ref: ref, score: 1 };
            if (this.documentStore.hasDoc(ref)) {
                found.doc = this.documentStore.getDoc(ref);
            }
            results.unshift(found);
        }, this);

        return results;
    };
})();
