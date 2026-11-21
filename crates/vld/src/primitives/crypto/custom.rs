//! User-defined address templates.

use super::codec::{self, Variant};
use super::networks::{check_eip55, eip55};
use super::{AddressKind, CryptoAddressReason as R, Decoded};

/// Base58 alphabet of a custom template.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Alphabet {
    /// `123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz`
    /// (Bitcoin, Solana, TRON, SS58, IPFS).
    Bitcoin,
    /// `rpshnaf39wBUDNEGHJKLM4PQRST7VWXYZ2bcdeCg65jkm8oFqi1tuvAxyz` (XRP).
    Ripple,
    /// `123456789abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ` (Flickr).
    Flickr,
}

impl Alphabet {
    fn chars(self) -> &'static [u8; 58] {
        match self {
            Alphabet::Bitcoin => codec::BTC_ALPHABET,
            Alphabet::Ripple => codec::RIPPLE_ALPHABET,
            Alphabet::Flickr => codec::FLICKR_ALPHABET,
        }
    }
}

/// Checksum of a custom template, appended to `version ++ payload` (byte
/// encodings: Base58, hex, base64; bech32 always has its own polymod and
/// text templates none).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Checksum {
    /// No checksum.
    None,
    /// First 4 bytes of `sha256(sha256(body))` (Base58Check).
    Sha256d,
    /// First 4 bytes of `keccak256(body)` (Monero style).
    Keccak256,
    /// EIP-55 mixed-case checksum of a hex template (no extra bytes; hex only,
    /// ignored by other encodings).
    Eip55,
    /// First 2 bytes of `blake2b-512("SS58PRE" ++ body)` (SS58 style).
    Ss58,
    /// CRC16-XMODEM, 2 bytes big-endian (TON style).
    Crc16Xmodem,
    /// CRC-32 (IEEE), 4 bytes big-endian.
    Crc32,
}

impl Checksum {
    fn bytes(self, body: &[u8]) -> Vec<u8> {
        match self {
            Checksum::None | Checksum::Eip55 => vec![],
            Checksum::Sha256d => codec::sha256d(body)[..4].to_vec(),
            Checksum::Keccak256 => codec::keccak256(body)[..4].to_vec(),
            Checksum::Ss58 => codec::ss58_checksum(body).to_vec(),
            Checksum::Crc16Xmodem => codec::crc16_xmodem(body).to_be_bytes().to_vec(),
            Checksum::Crc32 => codec::crc32(body).to_be_bytes().to_vec(),
        }
    }

    fn len(self) -> usize {
        match self {
            Checksum::None | Checksum::Eip55 => 0,
            Checksum::Ss58 | Checksum::Crc16Xmodem => 2,
            Checksum::Sha256d | Checksum::Keccak256 | Checksum::Crc32 => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Encoding {
    /// No decoding: prefix, length and pattern only.
    Text,
    Base58(Alphabet),
    Bech32(Variant),
    Hex,
    Base64 {
        url: bool,
    },
}

/// A user-defined address template, built from
/// [`CryptoNetwork::custom`](super::CryptoNetwork::custom) and used wherever a
/// [`CryptoNetwork`](super::CryptoNetwork) is (`crypto_address`,
/// [`address`](super::address)).
///
/// The text is checked in this order, each failure with its reason:
/// character length ([`length`](Self::length), `length`), [`pattern`](Self::pattern)
/// (`encoding`), literal [`prefix`](Self::prefix) (`prefix`), decoding
/// (`encoding`; bech32: hrp `prefix`, variant `checksum`), [`Checksum`]
/// (`checksum`), [`version_bytes`](Self::version_bytes) (`prefix`) and the
/// payload length ([`payload_len`](Self::payload_len), `length`). Valid
/// addresses have kind [`AddressKind::Account`] and are never testnet.
///
/// ```
/// use vld::crypto::{Checksum, CryptoNetwork};
///
/// // An EVM-style hex address with a required EIP-55 checksum.
/// let evm = CryptoNetwork::custom("evm").prefix("0x").hex().payload_len(20)
///     .checksum(Checksum::Eip55);
/// let net = CryptoNetwork::from(evm.clone()).checksummed();
/// assert!(net.validate("0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed").is_ok());
///
/// // Encode a payload with the template, read it back.
/// let addr = evm.encode(&[7; 20]).unwrap();
/// assert_eq!(net.validate(&addr).unwrap().payload, vec![7; 20]);
/// ```
#[derive(Clone, Debug)]
pub struct CustomNetwork {
    name: String,
    encoding: Encoding,
    checksum: Checksum,
    versions: Vec<Vec<u8>>,
    hrps: Vec<String>,
    payload: (usize, usize),
    chars: (usize, usize),
    prefix: String,
    #[cfg(feature = "regex")]
    pattern: Option<crate::rules::Pattern>,
}

impl CustomNetwork {
    pub(super) fn new(name: String) -> Self {
        Self {
            name,
            encoding: Encoding::Text,
            checksum: Checksum::None,
            versions: vec![],
            hrps: vec![],
            payload: (0, usize::MAX),
            chars: (1, usize::MAX),
            prefix: String::new(),
            #[cfg(feature = "regex")]
            pattern: None,
        }
    }

    /// The template's name (the issue's `network` parameter).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Plain Base58 in `alphabet` (no checksum unless [`checksum`](Self::checksum) sets one).
    pub fn base58(mut self, alphabet: Alphabet) -> Self {
        self.encoding = Encoding::Base58(alphabet);
        self
    }

    /// Base58Check: Bitcoin alphabet (change with [`alphabet`](Self::alphabet)),
    /// [`Checksum::Sha256d`], one leading version byte out of `versions`.
    pub fn base58check(mut self, versions: &[u8]) -> Self {
        self.encoding = Encoding::Base58(Alphabet::Bitcoin);
        self.checksum = Checksum::Sha256d;
        self.versions = versions.iter().map(|v| vec![*v]).collect();
        self
    }

    /// Base58 alphabet (switches the encoding to Base58).
    pub fn alphabet(mut self, alphabet: Alphabet) -> Self {
        self.encoding = Encoding::Base58(alphabet);
        self
    }

    /// Allowed leading version byte sequences (multi-byte versions, e.g.
    /// Zcash `[0x1c, 0xb8]`); the payload is what follows.
    pub fn version_bytes(mut self, versions: &[&[u8]]) -> Self {
        self.versions = versions.iter().map(|v| v.to_vec()).collect();
        self
    }

    /// bech32 (BIP-173) with one of these hrps (any hrp when empty); the
    /// payload is the 8-bit regrouped data. No 90-character limit unless
    /// [`length`](Self::length) sets one.
    pub fn bech32(mut self, hrps: &[&str]) -> Self {
        self.encoding = Encoding::Bech32(Variant::Bech32);
        self.hrps = hrps.iter().map(|h| h.to_ascii_lowercase()).collect();
        self
    }

    /// bech32m (BIP-350), as [`bech32`](Self::bech32).
    pub fn bech32m(mut self, hrps: &[&str]) -> Self {
        self.encoding = Encoding::Bech32(Variant::Bech32m);
        self.hrps = hrps.iter().map(|h| h.to_ascii_lowercase()).collect();
        self
    }

    /// Hex (either case; a `0x` goes in [`prefix`](Self::prefix)).
    pub fn hex(mut self) -> Self {
        self.encoding = Encoding::Hex;
        self
    }

    /// Standard base64 (`+/`), padding optional.
    pub fn base64(mut self) -> Self {
        self.encoding = Encoding::Base64 { url: false };
        self
    }

    /// URL-safe base64 (`-_`), padding optional.
    pub fn base64url(mut self) -> Self {
        self.encoding = Encoding::Base64 { url: true };
        self
    }

    /// A literal the address starts with, before the encoded part (`"0x"`,
    /// `"nano_"`); ignored by bech32 (use the hrp list).
    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Checksum of byte encodings (`Base58Check` sets [`Checksum::Sha256d`]).
    pub fn checksum(mut self, checksum: Checksum) -> Self {
        self.checksum = checksum;
        self
    }

    /// Exact decoded payload length in bytes (after version, before checksum).
    pub fn payload_len(self, n: usize) -> Self {
        self.payload_len_range(n, n)
    }

    /// Payload length range in bytes (inclusive).
    pub fn payload_len_range(mut self, min: usize, max: usize) -> Self {
        self.payload = (min, max);
        self
    }

    /// Exact address length in characters (prefix included).
    pub fn length(self, n: usize) -> Self {
        self.length_range(n, n)
    }

    /// Address length range in characters (inclusive).
    pub fn length_range(mut self, min: usize, max: usize) -> Self {
        self.chars = (min, max);
        self
    }

    /// The whole address must match this regular expression (feature
    /// `regex`); with no encoding set, the template is pattern + length only.
    #[cfg(feature = "regex")]
    #[cfg_attr(docsrs, doc(cfg(feature = "regex")))]
    pub fn pattern(mut self, pattern: impl crate::rules::IntoPattern) -> Self {
        self.pattern = Some(pattern.into_pattern());
        self
    }

    /// The address of `payload` with the first version / hrp: `None` for a
    /// text template, or bech32 without an hrp. Length limits are not
    /// applied, so the result fails validation when `payload` breaks them.
    pub fn encode(&self, payload: &[u8]) -> Option<String> {
        self.encode_with(0, payload)
    }

    fn encode_with(&self, version: usize, payload: &[u8]) -> Option<String> {
        let body_text = match self.encoding {
            Encoding::Text => return None,
            Encoding::Bech32(variant) => {
                let hrp = self.hrps.first()?;
                let data = codec::convert_bits(payload, 8, 5, true)?;
                return Some(codec::bech32_encode(hrp, &data, variant));
            }
            enc => {
                let mut body = self.versions.get(version).cloned().unwrap_or_default();
                body.extend_from_slice(payload);
                let sum = self.checksum.bytes(&body);
                body.extend(sum);
                match enc {
                    Encoding::Base58(a) => codec::base58_encode(&body, a.chars()),
                    Encoding::Hex if self.checksum == Checksum::Eip55 => {
                        eip55(&codec::hex_encode(&body))
                    }
                    Encoding::Hex => codec::hex_encode(&body),
                    Encoding::Base64 { url } => codec::base64_encode(&body, url),
                    Encoding::Text | Encoding::Bech32(_) => unreachable!(),
                }
            }
        };
        Some(format!("{}{body_text}", self.prefix))
    }

    /// A random payload (20 bytes when the length is open) with a random
    /// version.
    pub(super) fn sample(&self, rnd: &mut dyn FnMut() -> u8) -> Option<String> {
        let (lo, hi) = self.payload;
        let hi = if hi == usize::MAX { lo.max(20) } else { hi };
        let len = lo + (usize::from(rnd()) % (hi.saturating_sub(lo) + 1));
        let payload: Vec<u8> = (0..len).map(|_| rnd()).collect();
        let version = usize::from(rnd()) % self.versions.len().max(1);
        self.encode_with(version, &payload)
    }

    pub(super) fn validate(&self, s: &str, checksummed: bool) -> Result<Decoded, R> {
        let n = s.chars().count();
        if n < self.chars.0 || n > self.chars.1 {
            return Err(R::Length);
        }
        #[cfg(feature = "regex")]
        if self.pattern.as_ref().is_some_and(|p| !p.is_match(s)) {
            return Err(R::Encoding);
        }
        if let Encoding::Bech32(variant) = self.encoding {
            let b = codec::bech32_decode(s, None)?;
            if !self.hrps.is_empty() && !self.hrps.contains(&b.hrp) {
                return Err(R::Prefix);
            }
            if b.variant != variant {
                return Err(R::Checksum);
            }
            let payload = codec::convert_bits(&b.data, 5, 8, false).ok_or(R::Encoding)?;
            return self.finish(s.to_ascii_lowercase(), payload);
        }
        let rest = s.strip_prefix(self.prefix.as_str()).ok_or(R::Prefix)?;
        let (raw, normalized) = match self.encoding {
            Encoding::Text => return self.finish(s.to_string(), rest.as_bytes().to_vec()),
            Encoding::Base58(a) => (
                codec::base58_decode(rest, a.chars()).ok_or(R::Encoding)?,
                s.to_string(),
            ),
            Encoding::Hex => {
                let raw = codec::hex_decode(rest).ok_or(R::Encoding)?;
                let text = if self.checksum == Checksum::Eip55 {
                    check_eip55(rest, checksummed)?
                } else {
                    rest.to_ascii_lowercase()
                };
                (raw, format!("{}{text}", self.prefix))
            }
            Encoding::Base64 { url } => (
                codec::base64_decode(rest, url).ok_or(R::Encoding)?,
                s.to_string(),
            ),
            Encoding::Bech32(_) => unreachable!(),
        };
        let sum_len = self.checksum.len();
        if raw.len() < sum_len {
            return Err(R::Length);
        }
        let (body, sum) = raw.split_at(raw.len() - sum_len);
        if self.checksum.bytes(body) != sum {
            return Err(R::Checksum);
        }
        let payload = if self.versions.is_empty() {
            body
        } else {
            let v = self
                .versions
                .iter()
                .find(|v| body.starts_with(v))
                .ok_or(R::Prefix)?;
            &body[v.len()..]
        };
        self.finish(normalized, payload.to_vec())
    }

    fn finish(&self, normalized: String, payload: Vec<u8>) -> Result<Decoded, R> {
        if payload.len() < self.payload.0 || payload.len() > self.payload.1 {
            return Err(R::Length);
        }
        Ok(Decoded {
            kind: AddressKind::Account,
            testnet: None,
            normalized,
            payload,
        })
    }

    /// A `pattern` approximating the template: its own regex when set,
    /// otherwise prefix + character class + length.
    #[cfg(feature = "openapi")]
    pub(super) fn schema_pattern(&self) -> String {
        #[cfg(feature = "regex")]
        if let Some(p) = &self.pattern {
            return p.as_str().to_string();
        }
        let p = self.prefix.chars().count();
        let (lo, hi) = self.chars;
        let quant = |fixed: usize| match hi {
            usize::MAX => format!("{{{},}}", lo.saturating_sub(fixed)),
            hi => format!(
                "{{{},{}}}",
                lo.saturating_sub(fixed),
                hi.saturating_sub(fixed)
            ),
        };
        let class = match self.encoding {
            Encoding::Bech32(_) => {
                let hrps: Vec<String> = if self.hrps.is_empty() {
                    vec!["[!-~]+".to_string()]
                } else {
                    self.hrps
                        .iter()
                        .flat_map(|h| [escape(h), escape(&h.to_ascii_uppercase())])
                        .collect()
                };
                return format!("(?:{})1[02-9ac-hj-np-zAC-HJ-NP-Z]{{6,}}", hrps.join("|"));
            }
            Encoding::Base58(_) => "[1-9A-HJ-NP-Za-km-z]",
            Encoding::Hex => "[0-9a-fA-F]",
            Encoding::Base64 { url: false } => "[A-Za-z0-9+/=]",
            Encoding::Base64 { url: true } => "[A-Za-z0-9_=-]",
            Encoding::Text => "[\\s\\S]",
        };
        format!("{}{class}{}", escape(&self.prefix), quant(p))
    }
}

/// Escape a literal for an ECMA-262 `pattern`.
#[cfg(feature = "openapi")]
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if "^$\\.*+?()[]{}|/".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
