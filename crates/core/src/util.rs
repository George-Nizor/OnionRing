use regex::Regex;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;
use std::sync::LazyLock;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

static TAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*[\(\[][^\)\]]*[\)\]]").unwrap());
static LEAD_NUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{3,5}\s*-\s*").unwrap());
static NOISE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(decrypted|encrypted|trimmed|nkit)\b").unwrap());
static ARTICLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?P<a>.+?),\s*(?P<art>The|A|An)(?P<rest>\s+-\s+.*)?$").unwrap());
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s{2,}").unwrap());
static REGION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\((usa|us|u|europe|eur|e|japan|jpn|j|world|w|australia|asia|korea|uk|germany|france|spain|italy)(?:[,\s][^)]*)?\)")
        .unwrap()
});

/// Split a file name into a stem and a lowercase extension, treating `.nkit.iso`-style
/// double extensions as part of the extension so titles stay clean.
pub fn split_name(path: &Path) -> (String, String) {
    let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let lower = name.to_lowercase();
    for double in [".nkit.iso", ".nkit.gcz", ".iso.gz"] {
        if lower.ends_with(double) {
            let stem = &name[..name.len() - double.len()];
            let ext = &double[double.rfind('.').unwrap()..];
            return (stem.to_string(), ext.to_string());
        }
    }
    match name.rfind('.') {
        Some(i) if i > 0 => (name[..i].to_string(), lower[i..].to_string()),
        _ => (name, String::new()),
    }
}

/// Turn a No-Intro/Redump style name into a display title:
/// "Legend of Zelda, The - The Wind Waker (USA)" -> "The Legend of Zelda: The Wind Waker".
pub fn clean_title(raw: &str) -> String {
    let mut s = raw.replace('_', " ");
    s = LEAD_NUM.replace(&s, "").to_string();
    s = TAGS.replace_all(&s, "").to_string();
    s = NOISE.replace_all(&s, "").to_string();
    s = s.replace(['®', '™', '©'], "");
    s = SPACES.replace_all(s.trim(), " ").to_string();
    if let Some(c) = ARTICLE.captures(&s) {
        let rest = c.name("rest").map(|m| m.as_str()).unwrap_or("");
        s = format!("{} {}{}", &c["art"], &c["a"], rest);
    }
    s = s.replace(" - ", ": ");
    let t = s.trim().trim_end_matches(['-', ':', ',', '.']).trim().to_string();
    if t.is_empty() { raw.to_string() } else { t }
}

/// "GOD OF WAR III" -> "God of War III". Leaves mixed-case titles alone.
pub fn tidy_case(title: &str) -> String {
    let letters: Vec<char> = title.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() < 4 || letters.iter().any(|c| c.is_lowercase()) {
        return title.to_string();
    }
    let small = ["of", "the", "and", "a", "an", "in", "on", "to", "for", "at", "by", "or"];
    let roman = Regex::new(r"^(?i)[ivxlcdm]+$").unwrap();
    title
        .split(' ')
        .enumerate()
        .map(|(i, w)| {
            let lw = w.to_lowercase();
            if roman.is_match(w) && w.len() <= 4 {
                w.to_string()
            } else if i > 0 && small.contains(&lw.as_str()) {
                lw
            } else {
                let mut c = lw.chars();
                c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Key used to recognise the same game across copies: lowercase alphanumerics, articles dropped.
pub fn norm_key(title: &str) -> String {
    let cleaned = clean_title(title).to_lowercase();
    let mut out = Vec::new();
    for tok in cleaned.split(|c: char| !c.is_alphanumeric()) {
        if tok.is_empty() || tok == "the" || tok == "a" || tok == "an" {
            continue;
        }
        out.push(tok);
    }
    out.join(" ")
}

pub fn region_of(raw: &str) -> Option<String> {
    let c = REGION.captures(raw)?;
    let r = c.get(1)?.as_str().to_lowercase();
    Some(
        match r.as_str() {
            "usa" | "us" | "u" => "USA",
            "europe" | "eur" | "e" | "uk" | "germany" | "france" | "spain" | "italy" => "EUR",
            "japan" | "jpn" | "j" => "JPN",
            "australia" => "AUS",
            "asia" => "ASIA",
            "korea" => "KOR",
            _ => "WORLD",
        }
        .to_string(),
    )
}

pub fn sha256_file(path: &Path) -> anyhow::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

pub fn short_hash(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    hex::encode(&h.finalize()[..8])
}

pub fn dir_size(path: &Path) -> u64 {
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

pub fn path_str(p: &Path) -> String {
    p.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles() {
        assert_eq!(
            clean_title("Legend of Zelda, The - The Wind Waker (USA)"),
            "The Legend of Zelda: The Wind Waker"
        );
        assert_eq!(clean_title("Matrix, The - Path of Neo (Europe, Australia) (En,Fr,De,Es)"), "The Matrix: Path of Neo");
        assert_eq!(clean_title("1015 - Pokemon Diamond (v05) (U)(Legacy)"), "Pokemon Diamond");
        assert_eq!(clean_title("Rayman Origins (USA) (En,Fr,Es) Decrypted"), "Rayman Origins");
        assert_eq!(clean_title("GOD OF WAR® III"), "GOD OF WAR III");
    }

    #[test]
    fn keys_match_across_copies() {
        assert_eq!(norm_key("Matrix The - Path of Neo (Europe Australia) (EnFrDeEs)"), norm_key("Matrix, The - Path of Neo (Europe, Australia)"));
        assert_eq!(norm_key("God of War III (USA) (v02.00)"), norm_key("GOD OF WAR® III"));
        assert_eq!(norm_key("Skate 3 (USA, Asia) (En,Fr,Es)"), "skate 3");
    }

    #[test]
    fn split() {
        let (s, e) = split_name(Path::new("Star Fox Adventures (USA) (Rev 1).nkit.iso"));
        assert_eq!((s.as_str(), e.as_str()), ("Star Fox Adventures (USA) (Rev 1)", ".iso"));
        assert_eq!(region_of("Pokemon Diamond (v05) (U)(Legacy)").as_deref(), Some("USA"));
    }
}
