//! What the search field asks for (docs/SEARCH.md): words separated by
//! spaces, all of which an article has; `"a phrase"` as one word; `-word`
//! for one it hasn't; `OR` between two sets of them, either of which will do.

#[derive(Debug, PartialEq)]
pub struct Term {
    pub text: String,
    /// `-word`: the article must not have it.
    pub negated: bool,
}

/// The query as alternatives (`OR`), each a set of terms that all hold.
/// Empty when it asks for nothing.
pub fn parse(q: &str) -> Vec<Vec<Term>> {
    let mut groups = vec![Vec::new()];
    let mut chars = q.chars().peekable();
    loop {
        while chars.next_if(|c| c.is_whitespace()).is_some() {}
        let Some(&first) = chars.peek() else { break };
        let negated = first == '-' || first == '－';
        if negated {
            chars.next();
        }
        let quoted = chars.next_if(|&c| is_quote(c)).is_some();
        let mut text = String::new();
        if quoted {
            for c in chars.by_ref() {
                if is_quote(c) {
                    break;
                }
                text.push(c);
            }
        } else {
            while let Some(c) = chars.next_if(|c| !c.is_whitespace()) {
                text.push(c);
            }
        }
        if !quoted && !negated && text == "OR" {
            groups.push(Vec::new());
            continue;
        }
        let text = text.trim().to_string();
        if !text.is_empty() {
            groups.last_mut().unwrap().push(Term { text, negated });
        }
    }
    groups.retain(|g| !g.is_empty());
    groups
}

fn is_quote(c: char) -> bool {
    matches!(c, '"' | '“' | '”' | '＂')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(text: &str, negated: bool) -> Term {
        Term {
            text: text.into(),
            negated,
        }
    }

    #[test]
    fn words_phrases_exclusions_and_or() {
        assert_eq!(
            parse(r#"  rust　"tauri 2" -beta OR 記事 －PR "#),
            [
                vec![t("rust", false), t("tauri 2", false), t("beta", true)],
                vec![t("記事", false), t("PR", true)],
            ]
        );
        // A quote left open runs to the end; “ ” work too.
        assert_eq!(parse("“新しい 版"), [vec![t("新しい 版", false)]]);
        // A lone or quoted OR, a lone -, and OR at either end are not operators.
        assert_eq!(
            parse(r#"OR a "OR" -OR - OR"#),
            [vec![t("a", false), t("OR", false), t("OR", true)]]
        );
        assert_eq!(parse("or"), [vec![t("or", false)]]);
        assert!(parse(r#"  "" - "#).is_empty());
    }
}
