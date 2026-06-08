pub(crate) fn encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

pub(crate) fn decode_prefixed(s: &str) -> anyhow::Result<Vec<u8>> {
    decode(s.strip_prefix("0x").unwrap_or(s))
}

pub(crate) fn decode_prefixed_fixed<const N: usize>(s: &str) -> anyhow::Result<[u8; N]> {
    let bytes = decode_prefixed(s)?;
    bytes
        .try_into()
        .map_err(|bytes: Vec<u8>| anyhow::anyhow!("expected {N} bytes, got {}", bytes.len()))
}

fn decode(s: &str) -> anyhow::Result<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        anyhow::bail!("odd-length hex string");
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = nibble(bytes[i])?;
        let lo = nibble(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

fn nibble(b: u8) -> anyhow::Result<u8> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(10 + (b - b'a')),
        b'A'..=b'F' => Ok(10 + (b - b'A')),
        _ => anyhow::bail!("invalid hex byte 0x{b:02x}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_round_values() {
        assert_eq!(encode(&[]), "");
        assert_eq!(encode(&[0x00]), "00");
        assert_eq!(encode(&[0xff]), "ff");
        assert_eq!(encode(&[0x12, 0x34, 0xab, 0xcd]), "1234abcd");
    }

    #[test]
    fn decode_prefixed_fixed_checks_length() {
        assert_eq!(
            decode_prefixed_fixed::<2>("0x1234").expect("decode"),
            [0x12, 0x34]
        );
        assert!(decode_prefixed_fixed::<3>("0x1234").is_err());
    }
}
