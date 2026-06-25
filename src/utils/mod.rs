/// Read a file that may be UTF-16LE (with BOM) or UTF-8, returning a UTF-8 String.
/// MT5 .set and .ini files are typically UTF-16LE with BOM (0xFF 0xFE).
pub fn read_file_as_utf8(path: &std::path::Path) -> anyhow::Result<String> {
    let bytes = std::fs::read(path)?;
    
    // Check for UTF-16LE BOM (0xFF 0xFE)
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        // UTF-16LE with BOM - skip the 2-byte BOM and decode
        let utf16_data: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        String::from_utf16(&utf16_data)
            .map_err(|e| anyhow::anyhow!("Failed to decode UTF-16LE: {}", e))
    } else {
        // Try UTF-8
        String::from_utf8(bytes)
            .map_err(|e| anyhow::anyhow!("Failed to decode as UTF-8: {}", e))
    }
}
