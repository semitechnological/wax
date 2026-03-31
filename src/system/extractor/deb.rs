/// Extract a .deb package to dest_dir.
///
/// .deb files are ar(1) archives with the structure:
///   - `debian-binary`   — "2.0\n"
///   - `control.tar.*`   — metadata (skipped)
///   - `data.tar.*`      — the actual file tree (we extract this)
use crate::error::{Result, WaxError};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use tar::Archive;

pub fn extract(path: &Path, dest_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dest_dir)?;
    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);

    // Validate the global ar header
    let mut global = [0u8; 8];
    reader.read_exact(&mut global)?;
    if &global != b"!<arch>\n" {
        return Err(WaxError::InstallError(
            "Not a valid ar archive (missing global header)".to_string(),
        ));
    }

    loop {
        // Each file header is 60 bytes
        let mut header = [0u8; 60];
        match reader.read_exact(&mut header) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        }

        // Filename: bytes 0..16, right-padded with spaces
        let filename_raw = std::str::from_utf8(&header[0..16])
            .map_err(|e| WaxError::ParseError(format!("ar filename: {}", e)))?;
        let filename = filename_raw.trim_end_matches(' ').trim_end_matches('/');

        // File size: bytes 48..58, ASCII decimal, right-padded with spaces
        let size_str = std::str::from_utf8(&header[48..58])
            .map_err(|e| WaxError::ParseError(format!("ar size field: {}", e)))?
            .trim();
        let size: u64 = size_str
            .parse()
            .map_err(|e| WaxError::ParseError(format!("ar size '{}': {}", size_str, e)))?;

        // End magic: bytes 58..60 = "`\n"
        if &header[58..60] != b"`\n" {
            return Err(WaxError::ParseError(
                "ar file header: missing end magic".to_string(),
            ));
        }

        if filename.starts_with("data.tar") {
            // This is the payload we want
            let compression = if filename.ends_with(".gz") {
                "gz"
            } else if filename.ends_with(".xz") {
                "xz"
            } else if filename.ends_with(".zst") {
                "zst"
            } else if filename.ends_with(".bz2") {
                "bz2"
            } else {
                "none"
            };

            extract_data_tar(&mut reader, size, compression, dest_dir)?;
            return Ok(());
        } else {
            // Skip this member; pad to even boundary
            let padded = size + (size & 1);
            reader.seek(SeekFrom::Current(padded as i64))?;
        }
    }

    Err(WaxError::InstallError(
        "data.tar.* member not found in .deb archive".to_string(),
    ))
}

fn extract_data_tar<R: Read>(
    reader: &mut R,
    size: u64,
    compression: &str,
    dest_dir: &Path,
) -> Result<()> {
    // Read exactly `size` bytes into a buffer, then decompress
    let mut buf = vec![0u8; size as usize];
    reader.read_exact(&mut buf)?;

    match compression {
        "gz" => {
            let decoder = flate2::read::GzDecoder::new(&buf[..]);
            untar(decoder, dest_dir)
        }
        "xz" => {
            let decoder = xz2::read::XzDecoder::new(&buf[..]);
            untar(decoder, dest_dir)
        }
        "zst" => {
            let decoder = zstd::Decoder::new(&buf[..])
                .map_err(|e| WaxError::InstallError(format!("zstd decoder error: {}", e)))?;
            untar(decoder, dest_dir)
        }
        "bz2" => {
            let decoder = bzip2::read::BzDecoder::new(&buf[..]);
            untar(decoder, dest_dir)
        }
        _ => {
            // No compression — raw tar
            untar(&buf[..], dest_dir)
        }
    }
}

fn untar<R: Read>(reader: R, dest_dir: &Path) -> Result<()> {
    let mut archive = Archive::new(reader);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let entry_path = entry.path()?;

        // Strip leading "./" and skip ".." entries
        let entry_str = entry_path.to_string_lossy();
        let stripped = if let Some(s) = entry_str.strip_prefix("./") {
            s.to_string()
        } else {
            entry_str.to_string()
        };

        if stripped.is_empty() || stripped.contains("..") {
            continue;
        }

        let dest = dest_dir.join(&stripped);

        if entry.header().entry_type().is_dir() {
            std::fs::create_dir_all(&dest)?;
        } else if entry.header().entry_type().is_symlink() {
            if let Some(link_target) = entry.link_name()? {
                // Remove existing destination if any
                let _ = std::fs::remove_file(&dest);
                let _ = std::fs::remove_dir_all(&dest);
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                #[cfg(unix)]
                std::os::unix::fs::symlink(link_target.as_ref(), &dest)?;
            }
        } else {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            entry.unpack(&dest)?;
        }
    }
    Ok(())
}
