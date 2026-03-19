use vld_schemars::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct UserSchema {
        pub name: String  => vld::string().min(2).max(100),
        pub email: String => vld::string().email(),
        pub age: i64      => vld::number().int().min(0).max(150),
    }
}

impl_json_schema!(UserSchema);

fn main() {
    println!("=== vld-schemars example ===\n");

    // 1. vld → schemars
    println!("--- vld → schemars ---");
    let vld_json = UserSchema::json_schema();
    let schemars_schema = vld_to_schemars(&vld_json);
    println!(
        "schemars Schema:\n{}\n",
        serde_json::to_string_pretty(schemars_schema.as_value()).unwrap()
    );

    // 2. schemars → vld (JSON)
    println!("--- schemars → JSON ---");
    let back = schemars_to_json(&schemars_schema);
    println!("Back to JSON:\n{}\n", serde_json::to_string_pretty(&back).unwrap());

    // 3. impl_json_schema! usage
    println!("--- impl_json_schema! ---");
    let mut gen = schemars::SchemaGenerator::default();
    let schema = <UserSchema as schemars::JsonSchema>::json_schema(&mut gen);
    println!("schema_name: {}", <UserSchema as schemars::JsonSchema>::schema_name());
    println!("schema_id:   {}", <UserSchema as schemars::JsonSchema>::schema_id());
    println!(
        "Generated:\n{}\n",
        serde_json::to_string_pretty(schema.as_value()).unwrap()
    );

    // 4. Introspection
    println!("--- Introspection ---");
    let props = list_properties(&vld_json);
    for p in &props {
        println!(
            "  {} (type: {}, required: {})",
            p.name,
            p.schema_type.as_deref().unwrap_or("?"),
            p.required
        );
    }

    // 5. Generate from schemars type
    println!("\n--- generate_from_schemars ---");
    let string_schema = generate_from_schemars::<String>();
    println!(
        "String schema:\n{}",
        serde_json::to_string_pretty(&string_schema).unwrap()
    );

    // 6. Overlay constraints
    println!("\n--- overlay_constraints ---");
    let base = serde_json::json!({"type": "object", "properties": {"name": {"type": "string"}}});
    let extra = serde_json::json!({"properties": {"name": {"minLength": 2}}, "required": ["name"]});
    let merged = overlay_constraints(&base, &extra);
    println!("Merged:\n{}", serde_json::to_string_pretty(&merged).unwrap());

    println!("\n=== Example complete ===");
}
