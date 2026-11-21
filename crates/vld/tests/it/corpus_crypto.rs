//! Corpus: cryptocurrency addresses (`vld::crypto`). Every built-in network
//! gets published addresses from its spec / docs (accepted with the detected
//! kind and network) and near misses (one character changed, wrong prefix /
//! hrp / length, mixed-case bech32, bad EIP-55 case) rejected with the exact
//! `reason`. Each row goes through both `string().crypto_address(..)` (one
//! root `invalid_crypto_address` issue with `network` and `reason`) and the
//! typed `vld::crypto::address(..)`.
//!
//! Sources: BIP-173 / BIP-350 test vectors, EIP-55 examples, the CashAddr
//! spec test vectors, CIP-19 test vectors, Substrate's well-known `Alice`
//! account in SS58, the TON docs address example, the Monero general fund
//! address, Solana program ids, XRPL special accounts.

use serde_json::json;
use vld::crypto::{AddressKind as K, CryptoAddressReason as R, CryptoNetwork, Network as N};
use vld::error::IssueCode;
use vld::prelude::*;

/// `(network, input, Ok((kind, testnet)) or Err(reason))`.
type Row = (CryptoNetwork, &'static str, Result<(K, bool), R>);

fn rows() -> Vec<Row> {
    let n = |n: N| CryptoNetwork::from(n);
    let ok = |k: K| Ok((k, false));
    let test = |k: K| Ok((k, true));
    vec![
        // ── Bitcoin: Base58Check ──
        (
            n(N::Bitcoin),
            "1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2",
            ok(K::P2pkh),
        ),
        (
            n(N::Bitcoin),
            "3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy",
            ok(K::P2sh),
        ),
        (
            n(N::Bitcoin),
            "1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN3",
            Err(R::Checksum),
        ),
        (
            n(N::Bitcoin),
            "1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN0",
            Err(R::Encoding),
        ),
        (n(N::Bitcoin), "1BvBMSEYstWetqTFn5Au4m", Err(R::Length)),
        // TRON's version byte 0x41 is not Bitcoin's.
        (
            n(N::Bitcoin),
            "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t",
            Err(R::Prefix),
        ),
        // ── Bitcoin: SegWit (BIP-173 valid vectors) ──
        (
            n(N::Bitcoin),
            "BC1QW508D6QEJXTDG4Y5R3ZARVARY0C5XW7KV8F3T4",
            ok(K::P2wpkh),
        ),
        (
            n(N::Bitcoin),
            "tb1qrp33g0q5c5txsp9arysrx4k6zdkfs4nce4xj0gdcccefvpysxf3q0sl5k7",
            test(K::P2wsh),
        ),
        (
            n(N::Bitcoin),
            "tb1qqqqqp399et2xygdj5xreqhjjvcmzhxw4aywxecjdzew6hylgvsesrxh6hy",
            test(K::P2wsh),
        ),
        // ── Bitcoin: Taproot and future versions (BIP-350 valid vectors) ──
        (
            n(N::Bitcoin),
            "bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqzk5jj0",
            ok(K::P2tr),
        ),
        (
            n(N::Bitcoin),
            "tb1pqqqqp399et2xygdj5xreqhjjvcmzhxw4aywxecjdzew6hylgvsesf3hn0c",
            test(K::P2tr),
        ),
        (
            n(N::Bitcoin),
            "bc1pw508d6qejxtdg4y5r3zarvary0c5xw7kw508d6qejxtdg4y5r3zarvary0c5xw7kt5nd6y",
            ok(K::WitnessUnknown),
        ),
        (n(N::Bitcoin), "BC1SW50QGDZ25J", ok(K::WitnessUnknown)),
        (
            n(N::Bitcoin),
            "bc1zw508d6qejxtdg4y5r3zarvaryvaxxpcs",
            ok(K::WitnessUnknown),
        ),
        // ── Bitcoin: BIP-350 invalid vectors ──
        (
            n(N::Bitcoin),
            "tc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vq5zuyut",
            Err(R::Prefix),
        ),
        (
            n(N::Bitcoin),
            "bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqh2y7hd",
            Err(R::Checksum),
        ),
        (
            n(N::Bitcoin),
            "BC1S0XLXVLHEMJA6C4DQV22UAPCTQUPFHLXM9H8Z3K2E72Q4K9HCZ7VQ54WELL",
            Err(R::Checksum),
        ),
        (
            n(N::Bitcoin),
            "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kemeawh",
            Err(R::Checksum),
        ),
        (
            n(N::Bitcoin),
            "bc1p38j9r5y49hruaue7wxjce0updqjuyyx0kh56v8s25huc6995vvpql3jow4",
            Err(R::Encoding),
        ),
        (
            n(N::Bitcoin),
            "BC130XLXVLHEMJA6C4DQV22UAPCTQUPFHLXM9H8Z3K2E72Q4K9HCZ7VQ7ZWS8R",
            Err(R::Encoding),
        ),
        (n(N::Bitcoin), "bc1pw5dgrnzv", Err(R::Length)),
        (
            n(N::Bitcoin),
            "bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7v8n0nx0muaewav253zgeav",
            Err(R::Length),
        ),
        (
            n(N::Bitcoin),
            "BC1QR508D6QEJXTDG4Y5R3ZARVARYV98GJ9P",
            Err(R::Length),
        ),
        (
            n(N::Bitcoin),
            "tb1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vq47Zagq",
            Err(R::Case),
        ),
        (
            n(N::Bitcoin),
            "bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7v07qwwzcrf",
            Err(R::Encoding),
        ),
        (
            n(N::Bitcoin),
            "tb1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vpggkg4j",
            Err(R::Encoding),
        ),
        (n(N::Bitcoin), "bc1gmk9yu", Err(R::Length)),
        // Options: mainnet only, kinds.
        (
            n(N::Bitcoin).mainnet(),
            "tb1qrp33g0q5c5txsp9arysrx4k6zdkfs4nce4xj0gdcccefvpysxf3q0sl5k7",
            Err(R::Network),
        ),
        (
            N::Bitcoin.testnet(),
            "1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2",
            Err(R::Network),
        ),
        (
            N::Bitcoin.kinds(&[K::P2wpkh, K::P2tr]),
            "3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy",
            Err(R::Kind),
        ),
        // ── Litecoin ──
        (
            n(N::Litecoin),
            "LVg2kJoFNg45Nbpy53h7Fe1wKyeXVRhMH9",
            ok(K::P2pkh),
        ),
        (
            n(N::Litecoin),
            "3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy",
            ok(K::P2sh),
        ),
        (
            n(N::Litecoin),
            "1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2",
            Err(R::Prefix),
        ),
        (
            n(N::Litecoin),
            "BC1QW508D6QEJXTDG4Y5R3ZARVARY0C5XW7KV8F3T4",
            Err(R::Prefix),
        ),
        // ── Dogecoin ──
        (
            n(N::Dogecoin),
            "DH5yaieqoZN36fDVciNyRueRGvGLR3mr7L",
            ok(K::P2pkh),
        ),
        (
            n(N::Dogecoin),
            "DH5yaieqoZN36fDVciNyRueRGvGLR3mr7M",
            Err(R::Checksum),
        ),
        (
            n(N::Dogecoin),
            "1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2",
            Err(R::Prefix),
        ),
        // ── Bitcoin Cash: CashAddr spec vectors ──
        (
            n(N::BitcoinCash),
            "bitcoincash:qr6m7j9njldwwzlg9v7v53unlr4jkmx6eylep8ekg2",
            ok(K::P2pkh),
        ),
        (
            n(N::BitcoinCash),
            "qr6m7j9njldwwzlg9v7v53unlr4jkmx6eylep8ekg2",
            ok(K::P2pkh),
        ),
        (
            n(N::BitcoinCash),
            "BITCOINCASH:QR6M7J9NJLDWWZLG9V7V53UNLR4JKMX6EYLEP8EKG2",
            ok(K::P2pkh),
        ),
        (
            n(N::BitcoinCash),
            "bchtest:pr6m7j9njldwwzlg9v7v53unlr4jkmx6eyvwc0uz5t",
            test(K::P2sh),
        ),
        (
            n(N::BitcoinCash),
            "bitcoincash:q9adhakpwzztepkpwp5z0dq62m6u5v5xtyj7j3h2ws4mr9g0",
            ok(K::P2pkh),
        ),
        (
            n(N::BitcoinCash),
            "bchtest:p9adhakpwzztepkpwp5z0dq62m6u5v5xtyj7j3h2u94tsynr",
            test(K::P2sh),
        ),
        (
            n(N::BitcoinCash),
            "bitcoincash:qvch8mmxy0rtfrlarg7ucrxxfzds5pamg73h7370aa87d80gyhqxq5nlegake",
            ok(K::P2pkh),
        ),
        (
            n(N::BitcoinCash),
            "1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2",
            ok(K::P2pkh),
        ),
        (
            n(N::BitcoinCash),
            "bitcoincash:qr6m7j9njldwwzlg9v7v53unlr4jkmx6eylep8ekg3",
            Err(R::Checksum),
        ),
        (
            n(N::BitcoinCash),
            "pref:pr6m7j9njldwwzlg9v7v53unlr4jkmx6ey65nvtks5",
            Err(R::Prefix),
        ),
        (
            n(N::BitcoinCash),
            "bitcoincash:Qr6m7j9njldwwzlg9v7v53unlr4jkmx6eylep8ekg2",
            Err(R::Case),
        ),
        (
            n(N::BitcoinCash),
            "bitcoincash:qr6m7j9njldwwzlg9v7v53unlr",
            Err(R::Length),
        ),
        // ── Ethereum: EIP-55 examples ──
        (
            n(N::Ethereum),
            "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed",
            ok(K::Account),
        ),
        (
            n(N::Ethereum),
            "0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359",
            ok(K::Account),
        ),
        (
            n(N::Ethereum),
            "0xdbF03B407c01E7cD3CBea99509d93f8DDDC8C6FB",
            ok(K::Account),
        ),
        (
            n(N::Ethereum),
            "0xD1220A0cf47c7B9Be7A2E6BA89F429762e7b9aDb",
            ok(K::Account),
        ),
        (
            n(N::Ethereum),
            "0x52908400098527886E0F7030069857D2E4169EE7",
            ok(K::Account),
        ),
        (
            n(N::Ethereum),
            "0xde709f2102306220921060314715629080e2fb77",
            ok(K::Account),
        ),
        (
            n(N::Ethereum),
            "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAeD",
            Err(R::Checksum),
        ),
        // EIP-55's "all lower" example: its checksum has no uppercase letter.
        (
            N::Ethereum.checksummed(),
            "0xde709f2102306220921060314715629080e2fb77",
            ok(K::Account),
        ),
        (
            N::Ethereum.checksummed(),
            "0x5aaeb6053f3e94c9b9a09f33669435e7ef1beaed",
            Err(R::Checksum),
        ),
        (
            n(N::Ethereum),
            "5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed",
            Err(R::Prefix),
        ),
        (
            n(N::Ethereum),
            "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAe",
            Err(R::Length),
        ),
        (
            n(N::Ethereum),
            "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAeg",
            Err(R::Encoding),
        ),
        // ── TRON ──
        (
            n(N::Tron),
            "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t",
            ok(K::Account),
        ),
        (
            n(N::Tron),
            "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6u",
            Err(R::Checksum),
        ),
        (
            n(N::Tron),
            "1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2",
            Err(R::Prefix),
        ),
        // ── Solana: program ids (no checksum: only the 32-byte length) ──
        (
            n(N::Solana),
            "11111111111111111111111111111111",
            ok(K::Account),
        ),
        (
            n(N::Solana),
            "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
            ok(K::Account),
        ),
        (
            n(N::Solana),
            "So11111111111111111111111111111111111111112",
            ok(K::Account),
        ),
        (
            n(N::Solana),
            "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5D0",
            Err(R::Encoding),
        ),
        (
            n(N::Solana),
            "1111111111111111111111111111111",
            Err(R::Length),
        ),
        // ── XRP ──
        (
            n(N::Xrp),
            "rHb9CJAWyB4rj91VRWn96DkukG4bwdtyTh",
            ok(K::Account),
        ),
        (n(N::Xrp), "rrrrrrrrrrrrrrrrrrrrrhoLvTp", ok(K::Account)),
        (n(N::Xrp), "rrrrrrrrrrrrrrrrrrrrBZbvji", ok(K::Account)),
        (
            n(N::Xrp),
            "rHb9CJAWyB4rj91VRWn96DkukG4bwdtyTi",
            Err(R::Checksum),
        ),
        // ── Cardano: CIP-19 test vectors ──
        (
            n(N::Cardano),
            "addr1qx2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3n0d3vllmyqwsx5wktcd8cc3sq835lu7drv2xwl2wywfgse35a3x",
            ok(K::Base),
        ),
        (
            n(N::Cardano),
            "addr1z8phkx6acpnf78fuvxn0mkew3l0fd058hzquvz7w36x4gten0d3vllmyqwsx5wktcd8cc3sq835lu7drv2xwl2wywfgs9yc0hh",
            ok(K::Base),
        ),
        (
            n(N::Cardano),
            "addr1gx2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer5pnz75xxcrzqf96k",
            ok(K::Pointer),
        ),
        (
            n(N::Cardano),
            "addr1vx2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzers66hrl8",
            ok(K::Enterprise),
        ),
        (
            n(N::Cardano),
            "stake1uyehkck0lajq8gr28t9uxnuvgcqrc6070x3k9r8048z8y5gh6ffgw",
            ok(K::Reward),
        ),
        (
            n(N::Cardano),
            "addr_test1qz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3n0d3vllmyqwsx5wktcd8cc3sq835lu7drv2xwl2wywfgs68faae",
            test(K::Base),
        ),
        (
            n(N::Cardano),
            "addr_test1vz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzerspjrlsz",
            test(K::Enterprise),
        ),
        (
            n(N::Cardano),
            "addr1vx2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzers66hrl9",
            Err(R::Checksum),
        ),
        (
            N::Cardano.mainnet(),
            "addr_test1vz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzerspjrlsz",
            Err(R::Network),
        ),
        (
            n(N::Cardano),
            "BC1QW508D6QEJXTDG4Y5R3ZARVARY0C5XW7KV8F3T4",
            Err(R::Prefix),
        ),
        // ── Cosmos ──
        (
            n(N::Cosmos),
            "cosmos1qypqxpq9qcrsszg2pvxq6rs0zqg3yyc5lzv7xu",
            ok(K::Account),
        ),
        (
            n(N::Cosmos),
            "cosmos1qypqxpq9qcrsszg2pvxq6rs0zqg3yyc5lzv7xv",
            Err(R::Checksum),
        ),
        (
            n(N::Cosmos),
            "BC1QW508D6QEJXTDG4Y5R3ZARVARY0C5XW7KV8F3T4",
            Err(R::Prefix),
        ),
        (
            CryptoNetwork::cosmos("osmo"),
            "cosmos1qypqxpq9qcrsszg2pvxq6rs0zqg3yyc5lzv7xu",
            Err(R::Prefix),
        ),
        // ── SS58: Alice on Polkadot / Kusama / generic Substrate ──
        (
            n(N::Polkadot),
            "15oF4uVJwmo4TdGW7VfQxNLavjCXviqxT9S1MgbjMNHr6Sp5",
            ok(K::Account),
        ),
        (
            n(N::Kusama),
            "HNZata7iMYWmk5RvZRTiAsSDhV8366zq2YGb3tLH5Upf74F",
            ok(K::Account),
        ),
        (
            n(N::Substrate),
            "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
            ok(K::Account),
        ),
        (
            n(N::Substrate),
            "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQZ",
            Err(R::Checksum),
        ),
        (
            n(N::Kusama),
            "15oF4uVJwmo4TdGW7VfQxNLavjCXviqxT9S1MgbjMNHr6Sp5",
            Err(R::Prefix),
        ),
        (n(N::Polkadot), "15oF4uVJwmo4TdGW7Vf", Err(R::Length)),
        // ── TON: docs example, bounceable / non-bounceable / raw ──
        (
            n(N::Ton),
            "EQDKbjIcfM6ezt8KjKJJLshZJJSqX7XOA4ff-W72r5gqPrHF",
            ok(K::Bounceable),
        ),
        (
            n(N::Ton),
            "UQDKbjIcfM6ezt8KjKJJLshZJJSqX7XOA4ff-W72r5gqPuwA",
            ok(K::NonBounceable),
        ),
        (
            n(N::Ton),
            "0:ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e",
            ok(K::Raw),
        ),
        (
            n(N::Ton),
            "EQDKbjIcfM6ezt8KjKJJLshZJJSqX7XOA4ff-W72r5gqPrHG",
            Err(R::Checksum),
        ),
        (
            n(N::Ton),
            "EQDKbjIcfM6ezt8KjKJJLshZJJSqX7XOA4ff-W72r5gqPrH",
            Err(R::Length),
        ),
        (
            n(N::Ton),
            "0:ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3",
            Err(R::Length),
        ),
        (
            n(N::Ton),
            "-1:ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e",
            ok(K::Raw),
        ),
        // the workchain in canonical decimal only, as the schema pattern
        (
            n(N::Ton),
            "+0:ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e",
            Err(R::Prefix),
        ),
        (
            n(N::Ton),
            "00:ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e",
            Err(R::Prefix),
        ),
        (
            n(N::Ton),
            "-0:ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e",
            Err(R::Prefix),
        ),
        // ── Monero: the general fund address ──
        (
            n(N::Monero),
            "44AFFq5kSiGBoZ4NMDwYtN18obc8AemS33DBLWs3H7otXft3XjrpDtQGv7SqSsaBYBb98uNbr2VBBEt7f2wfn3RVGQBEP3A",
            ok(K::Standard),
        ),
        (
            n(N::Monero),
            "44AFFq5kSiGBoZ4NMDwYtN18obc8AemS33DBLWs3H7otXft3XjrpDtQGv7SqSsaBYBb98uNbr2VBBEt7f2wfn3RVGQBEP3B",
            Err(R::Checksum),
        ),
        (
            n(N::Monero),
            "44AFFq5kSiGBoZ4NMDwYtN18obc8AemS33DBLWs3H7otXft3XjrpDtQGv7SqSsaBYBb98uNbr2VBBEt7f2wfn3RVGQBEP3",
            Err(R::Length),
        ),
    ]
}

#[test]
fn every_network_accepts_its_vectors_and_rejects_near_misses() {
    for (net, input, want) in rows() {
        let name = net.name();
        let got = vld::crypto::address(net.clone()).parse_value(&json!(input));
        let via_string = vld::string()
            .crypto_address(net.clone())
            .parse_value(&json!(input));
        match want {
            Ok((kind, testnet)) => {
                let a = got.unwrap_or_else(|e| panic!("{name} {input}: {:?}", e.issues[0].code));
                assert_eq!((a.kind, a.testnet), (kind, testnet), "{name} {input}");
                assert_eq!(a.network, name);
                assert_eq!(via_string.unwrap(), input);
                // The emitted JSON Schema (ECMA `pattern`) is never stricter.
                #[cfg(feature = "openapi")]
                {
                    let schema = vld::crypto::address(net.clone()).json_schema();
                    let v = jsonschema::draft202012::new(&schema).unwrap();
                    assert!(v.is_valid(&json!(input)), "{name} {input}: {schema}");
                }
                // The normalized form is itself valid.
                assert!(
                    net.validate(&a.normalized).is_ok(),
                    "{name} {}",
                    a.normalized
                );
            }
            Err(reason) => {
                let expected = IssueCode::InvalidCryptoAddress {
                    network: name.clone(),
                    reason,
                };
                for err in [got.map(|_| ()), via_string.map(|_| ())] {
                    let err = err.expect_err(input);
                    assert_eq!(err.issues.len(), 1, "{name} {input}");
                    assert_eq!(err.issues[0].code, expected, "{name} {input}");
                    assert!(err.issues[0].path.is_empty());
                    assert_eq!(err.issues[0].received, Some(json!(input)));
                }
            }
        }
    }
}

/// Canonical forms: EIP-55 case, lowercase bech32 / CashAddr with prefix.
#[test]
fn normalized_forms() {
    let norm = |n: CryptoNetwork, s: &str| n.validate(s).unwrap().normalized;
    assert_eq!(
        norm(
            N::Ethereum.into(),
            "0x5aaeb6053f3e94c9b9a09f33669435e7ef1beaed"
        ),
        "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed"
    );
    assert_eq!(
        norm(
            N::Bitcoin.into(),
            "BC1QW508D6QEJXTDG4Y5R3ZARVARY0C5XW7KV8F3T4"
        ),
        "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4"
    );
    assert_eq!(
        norm(
            N::BitcoinCash.into(),
            "qr6m7j9njldwwzlg9v7v53unlr4jkmx6eylep8ekg2"
        ),
        "bitcoincash:qr6m7j9njldwwzlg9v7v53unlr4jkmx6eylep8ekg2"
    );
    // The CashAddr payload is the spec vector's hash.
    let a = CryptoNetwork::from(N::BitcoinCash)
        .validate("bitcoincash:qr6m7j9njldwwzlg9v7v53unlr4jkmx6eylep8ekg2")
        .unwrap();
    assert_eq!(
        a.payload,
        [
            0xF5, 0xBF, 0x48, 0xB3, 0x97, 0xDA, 0xE7, 0x0B, 0xE8, 0x2B, 0x3C, 0xCA, 0x47, 0x93,
            0xF8, 0xEB, 0x2B, 0x6C, 0xDA, 0xC9
        ]
    );
}

/// `crypto_address_any`: the first accepting network wins; a failure names
/// the network whose checksum failed, or every network when none matched.
#[test]
fn any_of_several_networks() {
    let s = vld::string().crypto_address_any([N::Bitcoin, N::Ethereum]);
    assert!(
        s.parse(r#""0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed""#)
            .is_ok()
    );
    assert!(s.parse(r#""1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2""#).is_ok());
    let code = |input: &str| {
        s.parse_value(&json!(input)).unwrap_err().issues[0]
            .code
            .clone()
    };
    assert_eq!(
        code("0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAeD"),
        IssueCode::InvalidCryptoAddress {
            network: "ethereum".into(),
            reason: R::Checksum
        }
    );
    assert_eq!(
        code("hello"),
        IssueCode::InvalidCryptoAddress {
            network: "bitcoin|ethereum".into(),
            reason: R::Length
        }
    );
    let typed = vld::crypto::address_any([N::Solana, N::Tron]);
    let a = typed
        .parse(r#""TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t""#)
        .unwrap();
    // A TRON address is also 32+ Base58 chars, but not 32 bytes: TRON wins.
    assert_eq!(a.network, "tron");
}

/// JSON Schema keywords and the generated Zod / Valibot: the shape as a
/// regex, plus a note that the checksum is not checked there.
#[cfg(feature = "ts")]
#[test]
fn json_schema_and_codegen() {
    let s = vld::string().crypto_address(N::Ethereum.checksummed());
    assert_eq!(
        s.json_schema(),
        json!({
            "type": "string",
            "format": "crypto-address",
            "pattern": "^(?:0x[0-9a-fA-F]{40})$",
            "x-network": "ethereum",
            "x-checksummed": true
        })
    );
    let any = vld::string().crypto_address_any([N::Tron, N::Xrp]);
    assert_eq!(any.json_schema()["x-network"], json!(["tron", "xrp"]));
    let zod = vld::ts::to_zod(&s);
    assert_eq!(
        zod,
        "z.string() /* crypto address: shape only, checksum not checked */.regex(new RegExp(\"^(?:0x[0-9a-fA-F]{40})$\"))"
    );
    assert!(vld::ts::to_valibot(&s).contains("checksum not checked"));
}
