//! Allocation-free source decoding selection shared by capture and the checker executable.
//! The supported profile is UTF-8 (with an optional BOM), ASCII and Python Latin-1.

#[derive(Clone, Copy, Debug)]
pub(crate) enum DecodedSource<'a> {
    Utf8 {
        text: &'a str,
        original_start: usize,
    },
    Latin1(&'a [u8]),
}

impl<'a> DecodedSource<'a> {
    /// Select from captured bytes. An error retains the declared label, when present.
    pub(crate) fn select(bytes: &'a [u8], python: bool) -> Result<Self, Option<&'a str>> {
        let bom = bytes.starts_with(b"\xef\xbb\xbf");
        let payload = if bom { &bytes[3..] } else { bytes };
        let declared = python.then(|| python_cookie(payload)).flatten();
        let codec = declared.map_or(Codec::Utf8, codec);
        if bom && (codec != Codec::Utf8 || declared.is_some_and(|label| !normalized_utf8(label))) {
            return Err(declared);
        }
        match codec {
            Codec::Latin1 => Ok(Self::Latin1(payload)),
            Codec::Ascii if !payload.is_ascii() => Err(declared),
            Codec::Utf8 | Codec::Ascii => std::str::from_utf8(payload)
                .map(|text| Self::Utf8 {
                    text,
                    original_start: usize::from(bom) * 3,
                })
                .map_err(|_| declared),
            Codec::Unsupported => Err(declared),
        }
    }

    pub(crate) fn characters(self) -> impl Iterator<Item = (usize, char)> + 'a {
        let (utf8, latin1, start) = match self {
            Self::Utf8 {
                text,
                original_start,
            } => (Some(text), None, original_start),
            Self::Latin1(bytes) => (None, Some(bytes), 0),
        };
        utf8.into_iter()
            .flat_map(str::char_indices)
            .map(move |(offset, ch)| (offset + start, ch))
            .chain(
                latin1
                    .into_iter()
                    .flat_map(|bytes| bytes.iter().enumerate())
                    .map(|(offset, byte)| (offset, char::from(*byte))),
            )
    }

    pub(crate) fn original_len(self) -> usize {
        match self {
            Self::Utf8 {
                text,
                original_start,
            } => text.len() + original_start,
            Self::Latin1(bytes) => bytes.len(),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Codec {
    Utf8,
    Latin1,
    Ascii,
    Unsupported,
}

fn normalized_utf8(label: &str) -> bool {
    let mut prefix = label.bytes().take(6).map(|byte| {
        if byte == b'_' {
            b'-'
        } else {
            byte.to_ascii_lowercase()
        }
    });
    prefix.by_ref().take(5).eq(b"utf-8".iter().copied())
        && prefix.next().is_none_or(|byte| byte == b'-')
}

fn codec(label: &str) -> Codec {
    // Python's tokenizer normalizes these prefixes in the first twelve bytes before
    // codec lookup. Labels are ASCII by construction; comparison does not allocate.
    let mut normalized = [0_u8; 12];
    let count = label.len().min(normalized.len());
    for (to, from) in normalized.iter_mut().zip(label.bytes()) {
        *to = if from == b'_' {
            b'-'
        } else {
            from.to_ascii_lowercase()
        };
    }
    let prefix = &normalized[..count];
    if prefix == b"utf-8" || prefix.starts_with(b"utf-8-") {
        return Codec::Utf8;
    }
    if [b"latin-1".as_slice(), b"iso-8859-1", b"iso-latin-1"]
        .iter()
        .any(|name| {
            prefix == *name
                || prefix
                    .strip_prefix(*name)
                    .is_some_and(|rest| rest.starts_with(b"-"))
        })
    {
        return Codec::Latin1;
    }
    // Common codec aliases, after the standard '-'/'_' normalization.
    if label.len() <= normalized.len() {
        return match prefix {
            b"utf8" | b"utf" | b"u8" | b"cp65001" => Codec::Utf8,
            b"latin1" | b"latin" | b"l1" | b"iso8859-1" | b"cp819" | b"ibm819" | b"8859" => {
                Codec::Latin1
            }
            b"ascii" | b"us-ascii" | b"646" => Codec::Ascii,
            _ => Codec::Unsupported,
        };
    }
    Codec::Unsupported
}

fn python_cookie(bytes: &[u8]) -> Option<&str> {
    let mut lines = bytes.split_inclusive(|byte| *byte == b'\n');
    let first = lines.next()?;
    cookie_line(first).or_else(|| {
        // A second-line declaration is legal only after a blank or comment first line.
        let first = trim_indent(first);
        (first.is_empty() || matches!(first[0], b'#' | b'\r' | b'\n'))
            .then(|| lines.next().and_then(cookie_line))
            .flatten()
    })
}

fn trim_indent(bytes: &[u8]) -> &[u8] {
    &bytes[bytes
        .iter()
        .take_while(|byte| matches!(byte, b' ' | b'\t' | b'\x0c'))
        .count()..]
}

fn cookie_line(line: &[u8]) -> Option<&str> {
    let line = trim_indent(line).strip_prefix(b"#")?;
    for (index, window) in line.windows(7).enumerate() {
        if &window[..6] != b"coding" || !matches!(window[6], b':' | b'=') {
            continue;
        }
        let rest = &line[index + 7..];
        let rest = &rest[rest
            .iter()
            .take_while(|byte| matches!(byte, b' ' | b'\t'))
            .count()..];
        let end = rest
            .iter()
            .take_while(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
            .count();
        if end != 0 {
            return std::str::from_utf8(&rest[..end]).ok();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::DecodedSource;

    #[test]
    fn cookies_apply_before_utf8_and_respect_comment_placement() {
        for bytes in [
            b"# coding: latin-1\nvalue = '\xc3\xa9'\n".as_slice(),
            b"#!/bin/python\n# coding=latin_1\nvalue = '\xc3\xa9'\n",
        ] {
            let decoded = DecodedSource::select(bytes, true).unwrap();
            let text: String = decoded.characters().map(|(_, ch)| ch).collect();
            assert!(text.contains("'Ã©'"));
            assert_eq!(decoded.characters().last().unwrap().0, bytes.len() - 1);
        }
        for bytes in [
            b"value = 'coding: bad'\n".as_slice(),
            b"value = 1\n# coding: bad\n",
            b"# coding : bad\n",
        ] {
            assert!(DecodedSource::select(bytes, true).is_ok());
        }
        for bytes in [
            b"# coding: bad\n".as_slice(),
            b"\n# coding: bad\n",
            b"\xef\xbb\xbf# coding: latin-1\n",
            b"\xef\xbb\xbf# coding: utf8\n",
            b"# coding: ascii\n# \xc3\xa9\n",
            b"# coding: utf-8\n# \xff\n",
        ] {
            assert!(DecodedSource::select(bytes, true).is_err());
        }
        assert!(DecodedSource::select(b"# coding: bad\n", false).is_ok());
        let bom = DecodedSource::select(b"\xef\xbb\xbf# coding: utf_8\n", true).unwrap();
        assert_eq!(bom.characters().next(), Some((3, '#')));
        assert_eq!(bom.original_len(), 19);
    }
}
