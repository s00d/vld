#[allow(unused_imports)]
use vld::prelude::*;
use vld_fake::prelude::*;

vld::schema! {
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct User {
        pub name:  String      => vld::string().min(2).max(30),
        pub email: String      => vld::string().email(),
        pub age:   i64         => vld::number().int().min(18).max(99),
    }
}

vld_fake::impl_fake!(User);

vld::schema! {
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct Address {
        pub city:   String => vld::string().min(1).max(50),
        pub zip:    String => vld::string().min(5).max(10),
        pub street: String => vld::string().min(3).max(100),
    }
}

vld::schema! {
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct UserWithAddress {
        pub name:    String  => vld::string().min(2).max(30),
        pub email:   String  => vld::string().email(),
        pub address: Address => vld::nested(Address::parse_value),
    }
}

vld::schema! {
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct Product {
        pub id:          String         => vld::string().uuid(),
        pub title:       String         => vld::string().min(3).max(120),
        pub price:       f64            => vld::number().min(0.01).max(99999.99),
        pub in_stock:    bool           => vld::boolean(),
        pub tags:        Vec<String>    => vld::array(vld::string().min(1)).min_len(1).max_len(5),
    }
}

vld_fake::impl_fake!(Product);

fn main() {
    println!("=== vld-fake demo ===\n");

    // ── 1. Typed API — User::fake() ───────────────────────────────────
    println!("--- User::fake() — typed access ---");
    let user = User::fake();
    println!("  name:  {}", user.name);
    println!("  email: {}", user.email);
    println!("  age:   {}", user.age);

    // ── 2. Multiple ───────────────────────────────────────────────────
    println!("\n--- User::fake_many(3) ---");
    let users = User::fake_many(3);
    for (i, u) in users.iter().enumerate() {
        println!("  #{}: {} <{}> age={}", i + 1, u.name, u.email, u.age);
    }

    // ── 3. Seeded (reproducible) ──────────────────────────────────────
    println!("\n--- User::fake_seeded(42) — reproducible ---");
    let u1 = User::fake_seeded(42);
    let u2 = User::fake_seeded(42);
    println!("  u1: {} <{}>", u1.name, u1.email);
    println!("  u2: {} <{}>", u2.name, u2.email);
    assert_eq!(u1.name, u2.name);
    println!("  (identical ✓)");

    // ── 4. Product::fake() ────────────────────────────────────────────
    println!("\n--- Product::fake() ---");
    let product = Product::fake();
    println!("  id:       {}", product.id);
    println!("  title:    {}", product.title);
    println!("  price:    {:.2}", product.price);
    println!("  in_stock: {}", product.in_stock);
    println!("  tags:     {:?}", product.tags);

    // ── 5. Untyped API with nested schema template ────────────────────
    println!("\n--- fake_value (nested schema, address template) ---");
    let nested_schema = UserWithAddress::json_schema();
    let val = fake_value(&nested_schema);
    println!("{}", serde_json::to_string_pretty(&val).unwrap());

    // ── 6. Raw JSON Schema ────────────────────────────────────────────
    println!("\n--- Raw JSON Schema ---");
    let raw = serde_json::json!({
        "type": "object",
        "required": ["host", "port", "latitude", "longitude"],
        "properties": {
            "host":      {"type": "string", "format": "ipv4"},
            "port":      {"type": "integer", "minimum": 1, "maximum": 65535},
            "latitude":  {"type": "number"},
            "longitude": {"type": "number"}
        }
    });
    let config = fake_value(&raw);
    println!("  {}", serde_json::to_string(&config).unwrap());

    println!("\n=== done ===");
}
