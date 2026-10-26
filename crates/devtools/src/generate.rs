//! Passwords, UUIDs, placeholder text, hashes.

use rand::Rng as _;
use rand::seq::SliceRandom as _;

const LOWER: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
const UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS: &[u8] = b"0123456789";

/// `count` passwords of exactly `len` characters from A–Z, a–z and 0–9 (no symbols), each with at
/// least one of every kind; from the OS's secure random source.
pub fn passwords(len: usize, count: usize) -> String {
    let mut rng = rand::rngs::OsRng;
    let all: Vec<u8> = [LOWER, UPPER, DIGITS].concat();
    (0..count.max(1))
        .map(|_| {
            let mut p: Vec<u8> = [LOWER, UPPER, DIGITS]
                .iter()
                .map(|set| set[rng.gen_range(0..set.len())])
                .collect();
            p.extend((3..len).map(|_| all[rng.gen_range(0..all.len())]));
            p.shuffle(&mut rng);
            String::from_utf8(p).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn uuids(count: usize) -> String {
    (0..count.max(1))
        .map(|_| uuid::Uuid::new_v4().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

const WORDS: &[&str] = &[
    "lorem",
    "ipsum",
    "dolor",
    "sit",
    "amet",
    "consectetur",
    "adipiscing",
    "elit",
    "sed",
    "do",
    "eiusmod",
    "tempor",
    "incididunt",
    "ut",
    "labore",
    "et",
    "dolore",
    "magna",
    "aliqua",
    "enim",
    "ad",
    "minim",
    "veniam",
    "quis",
    "nostrud",
    "exercitation",
    "ullamco",
    "laboris",
    "nisi",
    "aliquip",
    "ex",
    "ea",
    "commodo",
    "consequat",
    "duis",
    "aute",
    "irure",
    "in",
    "reprehenderit",
    "voluptate",
    "velit",
    "esse",
    "cillum",
    "fugiat",
    "nulla",
    "pariatur",
    "excepteur",
    "sint",
    "occaecat",
    "cupidatat",
    "non",
    "proident",
    "sunt",
    "culpa",
    "qui",
    "officia",
    "deserunt",
    "mollit",
    "anim",
    "id",
    "est",
    "laborum",
];

fn sentence(rng: &mut impl rand::Rng) -> String {
    let n = rng.gen_range(6..14);
    let mut words: Vec<String> = (0..n)
        .map(|_| WORDS[rng.gen_range(0..WORDS.len())].to_string())
        .collect();
    // A comma now and then, as real prose has.
    if n > 8 {
        let at = rng.gen_range(3..n - 3);
        words[at].push(',');
    }
    let mut s = words.join(" ");
    if let Some(first) = s.get(0..1) {
        s.replace_range(0..1, &first.to_uppercase());
    }
    s.push('.');
    s
}

/// `mode`: 0 paragraphs, 1 sentences, 2 words. The first one starts "Lorem ipsum dolor sit amet".
pub fn lorem(mode: usize, count: usize) -> String {
    let mut rng = rand::thread_rng();
    let count = count.max(1);
    let opening = "Lorem ipsum dolor sit amet, consectetur adipiscing elit.";
    match mode {
        2 => {
            let mut words: Vec<&str> = vec!["lorem", "ipsum"];
            while words.len() < count {
                words.push(WORDS[rng.gen_range(0..WORDS.len())]);
            }
            words.truncate(count);
            words.join(" ")
        }
        1 => std::iter::once(opening.to_string())
            .chain((1..count).map(|_| sentence(&mut rng)))
            .collect::<Vec<_>>()
            .join(" "),
        _ => (0..count)
            .map(|k| {
                let n = rng.gen_range(4..7);
                let mut s: Vec<String> = (0..n).map(|_| sentence(&mut rng)).collect();
                if k == 0 {
                    s[0] = opening.to_string();
                }
                s.join(" ")
            })
            .collect::<Vec<_>>()
            .join("\n\n"),
    }
}

pub fn hashes(input: &str) -> String {
    use md5::Digest as _;
    let bytes = input.as_bytes();
    let hex = |d: &[u8]| d.iter().map(|b| format!("{b:02x}")).collect::<String>();
    [
        ("MD5", hex(&md5::Md5::digest(bytes))),
        ("SHA-1", hex(&sha1::Sha1::digest(bytes))),
        ("SHA-256", hex(&sha2::Sha256::digest(bytes))),
        ("SHA-384", hex(&sha2::Sha384::digest(bytes))),
        ("SHA-512", hex(&sha2::Sha512::digest(bytes))),
    ]
    .iter()
    .map(|(name, h)| format!("{name:<8} {h}"))
    .collect::<Vec<_>>()
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_shape() {
        for p in passwords(16, 50).lines() {
            assert_eq!(p.len(), 16);
            assert!(p.bytes().all(|b| b.is_ascii_alphanumeric()));
            assert!(p.bytes().any(|b| b.is_ascii_lowercase()));
            assert!(p.bytes().any(|b| b.is_ascii_uppercase()));
            assert!(p.bytes().any(|b| b.is_ascii_digit()));
        }
    }

    #[test]
    fn others() {
        assert_eq!(uuids(3).lines().count(), 3);
        assert_eq!(lorem(2, 5).split(' ').count(), 5);
        assert!(lorem(0, 2).starts_with("Lorem ipsum dolor sit amet"));
        assert!(hashes("abc").contains("900150983cd24fb0d6963f7d28e17f72"));
    }
}
