const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub(super) fn osc52(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", encode(text.as_bytes()))
}

pub(super) fn encode(input: &[u8]) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let held = u32::from(chunk[0]) << 16
            | u32::from(chunk.get(1).copied().unwrap_or_default()) << 8
            | u32::from(chunk.get(2).copied().unwrap_or_default());
        for index in 0..4 {
            if index > chunk.len() {
                out.push('=');
            } else {
                let shift = 18 - index * 6;
                out.push(TABLE[(held >> shift & 0x3f) as usize] as char);
            }
        }
    }
    out
}
