//! Seeded draws shared by the vld-fake test binaries.
#![allow(dead_code)] // each test binary uses a different subset

use serde_json::Value;

/// Every random draw in the tests is seeded: `VLD_FAKE_SEED` (default fixed)
/// mixed with the test name, so each test sees the same sequence on every run.
/// A failing test prints the base seed and the seed of its last draw.
pub mod seed {
    use std::cell::Cell;
    use std::sync::Once;

    thread_local! {
        static NEXT: Cell<Option<u64>> = const { Cell::new(None) };
        static LAST: Cell<Option<u64>> = const { Cell::new(None) };
    }

    pub fn base() -> u64 {
        std::env::var("VLD_FAKE_SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0x5eed_f00d)
    }

    /// FNV-1a of the test (thread) name.
    fn name_hash() -> u64 {
        let name = std::thread::current().name().unwrap_or("").to_string();
        name.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        })
    }

    fn install_hook() {
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            let prev = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                if let Some(last) = LAST.with(Cell::get) {
                    eprintln!(
                        "vld-fake: VLD_FAKE_SEED={} (last draw: fake_value_seeded(.., {last}))",
                        base()
                    );
                }
                prev(info)
            }));
        });
    }

    pub fn next() -> u64 {
        install_hook();
        NEXT.with(|n| {
            let cur = n.get().unwrap_or_else(|| base() ^ name_hash());
            n.set(Some(
                cur.wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407),
            ));
            LAST.with(|l| l.set(Some(cur)));
            cur
        })
    }
}

/// `vld_fake::fake_value_seeded` with the next per-test seed.
pub fn fake_value(schema: &Value) -> Value {
    vld_fake::fake_value_seeded(schema, seed::next())
}
