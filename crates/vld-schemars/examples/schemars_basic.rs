use vld_schemars::prelude::*;

// === Forward: vld → schemars ===

vld::schema! {
    #[derive(Debug)]
    pub struct UserSchema {
        pub name: String  => vld::string().min(2).max(100),
        pub email: String => vld::string().email(),
        pub age: i64      => vld::number().int().min(0).max(150),
    }
}

impl_json_schema!(UserSchema);

// === Reverse: schemars → vld ===

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct Product {
    name: String,
    price: f64,
    in_stock: bool,
}

impl_vld_parse!(Product);

fn main() {
    println!("=== vld-schemars example ===\n");

    // 1. Forward: vld type → schemars::JsonSchema
    println!("--- impl_json_schema! (vld → schemars) ---");
    let mut gen = schemars::SchemaGenerator::default();
    let schema = <UserSchema as schemars::JsonSchema>::json_schema(&mut gen);
    println!("schema_name: {}", <UserSchema as schemars::JsonSchema>::schema_name());
    println!(
        "Generated:\n{}\n",
        serde_json::to_string_pretty(schema.as_value()).unwrap()
    );

    // 2. Reverse: schemars type → vld validation via trait
    println!("--- impl_vld_parse! (schemars → vld) ---");

    // 2a. Validate existing instance
    let product = Product {
        name: "Widget".into(),
        price: 9.99,
        in_stock: true,
    };
    match product.vld_validate() {
        Ok(()) => println!("[OK] product.vld_validate() passed"),
        Err(e) => println!("[ERR] {}", e),
    }

    // 2b. Validate JSON against type's schema
    let json = serde_json::json!({"name": "Gadget", "price": 19.99, "in_stock": false});
    match Product::vld_validate_json(&json) {
        Ok(()) => println!("[OK] Product::vld_validate_json() passed"),
        Err(e) => println!("[ERR] {}", e),
    }

    // 2c. Validate + deserialize
    let parsed = Product::vld_parse(&json).unwrap();
    println!("[OK] Product::vld_parse() -> {:?}", parsed);

    // 2d. VldParse (for framework extractors)
    use vld::schema::VldParse;
    let parsed = Product::vld_parse_value(&json).unwrap();
    println!("[OK] Product::vld_parse_value() -> {:?}", parsed);

    // 3. Introspection
    println!("\n--- Introspection ---");
    let vld_json = UserSchema::json_schema();
    let props = list_properties(&vld_json);
    for p in &props {
        println!(
            "  {} (type: {}, required: {})",
            p.name,
            p.schema_type.as_deref().unwrap_or("?"),
            p.required
        );
    }

    println!("\n=== Example complete ===");
}
