//! Model: cryptocurrency addresses built from random payloads.
//!
//! - Custom templates: a random template (encoding × checksum × version
//!   bytes × payload length) encodes a random payload; validation must give
//!   the payload back, and a one-character substitution must be rejected
//!   where the code guarantees detection (bech32 polymod; CRC16 / CRC32 over
//!   hex or base64, where one character is a short burst). Elsewhere (hash
//!   checksums, Base58 arithmetic) a substitution must never decode to the
//!   same payload.
//! - Built-in networks: `random_address` for every network × kind × net
//!   validates with that kind / net, and a substitution is caught the same
//!   way (TON's CRC16, bech32, CashAddr, EIP-55 case flips always).
//! - Fake data (feature `fake`): every network's JSON Schema yields valid
//!   addresses.

use proptest::prelude::*;
use vld::crypto::{
    AddressKind as K, Alphabet, Checksum, CryptoAddressReason as R, CryptoNetwork, Network as N,
};

/// A deterministic byte stream from the proptest seed.
fn stream(seed: u64) -> impl FnMut() -> u8 {
    let mut x = seed | 1;
    move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x >> 24) as u8
    }
}

#[derive(Debug, Clone, Copy)]
enum Enc {
    Base58(Alphabet),
    Hex,
    Base64,
    Base64Url,
    Bech32,
    Bech32m,
}

const BECH32: &str = "qpzry9x8gf2tvdw0s3jn54khce6mua7l";

impl Enc {
    /// Characters a substitution may use (one case: no case-only flips).
    fn alphabet(self) -> &'static str {
        match self {
            Enc::Base58(Alphabet::Ripple) => {
                "rpshnaf39wBUDNEGHJKLM4PQRST7VWXYZ2bcdeCg65jkm8oFqi1tuvAxyz"
            }
            Enc::Base58(_) => "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz",
            Enc::Hex => "0123456789abcdef",
            Enc::Base64 => "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/",
            Enc::Base64Url => "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_",
            Enc::Bech32 | Enc::Bech32m => BECH32,
        }
    }
}

fn enc() -> impl Strategy<Value = Enc> {
    prop_oneof![
        Just(Enc::Base58(Alphabet::Bitcoin)),
        Just(Enc::Base58(Alphabet::Ripple)),
        Just(Enc::Base58(Alphabet::Flickr)),
        Just(Enc::Hex),
        Just(Enc::Base64),
        Just(Enc::Base64Url),
        Just(Enc::Bech32),
        Just(Enc::Bech32m),
    ]
}

fn checksum() -> impl Strategy<Value = Checksum> {
    prop_oneof![
        Just(Checksum::None),
        Just(Checksum::Sha256d),
        Just(Checksum::Keccak256),
        Just(Checksum::Ss58),
        Just(Checksum::Crc16Xmodem),
        Just(Checksum::Crc32),
    ]
}

/// Replace character `at` (of the encoded part after `skip` chars) with
/// another one of `alphabet`.
fn substitute(s: &str, skip: usize, at: usize, pick: usize, alphabet: &str) -> String {
    let mut chars: Vec<char> = s.chars().collect();
    let i = skip + at % (chars.len() - skip);
    let old = chars[i].to_ascii_lowercase();
    let pool: Vec<char> = alphabet
        .chars()
        .filter(|c| c.to_ascii_lowercase() != old)
        .collect();
    chars[i] = pool[pick % pool.len()];
    chars.into_iter().collect()
}

proptest! {
    #[test]
    fn custom_template_round_trips_and_catches_a_substitution(
        enc in enc(),
        sum in checksum(),
        version in prop::collection::vec(any::<u8>(), 0..3),
        payload in prop::collection::vec(any::<u8>(), 1..40),
        at in any::<usize>(),
        pick in any::<usize>(),
    ) {
        let base = CryptoNetwork::custom("mine");
        let t = match enc {
            Enc::Base58(a) => base.base58(a),
            Enc::Hex => base.prefix("0x").hex(),
            Enc::Base64 => base.base64(),
            Enc::Base64Url => base.base64url(),
            Enc::Bech32 => base.bech32(&["mine"]),
            Enc::Bech32m => base.bech32m(&["mine"]),
        };
        let t = t
            .checksum(sum)
            .version_bytes(&[&version])
            .payload_len_range(1, 40);
        let bech = matches!(enc, Enc::Bech32 | Enc::Bech32m);
        let want = payload.clone();
        let addr = t.encode(&payload).unwrap();
        let net = CryptoNetwork::from(t.clone());
        let a = net.validate(&addr).unwrap();
        prop_assert_eq!(&a.payload, &want);
        prop_assert_eq!(a.kind, K::Account);
        prop_assert_eq!(a.network, "mine");
        // The JSON Schema pattern (ECMA) accepts it too.
        #[cfg(feature = "openapi")]
        {
            use vld::prelude::*;
            let schema = vld::crypto::address(t.clone()).json_schema();
            let v = jsonschema::draft202012::new(&schema).unwrap();
            prop_assert!(v.is_valid(&serde_json::json!(addr)), "{} {}", addr, schema);
        }

        let skip = match enc { Enc::Hex => 2, Enc::Bech32 | Enc::Bech32m => 5, _ => 0 };
        let flipped = substitute(&addr, skip, at, pick, enc.alphabet());
        let guaranteed = bech
            || (matches!(sum, Checksum::Crc16Xmodem | Checksum::Crc32)
                && matches!(enc, Enc::Hex | Enc::Base64 | Enc::Base64Url));
        match net.validate(&flipped) {
            Err(r) => prop_assert!(r != R::Network && r != R::Kind, "{r:?}"),
            Ok(b) => {
                prop_assert!(!guaranteed, "{} accepted", flipped);
                prop_assert_ne!(b.payload, want);
            }
        }
    }

    #[test]
    fn builtin_random_addresses_validate(seed in any::<u64>(), at in any::<usize>(), pick in any::<usize>()) {
        for &n in N::ALL {
            for testnet in [false, true] {
                let base = CryptoNetwork::from(n);
                let net = if testnet { base.testnet() } else { base.mainnet() };
                let addr = net.random_address(stream(seed)).unwrap();
                let a = net.validate(&addr)
                    .unwrap_or_else(|r| panic!("{n:?} {addr}: {r:?}"));
                let has_testnet = matches!(
                    n,
                    N::Bitcoin | N::Litecoin | N::Dogecoin | N::BitcoinCash | N::Cardano | N::Ton | N::Monero
                );
                prop_assert_eq!(a.testnet, testnet && has_testnet && a.kind != K::Raw, "{} {}", n.name(), addr);

                // A substitution in the encoded part.
                let (skip, alphabet, guaranteed) = match n {
                    N::Ethereum => (2, "0123456789abcdef", false),
                    N::Xrp => (0, Enc::Base58(Alphabet::Ripple).alphabet(), false),
                    N::Ton if a.kind == K::Raw => (2, "0123456789abcdef", false),
                    N::Ton => (0, Enc::Base64Url.alphabet(), true),
                    N::Cardano | N::Cosmos => (addr.find('1').unwrap() + 1, BECH32, true),
                    N::BitcoinCash => (addr.find(':').unwrap() + 1, BECH32, true),
                    N::Bitcoin | N::Litecoin
                        if ["bc1", "tb1", "ltc1", "tltc1"].iter().any(|h| addr.starts_with(h)) =>
                    {
                        (addr.find('1').unwrap() + 1, BECH32, true)
                    }
                    _ => (0, Enc::Base58(Alphabet::Bitcoin).alphabet(), false),
                };
                let flipped = substitute(&addr, skip, at, pick, alphabet);
                match net.validate(&flipped) {
                    Err(_) => {}
                    Ok(b) => {
                        prop_assert!(!guaranteed, "{} {} accepted", n.name(), flipped);
                        prop_assert_ne!(b.payload, a.payload, "{} {}", n.name(), flipped);
                    }
                }
            }
        }
    }

    /// EIP-55: flipping the case of any letter breaks the checksum, unless
    /// the result is single-case (accepted as unchecksummed without
    /// `checksummed()`).
    #[test]
    fn eip55_case_flip_is_caught(seed in any::<u64>(), at in any::<usize>()) {
        let net = CryptoNetwork::from(N::Ethereum);
        let addr = net.random_address(stream(seed)).unwrap();
        let letters: Vec<usize> = addr
            .char_indices()
            .skip(2)
            .filter(|(_, c)| c.is_ascii_alphabetic())
            .map(|(i, _)| i)
            .collect();
        prop_assume!(!letters.is_empty());
        let i = letters[at % letters.len()];
        let mut b = addr.into_bytes();
        b[i] ^= 0x20;
        let flipped = String::from_utf8(b).unwrap();
        let hex = &flipped[2..];
        let single_case = !hex.bytes().any(|c| c.is_ascii_lowercase())
            || !hex.bytes().any(|c| c.is_ascii_uppercase());
        let want = if single_case { Ok(()) } else { Err(R::Checksum) };
        prop_assert_eq!(net.validate(&flipped).map(|_| ()), want);
        prop_assert_eq!(
            N::Ethereum.checksummed().validate(&flipped).map(|_| ()),
            Err(R::Checksum)
        );
    }
}

/// Every kind a network can produce is produced on request, and a kind it
/// cannot produce yields `None`.
#[test]
fn random_address_honours_kinds() {
    let cases: &[(N, &[K])] = &[
        (
            N::Bitcoin,
            &[K::P2pkh, K::P2sh, K::P2wpkh, K::P2wsh, K::P2tr],
        ),
        (
            N::Litecoin,
            &[K::P2pkh, K::P2sh, K::P2wpkh, K::P2wsh, K::P2tr],
        ),
        (N::Dogecoin, &[K::P2pkh, K::P2sh]),
        (N::BitcoinCash, &[K::P2pkh, K::P2sh]),
        (N::Cardano, &[K::Base, K::Pointer, K::Enterprise, K::Reward]),
        (N::Ton, &[K::Bounceable, K::NonBounceable, K::Raw]),
        (N::Monero, &[K::Standard, K::Integrated, K::Subaddress]),
    ];
    for &(n, kinds) in cases {
        for &k in kinds {
            for testnet in [false, true] {
                let net = if testnet { n.testnet() } else { n.mainnet() }.kinds(&[k]);
                let addr = net.random_address(stream(42)).unwrap();
                assert_eq!(net.validate(&addr).unwrap().kind, k, "{n:?} {addr}");
            }
        }
    }
    assert_eq!(
        N::Ethereum.kinds(&[K::P2tr]).random_address(stream(1)),
        None
    );
    // Configured Cosmos hrps and SS58 prefixes (two-byte prefix encoding too).
    for net in [
        CryptoNetwork::cosmos("osmo"),
        CryptoNetwork::ss58(7),
        CryptoNetwork::ss58(1284),
        CryptoNetwork::ss58(16383),
    ] {
        let addr = net.random_address(stream(9)).unwrap();
        assert!(net.validate(&addr).is_ok(), "{} {addr}", net.name());
        assert_eq!(
            CryptoNetwork::from_name(&net.name()).unwrap().name(),
            net.name()
        );
    }
    // The prefix is checked after the checksum: another chain's valid
    // address is a prefix failure.
    let osmo = CryptoNetwork::cosmos("osmo")
        .random_address(stream(3))
        .unwrap();
    assert_eq!(
        N::Cosmos.mainnet().validate(&osmo).map(|_| ()),
        Err(R::Prefix)
    );
    let moon = CryptoNetwork::ss58(1284).random_address(stream(3)).unwrap();
    assert_eq!(
        CryptoNetwork::ss58(1285).validate(&moon).map(|_| ()),
        Err(R::Prefix)
    );
}

#[cfg(feature = "fake")]
#[test]
fn fake_data_validates_for_every_network() {
    use vld::prelude::*;
    let mut nets: Vec<CryptoNetwork> = N::ALL.iter().map(|&n| n.into()).collect();
    nets.extend([
        N::Bitcoin.testnet(),
        N::Bitcoin.kinds(&[K::P2tr]),
        N::Ethereum.checksummed(),
        N::Monero.kinds(&[K::Integrated]),
        CryptoNetwork::cosmos("osmo"),
        CryptoNetwork::ss58(1284),
    ]);
    for net in nets {
        let schema = vld::string().crypto_address(net.clone());
        let js = schema.json_schema();
        for seed in 0..20 {
            let v = vld::fake::fake_value_seeded(&js, seed);
            assert!(schema.parse_value(&v).is_ok(), "{} {v}", net.name());
        }
    }
    // Several networks: each value is valid for one of them.
    let any = vld::string().crypto_address_any([N::Solana, N::Ton, N::Xrp]);
    let js = any.json_schema();
    for seed in 0..20 {
        let v = vld::fake::fake_value_seeded(&js, seed);
        assert!(any.parse_value(&v).is_ok(), "{v}");
    }
}
