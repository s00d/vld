//! Example: generate TypeScript Zod/Valibot schemas directly from vld types/schemas.
//!
//! Run:
//! ```sh
//! cargo run -p vld-ts --example generate_zod
//! ```

use vld::schema::VldSchema;
use vld_ts::{
    impl_to_openapi, impl_to_valibot, impl_to_zod, openapi_refs, to_openapi, to_valibot, to_zod,
    ToOpenApi, ToRefs, ToValibot, ToZod,
};

vld::schema! {
    #[derive(Debug)]
    pub struct Address {
        pub city: String => vld::string().min(1),
        pub zip: String => vld::string().min(5).max(10),
    }
}

impl_to_zod!(User);
impl_to_zod!(Address);
impl_to_valibot!(User);
impl_to_valibot!(Address);
impl_to_openapi!(User);
impl_to_openapi!(Address);

vld::schema! {
    #[derive(Debug)]
    pub struct User {
        pub name: String => vld::string().min(2).max(50).describe("User's full name"),
        pub email: String => vld::string().email(),
        pub age: Option<i64> => vld::number().int().min(0).max(150).optional(),
        pub tags: Vec<String> => vld::array(vld::string().min(1)).max_len(10),
        pub address: Address => vld::nested!(Address),
    }
}

fn main() {
    println!("=== Single vld schema -> zod ===\n");
    let single = to_zod(&vld::string().min(2).email());
    println!("const EmailSchema = {};\n", single);

    println!("=== User::to_zod() (single schema expression) ===\n");
    let user_zod = User::to_zod();
    println!("const UserSchema = {};", user_zod);

    println!("\n=== Address::to_zod() (single schema expression) ===\n");
    let address_zod = Address::to_zod();
    println!("const AddressSchema = {};", address_zod);

    println!("\n=== User::to_valibot() (single schema expression) ===\n");
    let user_valibot = User::to_valibot();
    println!("const UserSchema = {};", user_valibot);

    println!("\n=== Address::to_valibot() (single schema expression) ===\n");
    let address_valibot = Address::to_valibot();
    println!("const AddressSchema = {};", address_valibot);

    println!("\n=== Single vld schema -> valibot ===\n");
    let single_valibot = to_valibot(&vld::string().min(2).email());
    println!("const EmailSchema = {};\n", single_valibot);

    println!("\n=== User::to_openapi() (schema object) ===\n");
    let user_schema = User::to_openapi();
    println!("{}", user_schema);

    println!("\n=== Address::to_openapi() (schema object) ===\n");
    let address_schema = Address::to_openapi();
    println!("{}", address_schema);

    println!("\n=== to_openapi(schema) ===\n");
    let email_schema = to_openapi(&vld::string().email());
    println!("{}", email_schema);

    println!("\n=== refs from User::to_openapi() ===\n");
    let refs = openapi_refs(&user_schema);
    println!("{:?}", refs);

    println!("\n=== User::to_refs() ===\n");
    let refs2 = User::to_refs();
    println!("{:?}", refs2);
}
