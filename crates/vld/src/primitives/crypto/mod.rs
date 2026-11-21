//! Cryptocurrency wallet addresses (feature `crypto`): encoding, checksum,
//! version / prefix and length rules of the built-in [`Network`]s, plus
//! user-defined templates ([`CryptoNetwork::custom`]).
//!
//! A valid address is well-formed for its network; whether it exists, holds
//! funds or belongs to anyone is not (and cannot be) checked offline.
//!
//! ```
//! use vld::prelude::*;
//! use vld::crypto::{AddressKind, Network};
//!
//! // On a string schema: the output stays the input string.
//! let btc = vld::string().crypto_address(Network::Bitcoin);
//! assert!(btc.parse(r#""bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4""#).is_ok());
//!
//! // Typed: what was detected.
//! let eth = vld::crypto::address(Network::Ethereum.checksummed());
//! let a = eth.parse(r#""0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed""#).unwrap();
//! assert_eq!(a.kind, AddressKind::Account);
//!
//! // The issue says which network and why.
//! let err = btc.parse(r#""bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t5""#).unwrap_err();
//! assert_eq!(err.issues[0].code.key(), "invalid_crypto_address");
//! assert_eq!(err.issues[0].code.params()[1], ("reason", "checksum".to_string()));
//! ```

use std::sync::Arc;

use serde_json::Value;

use crate::error::{IssueCode, ValidationIssue, VldError};
use crate::schema::VldSchema;

pub use crate::error::CryptoAddressReason;

mod codec;
mod custom;
mod networks;

pub use custom::{Alphabet, Checksum, CustomNetwork};

/// Built-in networks. Turn one into a configured [`CryptoNetwork`] with
/// [`mainnet`](Self::mainnet), [`testnet`](Self::testnet),
/// [`kinds`](Self::kinds) or [`checksummed`](Self::checksummed), or pass it as
/// is (every kind, mainnet and testnet).
///
/// | network | forms | rules |
/// |---|---|---|
/// | `Bitcoin` | P2PKH / P2SH, SegWit v0, Taproot v1+ | Base58Check versions `0x00` / `0x05` (testnet `0x6f` / `0xc4`); bech32 (BIP-173) for v0, bech32m (BIP-350) for v1+, hrp `bc` / `tb`, program 2-40 bytes, v0 20 or 32 |
/// | `Litecoin` | `L` / `M` / `3`, `ltc1…` | versions `0x30` / `0x32` / `0x05` (testnet `0x6f` / `0x3a` / `0xc4`); hrp `ltc` / `tltc` |
/// | `Dogecoin` | `D`, `A` / `9` | versions `0x1e` / `0x16` (testnet `0x71` / `0xc4`) |
/// | `BitcoinCash` | CashAddr, legacy | `bitcoincash:` / `bchtest:` prefix optional, 40-bit polymod, version byte type + hash size; legacy as Bitcoin Base58Check |
/// | `Ethereum` | `0x` + 40 hex | EIP-55 checksum when mixed case (or always with `checksummed()`); any EVM chain |
/// | `Tron` | `T…` | Base58Check, version `0x41` |
/// | `Solana` | Base58 | decodes to exactly 32 bytes; there is no checksum |
/// | `Xrp` | `r…` | Base58Check in the Ripple alphabet, version `0x00` (X-addresses are not accepted) |
/// | `Cardano` | Shelley `addr1` / `addr_test1` / `stake1` / `stake_test1` | bech32 without the 90-char limit (CIP-19); header type sets the length, network id 1 (mainnet) / 0 (testnet) matches the hrp |
/// | `Cosmos` | bech32 `cosmos1…` | 20- or 32-byte payload; other hrps with [`CryptoNetwork::cosmos`] |
/// | `Polkadot` / `Kusama` / `Substrate` | SS58 | Base58, `blake2b-512("SS58PRE" ++ data)` 2-byte checksum, prefix 0 / 2 / 42 (others: [`CryptoNetwork::ss58`]), 32-byte account id or 33-byte ECDSA key |
/// | `Ton` | user-friendly, raw | 48 base64 / base64url chars: flags (`0x11` bounceable, `0x51` non-bounceable, `+0x80` testnet), workchain, 32-byte hash, CRC16-XMODEM; raw `<workchain>:<64 hex>` |
/// | `Monero` | standard, integrated, subaddress | Monero Base58 (8-byte blocks), keccak-256 4-byte checksum, network byte 18 / 19 / 42 (testnet 53 / 54 / 63, stagenet 24 / 25 / 36) |
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Network {
    /// Bitcoin.
    Bitcoin,
    /// Litecoin.
    Litecoin,
    /// Dogecoin.
    Dogecoin,
    /// Bitcoin Cash.
    BitcoinCash,
    /// Ethereum and other EVM chains.
    Ethereum,
    /// TRON.
    Tron,
    /// Solana.
    Solana,
    /// XRP Ledger.
    Xrp,
    /// Cardano (Shelley addresses).
    Cardano,
    /// Cosmos Hub (`cosmos` hrp).
    Cosmos,
    /// Polkadot (SS58 prefix 0).
    Polkadot,
    /// Kusama (SS58 prefix 2).
    Kusama,
    /// Generic Substrate (SS58 prefix 42).
    Substrate,
    /// TON.
    Ton,
    /// Monero.
    Monero,
}

impl Network {
    /// Every built-in network.
    pub const ALL: &'static [Network] = &[
        Network::Bitcoin,
        Network::Litecoin,
        Network::Dogecoin,
        Network::BitcoinCash,
        Network::Ethereum,
        Network::Tron,
        Network::Solana,
        Network::Xrp,
        Network::Cardano,
        Network::Cosmos,
        Network::Polkadot,
        Network::Kusama,
        Network::Substrate,
        Network::Ton,
        Network::Monero,
    ];

    /// Stable snake_case name (the issue's `network` parameter, JSON Schema
    /// `x-network`).
    pub fn name(self) -> &'static str {
        match self {
            Network::Bitcoin => "bitcoin",
            Network::Litecoin => "litecoin",
            Network::Dogecoin => "dogecoin",
            Network::BitcoinCash => "bitcoin_cash",
            Network::Ethereum => "ethereum",
            Network::Tron => "tron",
            Network::Solana => "solana",
            Network::Xrp => "xrp",
            Network::Cardano => "cardano",
            Network::Cosmos => "cosmos",
            Network::Polkadot => "polkadot",
            Network::Kusama => "kusama",
            Network::Substrate => "substrate",
            Network::Ton => "ton",
            Network::Monero => "monero",
        }
    }

    /// Mainnet addresses only (see [`CryptoNetwork::mainnet`]).
    pub fn mainnet(self) -> CryptoNetwork {
        CryptoNetwork::from(self).mainnet()
    }

    /// Testnet addresses only (see [`CryptoNetwork::testnet`]).
    pub fn testnet(self) -> CryptoNetwork {
        CryptoNetwork::from(self).testnet()
    }

    /// Only these address kinds (see [`CryptoNetwork::kinds`]).
    pub fn kinds(self, kinds: &[AddressKind]) -> CryptoNetwork {
        CryptoNetwork::from(self).kinds(kinds)
    }

    /// Require the case checksum (see [`CryptoNetwork::checksummed`]).
    pub fn checksummed(self) -> CryptoNetwork {
        CryptoNetwork::from(self).checksummed()
    }
}

/// The form of a valid address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AddressKind {
    /// Pay-to-public-key-hash (Base58Check; CashAddr type 0).
    P2pkh,
    /// Pay-to-script-hash (Base58Check; CashAddr type 1).
    P2sh,
    /// SegWit v0, 20-byte program.
    P2wpkh,
    /// SegWit v0, 32-byte program.
    P2wsh,
    /// SegWit v1, 32-byte program (Taproot).
    P2tr,
    /// Other SegWit versions / lengths (valid per BIP-350, not yet defined).
    WitnessUnknown,
    /// The only form of its network (Ethereum, TRON, Solana, XRP, Cosmos,
    /// SS58) or a custom template.
    Account,
    /// Cardano base address (payment + stake credential).
    Base,
    /// Cardano pointer address.
    Pointer,
    /// Cardano enterprise address (payment credential only).
    Enterprise,
    /// Cardano reward (stake) address.
    Reward,
    /// TON user-friendly, bounceable.
    Bounceable,
    /// TON user-friendly, non-bounceable.
    NonBounceable,
    /// TON raw `<workchain>:<hex>`.
    Raw,
    /// Monero standard address.
    Standard,
    /// Monero integrated address (with an 8-byte payment id).
    Integrated,
    /// Monero subaddress.
    Subaddress,
}

impl AddressKind {
    /// Stable snake_case key.
    pub fn key(self) -> &'static str {
        match self {
            AddressKind::P2pkh => "p2pkh",
            AddressKind::P2sh => "p2sh",
            AddressKind::P2wpkh => "p2wpkh",
            AddressKind::P2wsh => "p2wsh",
            AddressKind::P2tr => "p2tr",
            AddressKind::WitnessUnknown => "witness_unknown",
            AddressKind::Account => "account",
            AddressKind::Base => "base",
            AddressKind::Pointer => "pointer",
            AddressKind::Enterprise => "enterprise",
            AddressKind::Reward => "reward",
            AddressKind::Bounceable => "bounceable",
            AddressKind::NonBounceable => "non_bounceable",
            AddressKind::Raw => "raw",
            AddressKind::Standard => "standard",
            AddressKind::Integrated => "integrated",
            AddressKind::Subaddress => "subaddress",
        }
    }

    #[cfg(feature = "fake")]
    fn from_key(key: &str) -> Option<Self> {
        ALL_KINDS.iter().copied().find(|k| k.key() == key)
    }
}

#[cfg(feature = "fake")]
const ALL_KINDS: &[AddressKind] = &[
    AddressKind::P2pkh,
    AddressKind::P2sh,
    AddressKind::P2wpkh,
    AddressKind::P2wsh,
    AddressKind::P2tr,
    AddressKind::WitnessUnknown,
    AddressKind::Account,
    AddressKind::Base,
    AddressKind::Pointer,
    AddressKind::Enterprise,
    AddressKind::Reward,
    AddressKind::Bounceable,
    AddressKind::NonBounceable,
    AddressKind::Raw,
    AddressKind::Standard,
    AddressKind::Integrated,
    AddressKind::Subaddress,
];

/// A valid address and what was detected in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CryptoAddress {
    /// Name of the network that accepted it ([`CryptoNetwork::name`]).
    pub network: String,
    /// Address form.
    pub kind: AddressKind,
    /// A testnet (or stagenet) address. Always `false` for addresses that do
    /// not encode the network (Ethereum, TRON, Solana, XRP, Cosmos, SS58, raw
    /// TON, custom templates).
    pub testnet: bool,
    /// Canonical text: EIP-55 case for Ethereum, lowercase for bech32 / raw
    /// TON, `prefix:` + lowercase for CashAddr, the input otherwise.
    pub normalized: String,
    /// Decoded data without version bytes and checksum: the hash / key /
    /// witness program / account id (Cardano and TON: the bytes after the
    /// header; custom text templates: the text after the prefix).
    pub payload: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NetFilter {
    Any,
    Mainnet,
    Testnet,
}

#[derive(Clone, Debug)]
enum Spec {
    Builtin(Network),
    /// Cosmos-SDK bech32 with this hrp.
    Cosmos(String),
    /// SS58 with this network prefix.
    Ss58(u16),
    Custom(Arc<CustomNetwork>),
}

/// A network to validate against: a built-in [`Network`], a Cosmos hrp, an
/// SS58 prefix or a custom template, plus options (mainnet / testnet, address
/// kinds, required case checksum).
#[derive(Clone, Debug)]
pub struct CryptoNetwork {
    spec: Spec,
    net: NetFilter,
    kinds: Vec<AddressKind>,
    checksummed: bool,
}

impl From<Network> for CryptoNetwork {
    fn from(n: Network) -> Self {
        let spec = match n {
            Network::Polkadot => Spec::Ss58(0),
            Network::Kusama => Spec::Ss58(2),
            Network::Substrate => Spec::Ss58(42),
            n => Spec::Builtin(n),
        };
        Self::with(spec)
    }
}

impl From<&Network> for CryptoNetwork {
    fn from(n: &Network) -> Self {
        (*n).into()
    }
}

impl From<&CryptoNetwork> for CryptoNetwork {
    fn from(n: &CryptoNetwork) -> Self {
        n.clone()
    }
}

impl From<CustomNetwork> for CryptoNetwork {
    fn from(c: CustomNetwork) -> Self {
        Self::with(Spec::Custom(Arc::new(c)))
    }
}

impl From<&CustomNetwork> for CryptoNetwork {
    fn from(c: &CustomNetwork) -> Self {
        c.clone().into()
    }
}

impl CryptoNetwork {
    fn with(spec: Spec) -> Self {
        Self {
            spec,
            net: NetFilter::Any,
            kinds: vec![],
            checksummed: false,
        }
    }

    /// A Cosmos-SDK chain: bech32 with this hrp (`"osmo"`, `"juno"`, …),
    /// 20- or 32-byte payload. Named `cosmos:<hrp>` (`cosmos` for `"cosmos"`).
    pub fn cosmos(hrp: &str) -> Self {
        Self::with(Spec::Cosmos(hrp.to_ascii_lowercase()))
    }

    /// A Substrate chain with this SS58 network prefix (0..=16383; Polkadot 0,
    /// Kusama 2, generic 42). Named `substrate:<prefix>`.
    pub fn ss58(prefix: u16) -> Self {
        match prefix {
            0 => Network::Polkadot.into(),
            2 => Network::Kusama.into(),
            42 => Network::Substrate.into(),
            p => Self::with(Spec::Ss58(p)),
        }
    }

    /// Start a user-defined template (see [`CustomNetwork`]).
    ///
    /// ```
    /// use vld::prelude::*;
    /// use vld::crypto::CryptoNetwork;
    ///
    /// // Dogecoin P2PKH, defined by hand: Base58Check version 0x1e, 34 chars.
    /// let doge = CryptoNetwork::custom("mydoge").base58check(&[0x1e]).length(34);
    /// let s = vld::string().crypto_address(doge);
    /// assert!(s.parse(r#""DH5yaieqoZN36fDVciNyRueRGvGLR3mr7L""#).is_ok());
    /// ```
    pub fn custom(name: impl Into<String>) -> CustomNetwork {
        CustomNetwork::new(name.into())
    }

    /// Network name: [`Network::name`], `cosmos:<hrp>`, `substrate:<prefix>`
    /// or the custom template's name.
    pub fn name(&self) -> String {
        match &self.spec {
            Spec::Builtin(n) => n.name().to_string(),
            Spec::Cosmos(h) if h == "cosmos" => "cosmos".to_string(),
            Spec::Cosmos(h) => format!("cosmos:{h}"),
            Spec::Ss58(0) => "polkadot".to_string(),
            Spec::Ss58(2) => "kusama".to_string(),
            Spec::Ss58(42) => "substrate".to_string(),
            Spec::Ss58(p) => format!("substrate:{p}"),
            Spec::Custom(c) => c.name().to_string(),
        }
    }

    /// The network for a [`name`](Self::name) of a built-in, Cosmos or SS58
    /// network (custom templates have no name lookup).
    pub fn from_name(name: &str) -> Option<Self> {
        if let Some(n) = Network::ALL.iter().find(|n| n.name() == name) {
            return Some((*n).into());
        }
        if let Some(hrp) = name.strip_prefix("cosmos:") {
            return Some(Self::cosmos(hrp));
        }
        let p = name.strip_prefix("substrate:")?.parse().ok()?;
        (p <= 16383).then(|| Self::ss58(p))
    }

    /// Accept mainnet addresses only; a testnet address fails with reason
    /// `network`. No effect on networks whose addresses do not encode the
    /// network (see [`CryptoAddress::testnet`]).
    pub fn mainnet(mut self) -> Self {
        self.net = NetFilter::Mainnet;
        self
    }

    /// Accept testnet (and Monero stagenet) addresses only (reason `network`
    /// otherwise; no effect where addresses do not encode the network).
    pub fn testnet(mut self) -> Self {
        self.net = NetFilter::Testnet;
        self
    }

    /// Accept only these address kinds (reason `kind` otherwise), e.g.
    /// `[AddressKind::P2wpkh, AddressKind::P2tr]`.
    pub fn kinds(mut self, kinds: &[AddressKind]) -> Self {
        self.kinds = kinds.to_vec();
        self
    }

    /// Require the case checksum of hex addresses (EIP-55 for Ethereum and
    /// custom hex templates with [`Checksum::Eip55`]): all-lowercase /
    /// all-uppercase hex is then rejected with reason `checksum`.
    pub fn checksummed(mut self) -> Self {
        self.checksummed = true;
        self
    }

    /// Check `s` against this network.
    ///
    /// ```
    /// use vld::crypto::{AddressKind, CryptoAddressReason, Network};
    ///
    /// let btc = vld::crypto::CryptoNetwork::from(Network::Bitcoin).mainnet();
    /// let a = btc.validate("1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2").unwrap();
    /// assert_eq!((a.kind, a.testnet), (AddressKind::P2pkh, false));
    /// assert_eq!(
    ///     btc.validate("tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx"),
    ///     Err(CryptoAddressReason::Network)
    /// );
    /// ```
    pub fn validate(&self, s: &str) -> Result<CryptoAddress, CryptoAddressReason> {
        let d = match &self.spec {
            Spec::Builtin(n) => networks::validate(*n, s, self.checksummed)?,
            Spec::Cosmos(hrp) => networks::cosmos(s, hrp)?,
            Spec::Ss58(p) => networks::ss58(s, *p)?,
            Spec::Custom(c) => c.validate(s, self.checksummed)?,
        };
        match (self.net, d.testnet) {
            (NetFilter::Mainnet, Some(true)) | (NetFilter::Testnet, Some(false)) => {
                return Err(CryptoAddressReason::Network);
            }
            _ => {}
        }
        if !self.kinds.is_empty() && !self.kinds.contains(&d.kind) {
            return Err(CryptoAddressReason::Kind);
        }
        Ok(CryptoAddress {
            network: self.name(),
            kind: d.kind,
            testnet: d.testnet.unwrap_or(false),
            normalized: d.normalized,
            payload: d.payload,
        })
    }

    /// A random valid address (random payload, correct version bytes and
    /// checksum), honouring [`testnet`](Self::testnet) and
    /// [`kinds`](Self::kinds); `random_byte` supplies the randomness. Custom
    /// text templates (no encoding) return `None`, as does a kind list the
    /// network has none of.
    ///
    /// ```
    /// use vld::crypto::{CryptoNetwork, Network};
    ///
    /// let mut seed = 7u8;
    /// let mut next = || { seed = seed.wrapping_mul(31).wrapping_add(17); seed };
    /// let net = CryptoNetwork::from(Network::Monero);
    /// let addr = net.random_address(&mut next).unwrap();
    /// assert!(net.validate(&addr).is_ok());
    /// ```
    pub fn random_address(&self, mut random_byte: impl FnMut() -> u8) -> Option<String> {
        let testnet = self.net == NetFilter::Testnet;
        let rnd: &mut dyn FnMut() -> u8 = &mut random_byte;
        // A few tries: a custom length limit may reject some payloads.
        for _ in 0..64 {
            let s = match &self.spec {
                Spec::Builtin(n) => {
                    let kinds = networks::kinds(*n);
                    let kind = pick_kind(kinds, &self.kinds, rnd)?;
                    networks::sample(*n, kind, testnet, rnd)
                }
                Spec::Cosmos(hrp) => networks::sample_cosmos(hrp, rnd),
                Spec::Ss58(p) => networks::sample_ss58(*p, rnd),
                Spec::Custom(c) => {
                    if !self.kinds.is_empty() && !self.kinds.contains(&AddressKind::Account) {
                        return None;
                    }
                    c.sample(rnd)?
                }
            };
            if self.validate(&s).is_ok() {
                return Some(s);
            }
        }
        None
    }

    #[cfg(feature = "openapi")]
    fn pattern(&self) -> String {
        match &self.spec {
            Spec::Builtin(n) => networks::pattern(*n),
            Spec::Cosmos(hrp) => networks::bech32_pattern(&[hrp.as_str()], 38, 58),
            Spec::Ss58(_) => networks::pattern(Network::Substrate),
            Spec::Custom(c) => c.schema_pattern(),
        }
    }
}

fn pick_kind(
    supported: &[AddressKind],
    allowed: &[AddressKind],
    rnd: &mut dyn FnMut() -> u8,
) -> Option<AddressKind> {
    let pool: Vec<AddressKind> = supported
        .iter()
        .copied()
        .filter(|k| allowed.is_empty() || allowed.contains(k))
        .collect();
    if pool.is_empty() {
        return None;
    }
    Some(pool[rnd() as usize % pool.len()])
}

/// What a network's decoder found (before the network / kind filters).
pub(super) struct Decoded {
    kind: AddressKind,
    /// `None`: the address does not encode the network (Ethereum, TRON,
    /// Solana, XRP, Cosmos, SS58, raw TON, custom templates).
    testnet: Option<bool>,
    normalized: String,
    payload: Vec<u8>,
}

// ─────────────────────────── schema plumbing ───────────────────────────

/// The check behind [`ZString::crypto_address`](crate::primitives::ZString::crypto_address)
/// and [`ZCryptoAddress`]: valid for any of the networks.
#[derive(Clone)]
pub(crate) struct AddressCheck {
    networks: Vec<CryptoNetwork>,
}

impl AddressCheck {
    pub(crate) fn new<N: Into<CryptoNetwork>>(networks: impl IntoIterator<Item = N>) -> Self {
        Self {
            networks: networks.into_iter().map(Into::into).collect(),
        }
    }

    /// The first network that accepts `s`; otherwise the failure that got
    /// furthest (a checksum / network / kind failure names its network; when
    /// no network recognised the shape, every name joined with `|`).
    pub(crate) fn validate(&self, s: &str) -> Result<CryptoAddress, IssueCode> {
        let mut best: Option<(u8, usize, CryptoAddressReason)> = None;
        for (i, n) in self.networks.iter().enumerate() {
            match n.validate(s) {
                Ok(a) => return Ok(a),
                Err(r) => {
                    let rank = match r {
                        CryptoAddressReason::Encoding
                        | CryptoAddressReason::Prefix
                        | CryptoAddressReason::Length => 0,
                        _ => 1,
                    };
                    if best.is_none_or(|(b, _, _)| rank > b) {
                        best = Some((rank, i, r));
                    }
                }
            }
        }
        let (rank, i, reason) = best.unwrap_or((0, 0, CryptoAddressReason::Encoding));
        let network = if rank == 1 || self.networks.len() == 1 {
            self.networks[i].name()
        } else {
            self.names().join("|")
        };
        Err(IssueCode::InvalidCryptoAddress { network, reason })
    }

    fn names(&self) -> Vec<String> {
        self.networks.iter().map(CryptoNetwork::name).collect()
    }

    /// `format: crypto-address`, `x-network` (a name, or the list for
    /// several), a `pattern` approximating the shape, and for one network
    /// `x-testnet` / `x-kinds` / `x-checksummed` when set (fake data reads
    /// them).
    #[cfg(feature = "openapi")]
    pub(crate) fn json_schema(&self) -> Value {
        let pats: Vec<String> = self.networks.iter().map(CryptoNetwork::pattern).collect();
        let mut m = serde_json::Map::new();
        m.insert("format".into(), "crypto-address".into());
        m.insert("pattern".into(), format!("^(?:{})$", pats.join("|")).into());
        match self.networks.as_slice() {
            [n] => {
                m.insert("x-network".into(), n.name().into());
                if n.net != NetFilter::Any {
                    m.insert("x-testnet".into(), (n.net == NetFilter::Testnet).into());
                }
                if !n.kinds.is_empty() {
                    let kinds: Vec<&str> = n.kinds.iter().map(|k| k.key()).collect();
                    m.insert("x-kinds".into(), kinds.into());
                }
                if n.checksummed {
                    m.insert("x-checksummed".into(), true.into());
                }
            }
            _ => {
                m.insert("x-network".into(), self.names().into());
            }
        }
        Value::Object(m)
    }
}

/// A random valid address for a JSON Schema carrying `x-network` (and
/// `x-testnet` / `x-kinds` / `x-checksummed`); `None` for an unknown name.
#[cfg(feature = "fake")]
pub(crate) fn fake_address(
    obj: &serde_json::Map<String, Value>,
    random_byte: &mut dyn FnMut() -> u8,
) -> Option<String> {
    let name = match obj.get("x-network")? {
        Value::String(s) => s.clone(),
        Value::Array(a) if !a.is_empty() => {
            a[random_byte() as usize % a.len()].as_str()?.to_string()
        }
        _ => return None,
    };
    let mut net = CryptoNetwork::from_name(&name)?;
    match obj.get("x-testnet").and_then(Value::as_bool) {
        Some(true) => net = net.testnet(),
        Some(false) => net = net.mainnet(),
        None => {}
    }
    if let Some(kinds) = obj.get("x-kinds").and_then(Value::as_array) {
        let kinds: Vec<AddressKind> = kinds
            .iter()
            .filter_map(|k| AddressKind::from_key(k.as_str()?))
            .collect();
        net = net.kinds(&kinds);
    }
    net.random_address(random_byte)
}

/// Typed address schema: outputs a [`CryptoAddress`]. Built with
/// [`address`] / [`address_any`].
#[derive(Clone)]
pub struct ZCryptoAddress {
    check: AddressCheck,
    custom_type_error: Option<String>,
    messages: crate::primitives::Messages,
}

impl ZCryptoAddress {
    /// Message used when the input is not a string.
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }

    /// Replace messages by key (`"invalid_crypto_address"`), as
    /// [`ZString::with_messages`](crate::primitives::ZString::with_messages).
    pub fn with_messages<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> Option<String> + Send + Sync + 'static,
    {
        self.messages = Some(Arc::new(f));
        self
    }
}

/// An address valid for `network`; outputs what was detected.
///
/// ```
/// use vld::prelude::*;
/// use vld::crypto::{AddressKind, Network};
///
/// let a = vld::crypto::address(Network::Bitcoin)
///     .parse(r#""BC1QW508D6QEJXTDG4Y5R3ZARVARY0C5XW7KV8F3T4""#)
///     .unwrap();
/// assert_eq!(a.kind, AddressKind::P2wpkh);
/// assert_eq!(a.normalized, "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4");
/// ```
pub fn address(network: impl Into<CryptoNetwork>) -> ZCryptoAddress {
    address_any([network.into()])
}

/// An address valid for any of `networks` (the first that accepts it wins).
pub fn address_any<N: Into<CryptoNetwork>>(
    networks: impl IntoIterator<Item = N>,
) -> ZCryptoAddress {
    ZCryptoAddress {
        check: AddressCheck::new(networks),
        custom_type_error: None,
        messages: None,
    }
}

impl VldSchema for ZCryptoAddress {
    type Output = CryptoAddress;

    fn parse_value(&self, value: &Value) -> Result<CryptoAddress, VldError> {
        let s = value.as_str().ok_or_else(|| {
            crate::primitives::type_error(&self.custom_type_error, "string", value)
        })?;
        self.check.validate(s).map_err(|code| {
            let msg = self
                .messages
                .as_ref()
                .and_then(|f| f("invalid_crypto_address"));
            let mut err = VldError::new();
            err.issues
                .push(ValidationIssue::with_override(code, msg.as_ref()).with_received(value));
            err
        })
    }
}

#[cfg(feature = "openapi")]
impl crate::openapi::JsonSchema for ZCryptoAddress {
    fn json_schema(&self) -> Value {
        let mut schema = serde_json::json!({"type": "string"});
        crate::rules::merge_keywords(&mut schema, self.check.json_schema());
        schema
    }
}

#[cfg(feature = "openapi")]
impl crate::openapi::CollectNestedSchemas for ZCryptoAddress {}
