//! Just enough XML to read interchange files: elements, attributes, text and the five
//! predefined entities plus numeric references. Comments, processing instructions,
//! doctypes and CDATA are skipped or kept as text; namespaces are left in names.

use crate::TimelineError;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Element {
    pub name: String,
    pub attributes: Vec<(String, String)>,
    pub children: Vec<Element>,
    /// The text directly inside, trimmed, with the text between child elements joined.
    pub text: String,
}

impl Element {
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    pub fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|child| child.name == name)
    }

    pub fn children<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.children.iter().filter(move |child| child.name == name)
    }

    /// The element down `path` of child names, taking the first match at each step.
    pub fn find(&self, path: &[&str]) -> Option<&Element> {
        path.iter()
            .try_fold(self, |element, name| element.child(name))
    }

    pub fn text_at(&self, path: &[&str]) -> Option<&str> {
        self.find(path).map(|element| element.text.as_str())
    }

    pub fn number_at(&self, path: &[&str]) -> Option<i64> {
        self.text_at(path)?.trim().parse().ok()
    }
}

/// Parses `text` into its root element.
pub(crate) fn parse(text: &str) -> Result<Element, TimelineError> {
    let mut parser = Parser { text, at: 0 };
    let mut stack: Vec<Element> = Vec::new();
    loop {
        let rest = &text[parser.at..];
        let Some(open) = rest.find('<') else {
            break;
        };
        if let Some(top) = stack.last_mut() {
            push_text(top, &decode(&rest[..open]));
        }
        parser.at += open;
        let rest = &text[parser.at..];
        if rest.starts_with("<!--") {
            parser.skip_past("-->")?;
        } else if let Some(body) = rest.strip_prefix("<![CDATA[") {
            let end = body.find("]]>").ok_or_else(|| parser.error())?;
            if let Some(top) = stack.last_mut() {
                push_text(top, &body[..end]);
            }
            parser.at += "<![CDATA[".len() + end + "]]>".len();
        } else if rest.starts_with("<?") {
            parser.skip_past("?>")?;
        } else if rest.starts_with("<!") {
            parser.skip_doctype()?;
        } else if let Some(name) = rest.strip_prefix("</") {
            let end = name.find('>').ok_or_else(|| parser.error())?;
            let element = stack.pop().ok_or_else(|| parser.error())?;
            if element.name != name[..end].trim() {
                return Err(parser.error());
            }
            parser.at += 2 + end + 1;
            match stack.last_mut() {
                Some(parent) => parent.children.push(element),
                None => return Ok(element),
            }
        } else {
            let (element, closed) = parser.start_tag()?;
            if closed {
                match stack.last_mut() {
                    Some(parent) => parent.children.push(element),
                    None => return Ok(element),
                }
            } else {
                stack.push(element);
            }
        }
    }
    Err(parser.error())
}

fn push_text(element: &mut Element, text: &str) {
    let text = text.trim();
    if !text.is_empty() {
        element.text.push_str(text);
    }
}

struct Parser<'a> {
    text: &'a str,
    at: usize,
}

impl Parser<'_> {
    fn error(&self) -> TimelineError {
        let line = self.text[..self.at.min(self.text.len())]
            .matches('\n')
            .count()
            + 1;
        TimelineError::InvalidXml { line }
    }

    fn skip_past(&mut self, end: &str) -> Result<(), TimelineError> {
        let found = self.text[self.at..].find(end).ok_or_else(|| self.error())?;
        self.at += found + end.len();
        Ok(())
    }

    /// A doctype may hold an internal subset in brackets, with `>` inside it.
    fn skip_doctype(&mut self) -> Result<(), TimelineError> {
        let mut depth = 0_usize;
        for (offset, byte) in self.text[self.at..].bytes().enumerate() {
            match byte {
                b'[' => depth += 1,
                b']' => depth = depth.saturating_sub(1),
                b'>' if depth == 0 => {
                    self.at += offset + 1;
                    return Ok(());
                }
                _ => {}
            }
        }
        Err(self.error())
    }

    /// Reads `<name attr="value" ...>` or its self-closing form, which `closed` reports.
    fn start_tag(&mut self) -> Result<(Element, bool), TimelineError> {
        let rest = &self.text[self.at + 1..];
        let name_end = rest
            .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .ok_or_else(|| self.error())?;
        let mut element = Element {
            name: rest[..name_end].to_owned(),
            ..Element::default()
        };
        if element.name.is_empty() {
            return Err(self.error());
        }
        let mut cursor = &rest[name_end..];
        loop {
            cursor = cursor.trim_start();
            if let Some(after) = cursor.strip_prefix("/>") {
                self.at = self.text.len() - after.len();
                return Ok((element, true));
            }
            if let Some(after) = cursor.strip_prefix('>') {
                self.at = self.text.len() - after.len();
                return Ok((element, false));
            }
            let equals = cursor.find('=').ok_or_else(|| self.error())?;
            let key = cursor[..equals].trim().to_owned();
            let value = cursor[equals + 1..].trim_start();
            let quote = value.chars().next().filter(|c| *c == '"' || *c == '\'');
            let quote = quote.ok_or_else(|| self.error())?;
            let close = value[1..].find(quote).ok_or_else(|| self.error())?;
            element.attributes.push((key, decode(&value[1..1 + close])));
            cursor = &value[close + 2..];
        }
    }
}

fn decode(text: &str) -> String {
    let mut decoded = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        decoded.push_str(&rest[..amp]);
        let entity = &rest[amp + 1..];
        let Some(semicolon) = entity.find(';').filter(|&end| end <= 10) else {
            decoded.push('&');
            rest = entity;
            continue;
        };
        let name = &entity[..semicolon];
        let character = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => name
                .strip_prefix("#x")
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| name.strip_prefix('#').and_then(|dec| dec.parse().ok()))
                .and_then(char::from_u32),
        };
        match character {
            Some(character) => {
                decoded.push(character);
                rest = &entity[semicolon + 1..];
            }
            None => {
                decoded.push('&');
                rest = entity;
            }
        }
    }
    decoded.push_str(rest);
    decoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_elements_attributes_entities_and_skipped_markup_are_read() {
        let text = "<?xml version=\"1.0\"?>\n<!DOCTYPE xmeml [<!ENTITY x \"y\">]>\n\
            <root a='1 &amp; 2'><!-- note -->\n  <item id=\"i\"/>\n  \
            <name>R&#233;sum&#xE9; &lt;cut&gt;</name><![CDATA[<raw>]]></root>";
        let root = parse(text).unwrap();
        assert_eq!(root.attribute("a"), Some("1 & 2"));
        assert_eq!(
            root.child("item").and_then(|item| item.attribute("id")),
            Some("i")
        );
        assert_eq!(root.text_at(&["name"]), Some("Résumé <cut>"));
        assert_eq!(root.text, "<raw>");
    }

    #[test]
    fn a_mismatched_close_reports_its_line() {
        let error = parse("<a>\n<b>\n</a>").unwrap_err();
        assert_eq!(error, TimelineError::InvalidXml { line: 3 });
    }
}
