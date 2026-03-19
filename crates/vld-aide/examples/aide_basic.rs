use schemars::JsonSchema;
use vld_aide::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct CreateUser {
        pub name: String => vld::string().min(2).max(100),
        pub email: String => vld::string().email(),
        pub age: i64 => vld::number().int().min(13).max(150),
    }
}

impl_json_schema!(CreateUser);

vld::schema! {
    #[derive(Debug)]
    pub struct SearchQuery {
        pub q: String => vld::string().min(1).max(200),
        pub page: Option<i64> => vld::number().int().min(1).optional(),
    }
}

impl_json_schema!(SearchQuery);

fn main() {
    println!("=== vld-aide example ===\n");

    println!("CreateUser schema name: {}", CreateUser::schema_name());
    println!("CreateUser schema id:   {}", CreateUser::schema_id());

    let mut gen = schemars::SchemaGenerator::default();
    let schema = <CreateUser as JsonSchema>::json_schema(&mut gen);
    println!(
        "\nCreateUser JSON Schema:\n{}",
        serde_json::to_string_pretty(schema.as_value()).unwrap()
    );

    println!("\nSearchQuery schema name: {}", SearchQuery::schema_name());
    let schema2 = <SearchQuery as JsonSchema>::json_schema(&mut gen);
    println!(
        "\nSearchQuery JSON Schema:\n{}",
        serde_json::to_string_pretty(schema2.as_value()).unwrap()
    );

    // Direct conversion from vld schema value
    println!("\n--- Direct vld_to_schemars conversion ---");
    let vld_schema = vld::string().min(5).max(255).email();
    use vld::json_schema::JsonSchema as VldJsonSchema;
    let js = vld_schema.json_schema();
    let schemars_schema = vld_to_schemars(&js);
    println!(
        "Email field:\n{}",
        serde_json::to_string_pretty(schemars_schema.as_value()).unwrap()
    );
}
