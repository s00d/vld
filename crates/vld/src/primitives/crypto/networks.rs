//! The built-in networks: decoders and random valid samples.

use super::codec::{self, Variant};
use super::{AddressKind as K, CryptoAddressReason as R, Decoded, Network};

type Rnd<'a> = &'a mut dyn FnMut() -> u8;

fn bytes(n: usize, rnd: Rnd<'_>) -> Vec<u8> {
    (0..n).map(|_| rnd()).collect()
}

fn decoded(
    kind: K,
    testnet: impl Into<Option<bool>>,
    normalized: String,
    payload: Vec<u8>,
) -> Decoded {
    Decoded {
        kind,
        testnet: testnet.into(),
        normalized,
        payload,
    }
}

// ─────────────────────── Bitcoin family (Base58Check + SegWit) ───────────────────────

/// `(version byte, kind, testnet)` of a Base58Check family.
type Versions = &'static [(u8, K, bool)];

const BTC_B58: Versions = &[
    (0x00, K::P2pkh, false),
    (0x05, K::P2sh, false),
    (0x6f, K::P2pkh, true),
    (0xc4, K::P2sh, true),
];
const LTC_B58: Versions = &[
    (0x30, K::P2pkh, false),
    (0x32, K::P2sh, false),
    (0x05, K::P2sh, false),
    (0x6f, K::P2pkh, true),
    (0x3a, K::P2sh, true),
    (0xc4, K::P2sh, true),
];
const DOGE_B58: Versions = &[
    (0x1e, K::P2pkh, false),
    (0x16, K::P2sh, false),
    (0x71, K::P2pkh, true),
    (0xc4, K::P2sh, true),
];

/// `(hrp, testnet)` of a SegWit family.
type Hrps = &'static [(&'static str, bool)];
const BTC_HRP: Hrps = &[("bc", false), ("tb", true)];
const LTC_HRP: Hrps = &[("ltc", false), ("tltc", true)];

/// Base58Check with a 1-byte version and a 20-byte hash (Bitcoin wiki
/// "List of address prefixes").
fn base58_versioned(s: &str, versions: Versions) -> Result<Decoded, R> {
    if !(25..=35).contains(&s.len()) {
        return Err(R::Length);
    }
    let body = codec::base58check_decode(s, codec::BTC_ALPHABET)?;
    if body.len() != 21 {
        return Err(R::Length);
    }
    let &(_, kind, testnet) = versions
        .iter()
        .find(|(v, _, _)| *v == body[0])
        .ok_or(R::Prefix)?;
    Ok(decoded(kind, testnet, s.to_string(), body[1..].to_vec()))
}

/// SegWit address (BIP-173 / BIP-350): witness version 0..=16, program 2..=40
/// bytes (v0: 20 or 32), bech32 for v0 and bech32m for v1+.
fn segwit(s: &str, hrps: Hrps) -> Result<Decoded, R> {
    let b = codec::bech32_decode(s, Some(90))?;
    let &(_, testnet) = hrps.iter().find(|(h, _)| *h == b.hrp).ok_or(R::Prefix)?;
    let (&version, rest) = b.data.split_first().ok_or(R::Length)?;
    if version > 16 {
        return Err(R::Encoding);
    }
    let program = codec::convert_bits(rest, 5, 8, false).ok_or(R::Encoding)?;
    if !(2..=40).contains(&program.len()) || (version == 0 && ![20, 32].contains(&program.len())) {
        return Err(R::Length);
    }
    let want = if version == 0 {
        Variant::Bech32
    } else {
        Variant::Bech32m
    };
    if b.variant != want {
        return Err(R::Checksum);
    }
    let kind = match (version, program.len()) {
        (0, 20) => K::P2wpkh,
        (0, _) => K::P2wsh,
        (1, 32) => K::P2tr,
        _ => K::WitnessUnknown,
    };
    Ok(decoded(kind, testnet, s.to_ascii_lowercase(), program))
}

fn bitcoin_like(s: &str, versions: Versions, hrps: Hrps) -> Result<Decoded, R> {
    let lower = s.to_ascii_lowercase();
    if hrps.iter().any(|(h, _)| {
        lower
            .strip_prefix(h)
            .is_some_and(|rest| rest.starts_with('1'))
    }) {
        return segwit(s, hrps);
    }
    base58_versioned(s, versions).map_err(|r| match r {
        // A well-formed bech32 address of another chain (`ltc1…` for Bitcoin).
        R::Encoding | R::Length if codec::bech32_decode(s, Some(90)).is_ok() => R::Prefix,
        r => r,
    })
}

fn sample_base58(versions: Versions, kind: K, testnet: bool, rnd: Rnd<'_>) -> String {
    let opts: Vec<u8> = versions
        .iter()
        .filter(|(_, k, t)| *k == kind && *t == testnet)
        .map(|(v, _, _)| *v)
        .collect();
    let mut body = vec![opts[rnd() as usize % opts.len()]];
    body.extend(bytes(20, rnd));
    codec::base58check_encode(&body, codec::BTC_ALPHABET)
}

fn sample_segwit(hrps: Hrps, kind: K, testnet: bool, rnd: Rnd<'_>) -> String {
    let hrp = hrps
        .iter()
        .find(|(_, t)| *t == testnet)
        .map(|(h, _)| *h)
        .unwrap_or("bc");
    let (version, len, variant) = match kind {
        K::P2wpkh => (0u8, 20, Variant::Bech32),
        K::P2wsh => (0, 32, Variant::Bech32),
        _ => (1, 32, Variant::Bech32m),
    };
    let mut data = vec![version];
    data.extend(codec::convert_bits(&bytes(len, rnd), 8, 5, true).unwrap_or_default());
    codec::bech32_encode(hrp, &data, variant)
}

// ───────────────────────────── Bitcoin Cash ─────────────────────────────

/// CashAddr (Bitcoin Cash "cashaddr.md"): version byte = type << 3 | size;
/// types 0 (P2PKH) and 1 (P2SH); the size bits fix the hash length.
fn cashaddr(s: &str) -> Result<Decoded, R> {
    let (prefix, payload) = codec::cashaddr_decode(s, &["bitcoincash", "bchtest"])?;
    let (&version, hash) = payload.split_first().ok_or(R::Length)?;
    if version & 0x80 != 0 {
        return Err(R::Encoding);
    }
    let kind = match version >> 3 {
        0 => K::P2pkh,
        1 => K::P2sh,
        _ => return Err(R::Prefix),
    };
    if hash.len() != [20, 24, 28, 32, 40, 48, 56, 64][(version & 7) as usize] {
        return Err(R::Length);
    }
    let body = s.rsplit(':').next().unwrap_or(s).to_ascii_lowercase();
    let testnet = prefix == "bchtest";
    Ok(decoded(
        kind,
        testnet,
        format!("{prefix}:{body}"),
        hash.to_vec(),
    ))
}

fn bitcoin_cash(s: &str) -> Result<Decoded, R> {
    if s.contains(':') || s.starts_with(['q', 'p', 'Q', 'P']) {
        cashaddr(s)
    } else {
        base58_versioned(s, BTC_B58)
    }
}

// ───────────────────────────── Ethereum ─────────────────────────────

/// EIP-55: hex letter `i` uppercase iff nibble `i` of
/// `keccak256(lowercase hex)` is >= 8.
pub(super) fn eip55(hex: &str) -> String {
    let lower = hex.to_ascii_lowercase();
    let hash = codec::keccak256(lower.as_bytes());
    lower
        .chars()
        .enumerate()
        .map(|(i, c)| {
            let nibble = (hash[i / 2] >> if i % 2 == 0 { 4 } else { 0 }) & 0x0f;
            if nibble >= 8 {
                c.to_ascii_uppercase()
            } else {
                c
            }
        })
        .collect()
}

/// Case check of a hex string: mixed case must be EIP-55; single case only
/// when `required` is off.
pub(super) fn check_eip55(hex: &str, required: bool) -> Result<String, R> {
    let sum = eip55(hex);
    let mixed =
        hex.bytes().any(|b| b.is_ascii_lowercase()) && hex.bytes().any(|b| b.is_ascii_uppercase());
    if (mixed || required) && sum != hex {
        return Err(R::Checksum);
    }
    Ok(sum)
}

fn ethereum(s: &str, checksummed: bool) -> Result<Decoded, R> {
    let hex = s.strip_prefix("0x").ok_or(R::Prefix)?;
    if hex.len() != 40 {
        return Err(R::Length);
    }
    let payload = codec::hex_decode(hex).ok_or(R::Encoding)?;
    let sum = check_eip55(hex, checksummed)?;
    Ok(decoded(K::Account, None, format!("0x{sum}"), payload))
}

// ───────────────────────── TRON / Solana / XRP ─────────────────────────

/// Base58Check, one 0x41 version byte + 20 bytes (TRON docs "Account").
fn tron(s: &str) -> Result<Decoded, R> {
    one_version(s, codec::BTC_ALPHABET, 0x41)
}

/// Base58Check in the Ripple alphabet, version 0x00 + 20-byte account id
/// (XRPL docs "Address Encoding").
fn xrp(s: &str) -> Result<Decoded, R> {
    one_version(s, codec::RIPPLE_ALPHABET, 0x00)
}

fn one_version(s: &str, alphabet: &[u8; 58], version: u8) -> Result<Decoded, R> {
    if !(25..=35).contains(&s.len()) {
        return Err(R::Length);
    }
    let body = codec::base58check_decode(s, alphabet)?;
    if body.len() != 21 {
        return Err(R::Length);
    }
    if body[0] != version {
        return Err(R::Prefix);
    }
    Ok(decoded(K::Account, None, s.to_string(), body[1..].to_vec()))
}

fn sample_one_version(alphabet: &[u8; 58], version: u8, rnd: Rnd<'_>) -> String {
    let mut body = vec![version];
    body.extend(bytes(20, rnd));
    codec::base58check_encode(&body, alphabet)
}

/// Plain Base58 of a 32-byte ed25519 public key; Solana addresses carry no
/// checksum, so a typo that still decodes to 32 bytes passes.
fn solana(s: &str) -> Result<Decoded, R> {
    if !(32..=44).contains(&s.len()) {
        return Err(R::Length);
    }
    let raw = codec::base58_decode(s, codec::BTC_ALPHABET).ok_or(R::Encoding)?;
    if raw.len() != 32 {
        return Err(R::Length);
    }
    Ok(decoded(K::Account, None, s.to_string(), raw))
}

// ───────────────────────────── Cardano ─────────────────────────────

/// Shelley address (CIP-19): bech32 with no length limit, header byte
/// `type << 4 | network id`.
fn cardano(s: &str) -> Result<Decoded, R> {
    let b = codec::bech32_decode(s, None)?;
    if b.variant != Variant::Bech32 {
        return Err(R::Checksum);
    }
    let (reward_hrp, testnet) = match b.hrp.as_str() {
        "addr" => (false, false),
        "addr_test" => (false, true),
        "stake" => (true, false),
        "stake_test" => (true, true),
        _ => return Err(R::Prefix),
    };
    let raw = codec::convert_bits(&b.data, 5, 8, false).ok_or(R::Encoding)?;
    let (&header, rest) = raw.split_first().ok_or(R::Length)?;
    let (kind, len_ok) = match header >> 4 {
        0..=3 => (K::Base, rest.len() == 56),
        4 | 5 => (K::Pointer, rest.len() > 28 && pointer_ok(&rest[28..])),
        6 | 7 => (K::Enterprise, rest.len() == 28),
        14 | 15 => (K::Reward, rest.len() == 28),
        _ => return Err(R::Encoding),
    };
    if (kind == K::Reward) != reward_hrp {
        return Err(R::Prefix);
    }
    if !len_ok {
        return Err(R::Length);
    }
    if header & 0x0f != if testnet { 0 } else { 1 } {
        return Err(R::Network);
    }
    Ok(decoded(
        kind,
        testnet,
        s.to_ascii_lowercase(),
        rest.to_vec(),
    ))
}

/// Exactly three variable-length naturals (7 bits per byte, high bit =
/// more), slot / tx index / cert index.
fn pointer_ok(mut b: &[u8]) -> bool {
    for _ in 0..3 {
        let Some(end) = b.iter().position(|x| x & 0x80 == 0) else {
            return false;
        };
        if end >= 9 {
            return false;
        }
        b = &b[end + 1..];
    }
    b.is_empty()
}

fn sample_cardano(kind: K, testnet: bool, rnd: Rnd<'_>) -> String {
    let net = u8::from(!testnet);
    let (typ, len) = match kind {
        K::Enterprise => (6u8, 28),
        K::Pointer => (4, 28),
        K::Reward => (14, 28),
        _ => (0, 56),
    };
    let mut raw = vec![typ << 4 | net];
    raw.extend(bytes(len, rnd));
    if kind == K::Pointer {
        raw.extend([rnd() & 0x7f, rnd() & 0x7f, rnd() & 0x7f]);
    }
    let hrp = match (kind == K::Reward, testnet) {
        (false, false) => "addr",
        (false, true) => "addr_test",
        (true, false) => "stake",
        (true, true) => "stake_test",
    };
    let data = codec::convert_bits(&raw, 8, 5, true).unwrap_or_default();
    codec::bech32_encode(hrp, &data, Variant::Bech32)
}

// ───────────────────────────── Cosmos ─────────────────────────────

/// Cosmos-SDK account: bech32 (not bech32m), the chain's hrp, 20-byte
/// (secp256k1) or 32-byte (module / ICA) address.
pub(super) fn cosmos(s: &str, hrp: &str) -> Result<Decoded, R> {
    let b = codec::bech32_decode(s, Some(90))?;
    if b.hrp != hrp {
        return Err(R::Prefix);
    }
    if b.variant != Variant::Bech32 {
        return Err(R::Checksum);
    }
    let payload = codec::convert_bits(&b.data, 5, 8, false).ok_or(R::Encoding)?;
    if payload.len() != 20 && payload.len() != 32 {
        return Err(R::Length);
    }
    Ok(decoded(K::Account, None, s.to_ascii_lowercase(), payload))
}

pub(super) fn sample_cosmos(hrp: &str, rnd: Rnd<'_>) -> String {
    let data = codec::convert_bits(&bytes(20, rnd), 8, 5, true).unwrap_or_default();
    codec::bech32_encode(hrp, &data, Variant::Bech32)
}

// ───────────────────────────── SS58 ─────────────────────────────

/// SS58 network prefix as 1 byte (0..=63) or 2 bytes (64..=16383).
fn ss58_prefix_bytes(p: u16) -> Vec<u8> {
    if p < 64 {
        vec![p as u8]
    } else {
        vec![
            ((p & 0b1111_1100) >> 2) as u8 | 0b0100_0000,
            ((p >> 8) as u8) | (((p & 0b11) as u8) << 6),
        ]
    }
}

/// SS58 (Substrate docs "SS58 address format"): prefix, 32-byte account id
/// (or 33-byte ECDSA key), 2-byte blake2b-512 checksum over
/// `"SS58PRE" ++ prefix ++ id`.
pub(super) fn ss58(s: &str, prefix: u16) -> Result<Decoded, R> {
    if !(35..=60).contains(&s.len()) {
        return Err(R::Length);
    }
    let raw = codec::base58_decode(s, codec::BTC_ALPHABET).ok_or(R::Encoding)?;
    let (ident, plen) = match *raw.first().ok_or(R::Length)? {
        b0 @ 0..=63 => (u16::from(b0), 1),
        b0 @ 64..=127 => {
            let b1 = *raw.get(1).ok_or(R::Length)?;
            let lower = (b0 << 2) | (b1 >> 6);
            (u16::from(lower) | (u16::from(b1 & 0x3f) << 8), 2)
        }
        _ => return Err(R::Encoding),
    };
    if raw.len() < plen + 2 || !matches!(raw.len() - plen - 2, 32 | 33) {
        return Err(R::Length);
    }
    let body = raw.len() - 2;
    if codec::ss58_checksum(&raw[..body]) != raw[body..] {
        return Err(R::Checksum);
    }
    if ident != prefix {
        return Err(R::Prefix);
    }
    Ok(decoded(
        K::Account,
        None,
        s.to_string(),
        raw[plen..body].to_vec(),
    ))
}

pub(super) fn sample_ss58(prefix: u16, rnd: Rnd<'_>) -> String {
    let mut raw = ss58_prefix_bytes(prefix);
    raw.extend(bytes(32, rnd));
    let sum = codec::ss58_checksum(&raw);
    raw.extend(sum);
    codec::base58_encode(&raw, codec::BTC_ALPHABET)
}

// ───────────────────────────── TON ─────────────────────────────

/// TON (docs "Smart contract addresses"): user-friendly = base64(url) of
/// flags + workchain + 32-byte hash + CRC16-XMODEM, 48 characters; raw =
/// `<workchain>:<64 hex>`.
fn ton(s: &str) -> Result<Decoded, R> {
    if let Some((wc, hex)) = s.split_once(':') {
        // canonical decimal only (no `+0`, `00`, `-0`), as the schema pattern says
        let digits = wc.strip_prefix('-').unwrap_or(wc);
        if digits.is_empty()
            || !digits.bytes().all(|b| b.is_ascii_digit())
            || (digits.starts_with('0') && wc != "0")
        {
            return Err(R::Prefix);
        }
        let wc: i8 = wc.parse().map_err(|_| R::Prefix)?;
        if hex.len() != 64 {
            return Err(R::Length);
        }
        let hash = codec::hex_decode(hex).ok_or(R::Encoding)?;
        let norm = format!("{wc}:{}", hex.to_ascii_lowercase());
        return Ok(decoded(K::Raw, None, norm, hash));
    }
    if s.len() != 48 {
        return Err(R::Length);
    }
    let url = s.contains(['-', '_']);
    let raw = codec::base64_decode(s, url).ok_or(R::Encoding)?;
    if raw.len() != 36 {
        return Err(R::Length);
    }
    if codec::crc16_xmodem(&raw[..34]).to_be_bytes() != raw[34..] {
        return Err(R::Checksum);
    }
    let kind = match raw[0] & 0x7f {
        0x11 => K::Bounceable,
        0x51 => K::NonBounceable,
        _ => return Err(R::Prefix),
    };
    Ok(decoded(
        kind,
        raw[0] & 0x80 != 0,
        s.to_string(),
        raw[2..34].to_vec(),
    ))
}

fn sample_ton(kind: K, testnet: bool, rnd: Rnd<'_>) -> String {
    if kind == K::Raw {
        return format!("0:{}", codec::hex_encode(&bytes(32, rnd)));
    }
    let flag = if kind == K::NonBounceable { 0x51 } else { 0x11 };
    let mut raw = vec![flag | if testnet { 0x80 } else { 0 }, 0];
    raw.extend(bytes(32, rnd));
    raw.extend(codec::crc16_xmodem(&raw).to_be_bytes());
    codec::base64_encode(&raw, true)
}

// ───────────────────────────── Monero ─────────────────────────────

/// `(network byte, kind, testnet-or-stagenet)` (Monero
/// `cryptonote_config.h`).
const XMR: &[(u8, K, bool)] = &[
    (18, K::Standard, false),
    (19, K::Integrated, false),
    (42, K::Subaddress, false),
    (53, K::Standard, true),
    (54, K::Integrated, true),
    (63, K::Subaddress, true),
    (24, K::Standard, true),
    (25, K::Integrated, true),
    (36, K::Subaddress, true),
];

/// Monero Base58 of network byte + spend key + view key (+ 8-byte payment
/// id for integrated) + 4-byte keccak-256 checksum: 95 / 106 characters.
fn monero(s: &str) -> Result<Decoded, R> {
    if s.len() != 95 && s.len() != 106 {
        return Err(R::Length);
    }
    let raw = codec::monero_decode(s).ok_or(R::Encoding)?;
    let body = raw.len() - 4;
    if codec::keccak256(&raw[..body])[..4] != raw[body..] {
        return Err(R::Checksum);
    }
    let &(_, kind, testnet) = XMR.iter().find(|(b, _, _)| *b == raw[0]).ok_or(R::Prefix)?;
    if (kind == K::Integrated) != (raw.len() == 77) {
        return Err(R::Length);
    }
    Ok(decoded(kind, testnet, s.to_string(), raw[1..body].to_vec()))
}

fn sample_monero(kind: K, testnet: bool, rnd: Rnd<'_>) -> String {
    let &(byte, _, _) = XMR
        .iter()
        .find(|(_, k, t)| *k == kind && *t == testnet)
        .unwrap_or(&XMR[0]);
    let mut raw = vec![byte];
    raw.extend(bytes(if kind == K::Integrated { 72 } else { 64 }, rnd));
    let sum = codec::keccak256(&raw);
    raw.extend_from_slice(&sum[..4]);
    codec::monero_encode(&raw)
}

// ───────────────────────────── dispatch ─────────────────────────────

pub(super) fn validate(n: Network, s: &str, checksummed: bool) -> Result<Decoded, R> {
    match n {
        Network::Bitcoin => bitcoin_like(s, BTC_B58, BTC_HRP),
        Network::Litecoin => bitcoin_like(s, LTC_B58, LTC_HRP),
        Network::Dogecoin => base58_versioned(s, DOGE_B58),
        Network::BitcoinCash => bitcoin_cash(s),
        Network::Ethereum => ethereum(s, checksummed),
        Network::Tron => tron(s),
        Network::Solana => solana(s),
        Network::Xrp => xrp(s),
        Network::Cardano => cardano(s),
        Network::Cosmos => cosmos(s, "cosmos"),
        Network::Polkadot => ss58(s, 0),
        Network::Kusama => ss58(s, 2),
        Network::Substrate => ss58(s, 42),
        Network::Ton => ton(s),
        Network::Monero => monero(s),
    }
}

/// Kinds a network can produce in [`sample`].
pub(super) fn kinds(n: Network) -> &'static [K] {
    match n {
        Network::Bitcoin | Network::Litecoin => &[K::P2pkh, K::P2sh, K::P2wpkh, K::P2wsh, K::P2tr],
        Network::Dogecoin | Network::BitcoinCash => &[K::P2pkh, K::P2sh],
        Network::Cardano => &[K::Base, K::Pointer, K::Enterprise, K::Reward],
        Network::Ton => &[K::Bounceable, K::NonBounceable, K::Raw],
        Network::Monero => &[K::Standard, K::Integrated, K::Subaddress],
        _ => &[K::Account],
    }
}

pub(super) fn sample(n: Network, kind: K, testnet: bool, rnd: Rnd<'_>) -> String {
    match n {
        Network::Bitcoin | Network::Litecoin => {
            let (v, h) = if n == Network::Bitcoin {
                (BTC_B58, BTC_HRP)
            } else {
                (LTC_B58, LTC_HRP)
            };
            match kind {
                K::P2pkh | K::P2sh => sample_base58(v, kind, testnet, rnd),
                _ => sample_segwit(h, kind, testnet, rnd),
            }
        }
        Network::Dogecoin => sample_base58(DOGE_B58, kind, testnet, rnd),
        Network::BitcoinCash => {
            let mut payload = vec![if kind == K::P2sh { 1 << 3 } else { 0 }];
            payload.extend(bytes(20, rnd));
            let prefix = if testnet { "bchtest" } else { "bitcoincash" };
            codec::cashaddr_encode(prefix, &payload)
        }
        Network::Ethereum => format!("0x{}", eip55(&codec::hex_encode(&bytes(20, rnd)))),
        Network::Tron => sample_one_version(codec::BTC_ALPHABET, 0x41, rnd),
        Network::Xrp => sample_one_version(codec::RIPPLE_ALPHABET, 0x00, rnd),
        Network::Solana => codec::base58_encode(&bytes(32, rnd), codec::BTC_ALPHABET),
        Network::Cardano => sample_cardano(kind, testnet, rnd),
        Network::Cosmos => sample_cosmos("cosmos", rnd),
        Network::Polkadot => sample_ss58(0, rnd),
        Network::Kusama => sample_ss58(2, rnd),
        Network::Substrate => sample_ss58(42, rnd),
        Network::Ton => sample_ton(kind, testnet, rnd),
        Network::Monero => sample_monero(kind, testnet, rnd),
    }
}

// ───────────────────────────── JSON Schema ─────────────────────────────

/// Base58 characters (the Bitcoin and Ripple alphabets share the set).
#[cfg(feature = "openapi")]
pub(super) const B58_CLASS: &str = "[1-9A-HJ-NP-Za-km-z]";
#[cfg(feature = "openapi")]
const BECH32_CLASS: &str = "[02-9ac-hj-np-zAC-HJ-NP-Z]";

/// `hrp1<data>` in either case, `min..=max` data characters.
#[cfg(feature = "openapi")]
pub(super) fn bech32_pattern(hrps: &[&str], min: usize, max: usize) -> String {
    let alts: Vec<String> = hrps
        .iter()
        .flat_map(|h| [h.to_ascii_lowercase(), h.to_ascii_uppercase()])
        .collect();
    format!("(?:{})1{BECH32_CLASS}{{{min},{max}}}", alts.join("|"))
}

/// A `pattern` approximating the shape (never stricter than the check).
#[cfg(feature = "openapi")]
pub(super) fn pattern(n: Network) -> String {
    let b58 = |first: &str, min: usize, max: usize| format!("[{first}]{B58_CLASS}{{{min},{max}}}");
    match n {
        Network::Bitcoin => format!(
            "{}|{}",
            b58("13mn2", 24, 34),
            bech32_pattern(&["bc", "tb"], 8, 87)
        ),
        Network::Litecoin => format!(
            "{}|{}",
            b58("LM3mnQ2", 24, 34),
            bech32_pattern(&["ltc", "tltc"], 8, 86)
        ),
        Network::Dogecoin => b58("DA9n2", 24, 34),
        Network::BitcoinCash => format!(
            "(?:(?:bitcoincash|bchtest):)?[qprz][02-9ac-hj-np-z]{{41,111}}|(?:(?:BITCOINCASH|BCHTEST):)?[QPRZ][02-9AC-HJ-NP-Z]{{41,111}}|{}",
            b58("13mn2", 24, 34)
        ),
        Network::Ethereum => "0x[0-9a-fA-F]{40}".to_string(),
        Network::Tron => format!("T{B58_CLASS}{{33}}"),
        Network::Solana => format!("{B58_CLASS}{{32,44}}"),
        Network::Xrp => format!("r{B58_CLASS}{{24,34}}"),
        Network::Cardano => format!(
            "(?:addr|addr_test|stake|stake_test|ADDR|ADDR_TEST|STAKE|STAKE_TEST)1{BECH32_CLASS}{{6,}}"
        ),
        Network::Cosmos => bech32_pattern(&["cosmos"], 38, 58),
        Network::Polkadot | Network::Kusama | Network::Substrate => {
            format!("{B58_CLASS}{{35,60}}")
        }
        Network::Ton => "[A-Za-z0-9+/_-]{48}|-?[0-9]{1,3}:[0-9a-fA-F]{64}".to_string(),
        Network::Monero => format!("{B58_CLASS}{{95}}(?:{B58_CLASS}{{11}})?"),
    }
}
