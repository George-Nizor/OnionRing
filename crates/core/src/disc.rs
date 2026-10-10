//! Tiny readers for the few on-disc headers we need: PARAM.SFO (PS3/PS4) and
//! GameCube/Wii disc headers (plain ISO, RVZ/WIA, WBFS).

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Default, Clone)]
pub struct Sfo {
    pub values: HashMap<String, String>,
}

impl Sfo {
    pub fn get(&self, k: &str) -> Option<&str> {
        self.values.get(k).map(|s| s.as_str())
    }
}

fn u16le(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(o..o + 2)?.try_into().ok()?))
}
fn u32le(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}
fn u32be(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

pub fn read_sfo(path: &Path) -> Option<Sfo> {
    let data = std::fs::read(path).ok()?;
    if data.get(0..4)? != b"\0PSF" {
        return None;
    }
    let key_table = u32le(&data, 8)? as usize;
    let data_table = u32le(&data, 12)? as usize;
    let count = u32le(&data, 16)? as usize;
    let mut sfo = Sfo::default();
    for i in 0..count.min(256) {
        let e = 20 + i * 16;
        let key_off = u16le(&data, e)? as usize;
        let fmt = u16le(&data, e + 2)?;
        let len = u32le(&data, e + 4)? as usize;
        let off = u32le(&data, e + 12)? as usize;
        let ks = key_table + key_off;
        let kend = data.get(ks..)?.iter().position(|&c| c == 0)? + ks;
        let key = String::from_utf8_lossy(&data[ks..kend]).to_string();
        let ds = data_table + off;
        let raw = data.get(ds..ds + len)?;
        let value = match fmt {
            0x0204 | 0x0004 => String::from_utf8_lossy(raw).trim_end_matches('\0').trim().to_string(),
            0x0404 => u32le(raw, 0).map(|v| v.to_string()).unwrap_or_default(),
            _ => continue,
        };
        sfo.values.insert(key, value);
    }
    Some(sfo)
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum NintendoDisc {
    GameCube,
    Wii,
}

/// Identify a GameCube or Wii image and return its 6-character game ID when readable.
pub fn nintendo_disc(path: &Path) -> Option<(NintendoDisc, Option<String>)> {
    let mut f = File::open(path).ok()?;
    let mut head = [0u8; 0x100];
    let n = f.read(&mut head).ok()?;
    if n < 0x60 {
        return None;
    }
    let id_at = |b: &[u8], o: usize| -> Option<String> {
        let s = b.get(o..o + 6)?;
        s.iter().all(|c| c.is_ascii_alphanumeric()).then(|| String::from_utf8_lossy(s).to_string())
    };
    match &head[0..4] {
        b"WBFS" => {
            // Disc header sits at the first hd sector: 1 << head[8].
            let sec = 1u64 << head[8].min(20);
            let mut d = [0u8; 0x20];
            f.seek(SeekFrom::Start(sec)).ok()?;
            f.read_exact(&mut d).ok()?;
            Some((NintendoDisc::Wii, id_at(&d, 0)))
        }
        b"RVZ\x01" | b"WIA\x01" => {
            let kind = match u32be(&head, 0x48)? {
                1 => NintendoDisc::GameCube,
                2 => NintendoDisc::Wii,
                _ => return None,
            };
            Some((kind, id_at(&head, 0x58)))
        }
        _ => {
            if u32be(&head, 0x18)? == 0x5D1C_9EA3 {
                Some((NintendoDisc::Wii, id_at(&head, 0)))
            } else if u32be(&head, 0x1C)? == 0xC233_9F3D {
                Some((NintendoDisc::GameCube, id_at(&head, 0)))
            } else {
                None
            }
        }
    }
}
