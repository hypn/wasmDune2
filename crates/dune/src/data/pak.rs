//! PAK archives and a simple virtual file system over all loaded archives.

use std::collections::HashMap;

/// Parse a PAK archive into (NAME, bytes) entries.
pub fn parse(data: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut entries: Vec<(String, usize)> = Vec::new();
    let mut p = 0usize;
    loop {
        if p + 4 > data.len() {
            return Err("truncated PAK header".into());
        }
        let off = u32::from_le_bytes(data[p..p + 4].try_into().unwrap()) as usize;
        if off == 0 {
            break;
        }
        let name_start = p + 4;
        let name_end = data[name_start..]
            .iter()
            .position(|&b| b == 0)
            .map(|e| name_start + e)
            .ok_or("unterminated PAK name")?;
        let name = String::from_utf8_lossy(&data[name_start..name_end]).to_ascii_uppercase();
        entries.push((name, off));
        p = name_end + 1;
    }
    let mut out = Vec::with_capacity(entries.len());
    for i in 0..entries.len() {
        let start = entries[i].1;
        // Some shipped PAKs (e.g. the demo's INTROVOC.PAK) are truncated: clamp, skip if empty.
        let end = entries.get(i + 1).map(|e| e.1).unwrap_or(data.len()).min(data.len());
        if start >= end {
            continue;
        }
        out.push((entries[i].0.clone(), data[start..end].to_vec()));
    }
    Ok(out)
}

/// All files from all loaded PAKs (and loose files), looked up case-insensitively.
#[derive(Default)]
pub struct Vfs {
    files: HashMap<String, Vec<u8>>,
    paks: Vec<String>,
}

impl Vfs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_pak(&mut self, pak_name: &str, data: &[u8]) -> Result<(), String> {
        for (name, bytes) in parse(data)? {
            self.files.insert(name, bytes);
        }
        self.paks.push(pak_name.to_ascii_uppercase());
        Ok(())
    }

    pub fn add_file(&mut self, name: &str, data: Vec<u8>) {
        self.files.insert(name.to_ascii_uppercase(), data);
    }

    pub fn get(&self, name: &str) -> Option<&[u8]> {
        self.files.get(&name.to_ascii_uppercase()).map(|v| v.as_slice())
    }

    pub fn exists(&self, name: &str) -> bool {
        self.files.contains_key(&name.to_ascii_uppercase())
    }

    pub fn has_pak(&self, pak_name: &str) -> bool {
        self.paks.iter().any(|p| p.eq_ignore_ascii_case(pak_name))
    }

    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.files.keys()
    }
}
