//! Encodings and checksums behind the address checks, written here to keep
//! the `crypto` feature small: Base58 / Base58Check (Bitcoin wiki
//! "Base58Check encoding"), bech32 / bech32m (BIP-173, BIP-350), CashAddr
//! (Bitcoin Cash "cashaddr" spec), Monero's block-wise Base58, CRC16-XMODEM,
//! CRC32 (IEEE), hex and base64. Hashes come from RustCrypto (`sha2`, `sha3`,
//! `blake2`).

use crate::error::CryptoAddressReason as Reason;

/// Bitcoin's Base58 alphabet (also Solana, TRON, Polkadot SS58).
pub(super) const BTC_ALPHABET: &[u8; 58] =
    b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
/// Ripple's Base58 alphabet (XRP Ledger).
pub(super) const RIPPLE_ALPHABET: &[u8; 58] =
    b"rpshnaf39wBUDNEGHJKLM4PQRST7VWXYZ2bcdeCg65jkm8oFqi1tuvAxyz";
/// Flickr's Base58 alphabet (lowercase before uppercase).
pub(super) const FLICKR_ALPHABET: &[u8; 58] =
    b"123456789abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ";

// ───────────────────────────── hashes ─────────────────────────────

pub(super) fn sha256d(data: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut out = [0u8; 32];
    out.copy_from_slice(&Sha256::digest(Sha256::digest(data)));
    out
}

pub(super) fn keccak256(data: &[u8]) -> [u8; 32] {
    use sha3::{Digest, Keccak256};
    let mut out = [0u8; 32];
    out.copy_from_slice(&Keccak256::digest(data));
    out
}

/// First two bytes of `blake2b-512("SS58PRE" ++ data)` (SS58 spec, 32/33-byte
/// account ids).
pub(super) fn ss58_checksum(data: &[u8]) -> [u8; 2] {
    use blake2::{Blake2b512, Digest};
    let mut h = Blake2b512::new();
    h.update(b"SS58PRE");
    h.update(data);
    let d = h.finalize();
    [d[0], d[1]]
}

/// CRC16-XMODEM (poly 0x1021, init 0), as TON user-friendly addresses use.
pub(super) fn crc16_xmodem(data: &[u8]) -> u16 {
    let mut crc: u16 = 0;
    for &b in data {
        crc ^= u16::from(b) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}

/// CRC-32 (IEEE 802.3, reflected, poly 0xEDB88320).
pub(super) fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

// ───────────────────────────── base58 ─────────────────────────────

/// Plain Base58: big-endian base-58 number, each leading `alphabet[0]` is a
/// zero byte. `None` on a character outside the alphabet or input over 512
/// characters.
pub(super) fn base58_decode(s: &str, alphabet: &[u8; 58]) -> Option<Vec<u8>> {
    // Quadratic in the length: no address is anywhere near this long.
    if s.len() > 512 {
        return None;
    }
    let mut map = [0xffu8; 128];
    for (i, &c) in alphabet.iter().enumerate() {
        map[c as usize] = i as u8;
    }
    // Little-endian base-256 digits.
    let mut out: Vec<u8> = Vec::with_capacity(s.len());
    for c in s.bytes() {
        let d = *map.get(c as usize)?;
        if d == 0xff {
            return None;
        }
        let mut carry = u32::from(d);
        for b in out.iter_mut() {
            carry += u32::from(*b) * 58;
            *b = carry as u8;
            carry >>= 8;
        }
        while carry > 0 {
            out.push(carry as u8);
            carry >>= 8;
        }
    }
    let zeros = s.bytes().take_while(|&c| c == alphabet[0]).count();
    out.extend(std::iter::repeat_n(0, zeros));
    out.reverse();
    Some(out)
}

pub(super) fn base58_encode(data: &[u8], alphabet: &[u8; 58]) -> String {
    let zeros = data.iter().take_while(|&&b| b == 0).count();
    // Little-endian base-58 digits.
    let mut digits: Vec<u8> = Vec::with_capacity(data.len() * 138 / 100 + 1);
    for &b in data {
        let mut carry = u32::from(b);
        for d in digits.iter_mut() {
            carry += u32::from(*d) << 8;
            *d = (carry % 58) as u8;
            carry /= 58;
        }
        while carry > 0 {
            digits.push((carry % 58) as u8);
            carry /= 58;
        }
    }
    let mut s = String::with_capacity(zeros + digits.len());
    s.extend(std::iter::repeat_n(alphabet[0] as char, zeros));
    s.extend(digits.iter().rev().map(|&d| alphabet[d as usize] as char));
    s
}

/// Base58Check: the decoded bytes minus the trailing 4-byte
/// `sha256(sha256(body))` checksum.
pub(super) fn base58check_decode(s: &str, alphabet: &[u8; 58]) -> Result<Vec<u8>, Reason> {
    let mut raw = base58_decode(s, alphabet).ok_or(Reason::Encoding)?;
    if raw.len() < 5 {
        return Err(Reason::Length);
    }
    let body = raw.len() - 4;
    if sha256d(&raw[..body])[..4] != raw[body..] {
        return Err(Reason::Checksum);
    }
    raw.truncate(body);
    Ok(raw)
}

pub(super) fn base58check_encode(body: &[u8], alphabet: &[u8; 58]) -> String {
    let mut raw = body.to_vec();
    raw.extend_from_slice(&sha256d(body)[..4]);
    base58_encode(&raw, alphabet)
}

// ───────────────────────────── bech32 ─────────────────────────────

const CHARSET: &[u8; 32] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
const BECH32M_CONST: u32 = 0x2bc8_30a3;

/// bech32 (BIP-173) or bech32m (BIP-350) checksum constant.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Variant {
    Bech32,
    Bech32m,
}

impl Variant {
    fn constant(self) -> u32 {
        match self {
            Variant::Bech32 => 1,
            Variant::Bech32m => BECH32M_CONST,
        }
    }
}

fn charset_value(c: u8) -> Option<u8> {
    CHARSET.iter().position(|&x| x == c).map(|p| p as u8)
}

fn polymod(values: impl IntoIterator<Item = u8>) -> u32 {
    const GEN: [u32; 5] = [
        0x3b6a_57b2,
        0x2650_8e6d,
        0x1ea1_19fa,
        0x3d42_33dd,
        0x2a14_62b3,
    ];
    let mut chk: u32 = 1;
    for v in values {
        let top = chk >> 25;
        chk = ((chk & 0x01ff_ffff) << 5) ^ u32::from(v);
        for (i, g) in GEN.iter().enumerate() {
            if (top >> i) & 1 == 1 {
                chk ^= g;
            }
        }
    }
    chk
}

fn hrp_expand(hrp: &str) -> impl Iterator<Item = u8> + '_ {
    hrp.bytes()
        .map(|b| b >> 5)
        .chain(std::iter::once(0))
        .chain(hrp.bytes().map(|b| b & 31))
}

/// A decoded bech32 string: lowercase hrp, 5-bit data without the checksum.
pub(super) struct Bech32 {
    pub(super) hrp: String,
    pub(super) data: Vec<u8>,
    pub(super) variant: Variant,
}

/// Decode a bech32 / bech32m string. `max_len` is BIP-173's 90-character
/// limit (`None` for CIP-19 Cardano addresses, which may be longer).
pub(super) fn bech32_decode(s: &str, max_len: Option<usize>) -> Result<Bech32, Reason> {
    if !s.bytes().all(|b| (33..=126).contains(&b)) {
        return Err(Reason::Encoding);
    }
    if s.bytes().any(|b| b.is_ascii_lowercase()) && s.bytes().any(|b| b.is_ascii_uppercase()) {
        return Err(Reason::Case);
    }
    let s = s.to_ascii_lowercase();
    let pos = s.rfind('1').ok_or(Reason::Encoding)?;
    if pos == 0 {
        return Err(Reason::Prefix);
    }
    if s.len() - pos - 1 < 6 || max_len.is_some_and(|m| s.len() > m) {
        return Err(Reason::Length);
    }
    let (hrp, rest) = (&s[..pos], &s[pos + 1..]);
    let values = rest
        .bytes()
        .map(charset_value)
        .collect::<Option<Vec<u8>>>()
        .ok_or(Reason::Encoding)?;
    let variant = match polymod(hrp_expand(hrp).chain(values.iter().copied())) {
        1 => Variant::Bech32,
        BECH32M_CONST => Variant::Bech32m,
        _ => return Err(Reason::Checksum),
    };
    let data = values[..values.len() - 6].to_vec();
    Ok(Bech32 {
        hrp: hrp.to_string(),
        data,
        variant,
    })
}

pub(super) fn bech32_encode(hrp: &str, data: &[u8], variant: Variant) -> String {
    let pm =
        polymod(hrp_expand(hrp).chain(data.iter().copied()).chain([0u8; 6])) ^ variant.constant();
    let mut s = String::with_capacity(hrp.len() + 1 + data.len() + 6);
    s.push_str(hrp);
    s.push('1');
    s.extend(data.iter().map(|&d| CHARSET[d as usize] as char));
    s.extend((0..6).map(|i| CHARSET[((pm >> (5 * (5 - i))) & 31) as usize] as char));
    s
}

/// Regroup bits (`8 -> 5` with `pad`, `5 -> 8` without: leftover bits must be
/// fewer than `from` and zero).
pub(super) fn convert_bits(data: &[u8], from: u32, to: u32, pad: bool) -> Option<Vec<u8>> {
    let (mut acc, mut bits) = (0u32, 0u32);
    let maxv = (1u32 << to) - 1;
    let mut out = Vec::with_capacity(data.len() * from as usize / to as usize + 1);
    for &v in data {
        if u32::from(v) >> from != 0 {
            return None;
        }
        acc = (acc << from) | u32::from(v);
        bits += from;
        while bits >= to {
            bits -= to;
            out.push(((acc >> bits) & maxv) as u8);
        }
    }
    if pad {
        if bits > 0 {
            out.push(((acc << (to - bits)) & maxv) as u8);
        }
    } else if bits >= from || (acc << (to - bits)) & maxv != 0 {
        return None;
    }
    Some(out)
}

// ──────────────────────────── CashAddr ────────────────────────────

fn cash_polymod(values: impl IntoIterator<Item = u8>) -> u64 {
    let mut c: u64 = 1;
    for d in values {
        let c0 = (c >> 35) as u8;
        c = ((c & 0x07_ffff_ffff) << 5) ^ u64::from(d);
        for (i, g) in [
            0x98_f2bc_8e61u64,
            0x79_b76d_99e2,
            0xf3_3e5f_b3c4,
            0xae_2eab_e2a8,
            0x1e_4f43_e470,
        ]
        .iter()
        .enumerate()
        {
            if (c0 >> i) & 1 == 1 {
                c ^= g;
            }
        }
    }
    c ^ 1
}

fn cash_prefix(prefix: &str) -> impl Iterator<Item = u8> + '_ {
    prefix.bytes().map(|b| b & 31).chain(std::iter::once(0))
}

/// Decode a CashAddr: `(prefix, version byte ++ hash)`. Without an explicit
/// `prefix:` every candidate is tried (the checksum covers the prefix).
pub(super) fn cashaddr_decode(s: &str, prefixes: &[&str]) -> Result<(String, Vec<u8>), Reason> {
    if s.bytes().any(|b| b.is_ascii_lowercase()) && s.bytes().any(|b| b.is_ascii_uppercase()) {
        return Err(Reason::Case);
    }
    let s = s.to_ascii_lowercase();
    let (candidates, body): (Vec<&str>, &str) = match s.split_once(':') {
        Some((p, body)) if prefixes.contains(&p) => (vec![p], body),
        Some(_) => return Err(Reason::Prefix),
        None => (prefixes.to_vec(), s.as_str()),
    };
    let values = body
        .bytes()
        .map(charset_value)
        .collect::<Option<Vec<u8>>>()
        .ok_or(Reason::Encoding)?;
    // version byte + 20-byte hash + 40-bit checksum = 42 characters at least.
    if values.len() < 42 {
        return Err(Reason::Length);
    }
    let prefix = candidates
        .into_iter()
        .find(|p| cash_polymod(cash_prefix(p).chain(values.iter().copied())) == 0)
        .ok_or(Reason::Checksum)?;
    let payload = convert_bits(&values[..values.len() - 8], 5, 8, false).ok_or(Reason::Encoding)?;
    Ok((prefix.to_string(), payload))
}

pub(super) fn cashaddr_encode(prefix: &str, payload: &[u8]) -> String {
    let data = convert_bits(payload, 8, 5, true).unwrap_or_default();
    let m = cash_polymod(
        cash_prefix(prefix)
            .chain(data.iter().copied())
            .chain([0u8; 8]),
    );
    let mut s = format!("{prefix}:");
    s.extend(data.iter().map(|&d| CHARSET[d as usize] as char));
    s.extend((0..8).map(|i| CHARSET[((m >> (5 * (7 - i))) & 31) as usize] as char));
    s
}

// ─────────────────────────── Monero base58 ───────────────────────────

/// Encoded size of a block of `n` bytes (Monero's `encoded_block_sizes`).
const MONERO_BLOCK: [usize; 9] = [0, 2, 3, 5, 6, 7, 9, 10, 11];

/// Monero Base58: 8-byte blocks encoded as 11 characters each (the last,
/// shorter block per [`MONERO_BLOCK`]), each block a big-endian number.
pub(super) fn monero_decode(s: &str) -> Option<Vec<u8>> {
    let mut map = [0xffu8; 128];
    for (i, &c) in BTC_ALPHABET.iter().enumerate() {
        map[c as usize] = i as u8;
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() * 8 / 11 + 8);
    for chunk in bytes.chunks(11) {
        let size = MONERO_BLOCK.iter().position(|&n| n == chunk.len())?;
        if size == 0 {
            return None;
        }
        let mut num: u64 = 0;
        for &c in chunk {
            let d = *map.get(c as usize)?;
            if d == 0xff {
                return None;
            }
            num = num.checked_mul(58)?.checked_add(u64::from(d))?;
        }
        if size < 8 && num >> (8 * size) != 0 {
            return None;
        }
        out.extend_from_slice(&num.to_be_bytes()[8 - size..]);
    }
    Some(out)
}

pub(super) fn monero_encode(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len() / 8 * 11 + 11);
    for chunk in data.chunks(8) {
        let mut num = chunk.iter().fold(0u64, |n, &b| (n << 8) | u64::from(b));
        let mut block = vec![BTC_ALPHABET[0]; MONERO_BLOCK[chunk.len()]];
        for c in block.iter_mut().rev() {
            *c = BTC_ALPHABET[(num % 58) as usize];
            num /= 58;
        }
        s.extend(block.into_iter().map(char::from));
    }
    s
}

// ─────────────────────────── hex / base64 ───────────────────────────

pub(super) fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    s.as_bytes()
        .chunks(2)
        .map(|p| {
            let h = (p[0] as char).to_digit(16)?;
            let l = (p[1] as char).to_digit(16)?;
            Some((h * 16 + l) as u8)
        })
        .collect()
}

pub(super) fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}

/// Base64 without padding, standard (`+/`) or URL (`-_`) alphabet.
pub(super) fn base64_encode(data: &[u8], url: bool) -> String {
    let alphabet: &[u8; 64] = if url {
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
    } else {
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
    };
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
        for i in 0..=chunk.len() {
            s.push(alphabet[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    s
}

/// Base64 in either alphabet (not mixed), padding optional, canonical only
/// (unused trailing bits zero), so one address has one spelling.
pub(super) fn base64_decode(s: &str, url: bool) -> Option<Vec<u8>> {
    let bytes = if url {
        crate::primitives::bytes::decode_base64_url(s)?
    } else {
        let mut padded = s.to_string();
        while padded.len() % 4 != 0 {
            padded.push('=');
        }
        crate::primitives::bytes::decode_base64(&padded)?
    };
    (base64_encode(&bytes, url) == s.trim_end_matches('=')).then_some(bytes)
}
