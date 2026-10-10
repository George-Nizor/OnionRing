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

/// Seconds east of UTC for this PC's time zone right now (0 when unknown). Daylight saving is
/// taken into account for the current moment only.
pub fn local_offset() -> i64 {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct SystemTimeW([u16; 8]);
        /// TIME_ZONE_INFORMATION
        #[repr(C)]
        struct TimeZoneInformation {
            bias: i32,
            standard_name: [u16; 32],
            standard_date: SystemTimeW,
            standard_bias: i32,
            daylight_name: [u16; 32],
            daylight_date: SystemTimeW,
            daylight_bias: i32,
        }
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetTimeZoneInformation(info: *mut TimeZoneInformation) -> u32;
        }
        let mut info = std::mem::MaybeUninit::<TimeZoneInformation>::zeroed();
        // SAFETY: the struct has TIME_ZONE_INFORMATION's layout; the call only fills it in.
        let (mode, info) = unsafe {
            let mode = GetTimeZoneInformation(info.as_mut_ptr());
            (mode, info.assume_init())
        };
        let extra = match mode {
            0 => 0,                   // no daylight saving
            1 => info.standard_bias,  // standard time
            2 => info.daylight_bias,  // daylight saving time
            _ => return 0,            // failed
        };
        -((info.bias + extra) as i64) * 60
    }
    #[cfg(not(windows))]
    {
        0
    }
}

/// Calendar date and time of a Unix time in a zone `offset` seconds east of UTC:
/// (year, month, day, hour, minute).
pub fn civil(t: i64, offset: i64) -> (i64, u32, u32, u32, u32) {
    let t = t + offset;
    let (days, secs) = (t.div_euclid(86_400), t.rem_euclid(86_400));
    // Howard Hinnant's days -> civil date.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month as u32, day as u32, (secs / 3600) as u32, (secs % 3600 / 60) as u32)
}

/// Start of the week holding `t` (Monday 00:00 in a zone `offset` seconds east of UTC), as a
/// Unix time.
pub fn week_start(t: i64, offset: i64) -> i64 {
    const WEEK: i64 = 7 * 86_400;
    // The epoch was a Thursday; 1970-01-05 was the first Monday.
    const MONDAY: i64 = 4 * 86_400;
    (t + offset - MONDAY).div_euclid(WEEK) * WEEK + MONDAY - offset
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

/// A scratch folder under the system temp folder, removed when dropped (tests only).
#[cfg(test)]
pub(crate) struct TempDir(pub std::path::PathBuf);

#[cfg(test)]
impl TempDir {
    pub fn new(tag: &str) -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let n = N.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("onionring-test-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    /// Write `bytes` zero bytes to `rel`, creating folders on the way.
    pub fn file(&self, rel: &str, bytes: usize) -> std::path::PathBuf {
        self.write(rel, &vec![0u8; bytes])
    }

    pub fn write(&self, rel: &str, content: &[u8]) -> std::path::PathBuf {
        let p = self.0.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, content).unwrap();
        p
    }
}

#[cfg(test)]
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
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
    fn dates() {
        assert_eq!(civil(0, 0), (1970, 1, 1, 0, 0));
        assert_eq!(civil(951_782_400, 0), (2000, 2, 29, 0, 0));
        // 2024-01-01 00:00 UTC is 2023-12-31 19:00 five hours west.
        assert_eq!(civil(1_704_067_200, -5 * 3600), (2023, 12, 31, 19, 0));
        assert_eq!(civil(1_704_067_200 + 15 * 3600 + 32 * 60, 0), (2024, 1, 1, 15, 32));
    }

    #[test]
    fn weeks_start_on_monday() {
        let monday = 1_704_067_200; // 2024-01-01 00:00 UTC, a Monday
        assert_eq!(week_start(monday, 0), monday);
        assert_eq!(week_start(monday + 6 * 86_400 + 86_399, 0), monday);
        assert_eq!(week_start(monday - 1, 0), monday - 7 * 86_400);
        // One hour east of UTC, local Monday midnight is 23:00 UTC on Sunday.
        assert_eq!(week_start(monday, 3600), monday - 3600);
        assert!(local_offset().abs() <= 14 * 3600);
    }

    #[test]
    fn split() {
        let (s, e) = split_name(Path::new("Star Fox Adventures (USA) (Rev 1).nkit.iso"));
        assert_eq!((s.as_str(), e.as_str()), ("Star Fox Adventures (USA) (Rev 1)", ".iso"));
        assert_eq!(region_of("Pokemon Diamond (v05) (U)(Legacy)").as_deref(), Some("USA"));
    }
}
