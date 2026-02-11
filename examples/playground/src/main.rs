use vld::prelude::*;

// ---------------------------------------------------------------------------
// 1. Define schemas via macros
// ---------------------------------------------------------------------------

vld::schema! {
    #[derive(Debug, serde::Serialize, Default)]
    pub struct Address {
        pub city: String => vld::string().min(1),
        pub zip: String => vld::string().len(6),
    }
}

vld::schema! {
    #[derive(Debug, serde::Serialize)]
    pub struct User {
        pub name: String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
        pub age: Option<i64> => vld::number().int().gte(18).optional(),
        pub role: String => vld::enumeration(&["admin", "user", "moderator"])
            .with_default("user".to_string()),
        /// Nickname: at least 3 characters.
        /// If invalid, falls back to "anonymous" (catch).
        pub nickname: String => vld::string().min(3).max(20)
            .catch("anonymous".to_string()),
        pub tags: Vec<String> => vld::array(vld::string().non_empty()).max_len(5)
            .with_default(vec![]),
        pub address: Address => vld::nested(Address::parse_value),
    }
}

// Enable per-field validation + parse_lenient + save_to_file
vld::impl_validate_fields!(User {
    name     : String        => vld::string().min(2).max(50),
    email    : String        => vld::string().email(),
    age      : Option<i64>   => vld::number().int().gte(18).optional(),
    role     : String        => vld::enumeration(&["admin", "user", "moderator"]).with_default("user".to_string()),
    nickname : String        => vld::string().min(3).max(20).catch("anonymous".to_string()),
    tags     : Vec<String>   => vld::array(vld::string().non_empty()).max_len(5).with_default(vec![]),
    address  : Address       => vld::nested(Address::parse_value),
});

fn main() {
    println!("=== vld playground ===\n");

    // ---------------------------------------------------------------------------
    // 2. Primitives
    // ---------------------------------------------------------------------------
    println!("--- Primitives ---");

    let s = vld::string().min(3).email();
    match s.parse(r#""test@example.com""#) {
        Ok(v) => println!("[OK] email: {}", v),
        Err(e) => println!("[ERR] email: {}", e),
    }

    let n = vld::number().int().min(0).max(100);
    match n.parse("42") {
        Ok(v) => println!("[OK] int: {}", v),
        Err(e) => println!("[ERR] int: {}", e),
    }

    let b = vld::boolean().coerce();
    match b.parse(r#""true""#) {
        Ok(v) => println!("[OK] bool coerce: {}", v),
        Err(e) => println!("[ERR] bool coerce: {}", e),
    }

    // ---------------------------------------------------------------------------
    // 3. Literals and enumerations
    // ---------------------------------------------------------------------------
    println!("\n--- Literals and enumerations ---");

    let lit = vld::literal("admin");
    match lit.parse(r#""admin""#) {
        Ok(v) => println!("[OK] literal \"admin\": {}", v),
        Err(e) => println!("[ERR] literal: {}", e),
    }
    match lit.parse(r#""user""#) {
        Ok(v) => println!("[OK] literal \"user\": {}", v),
        Err(e) => println!("[ERR] literal \"user\" rejected: {}", e),
    }

    let role_enum = vld::enumeration(&["admin", "user", "moderator"]);
    match role_enum.parse(r#""admin""#) {
        Ok(v) => println!("[OK] enum: {}", v),
        Err(e) => println!("[ERR] enum: {}", e),
    }
    match role_enum.parse(r#""hacker""#) {
        Ok(_) => println!("[OK] enum: hacker"),
        Err(e) => println!("[ERR] enum \"hacker\": {}", e),
    }

    let lit_int = vld::literal(42i64);
    println!("[OK] literal(42) = {:?}", lit_int.parse("42"));
    println!("[ERR] literal(42) vs 43 = {:?}", lit_int.parse("43"));

    let any_schema = vld::any();
    println!("[OK] any(null) = {:?}", any_schema.parse("null"));
    println!("[OK] any([1,2]) = {:?}", any_schema.parse("[1,2]"));

    // ---------------------------------------------------------------------------
    // 4. String formats
    // ---------------------------------------------------------------------------
    println!("\n--- String formats ---");

    let ipv4 = vld::string().ipv4();
    println!("[OK] ipv4: {:?}", ipv4.parse(r#""192.168.1.1""#));
    println!("[ERR] ipv4: {:?}", ipv4.parse(r#""999.0.0.1""#));

    let ipv6 = vld::string().ipv6();
    println!(
        "[OK] ipv6: {:?}",
        ipv6.parse(r#""2001:0db8:85a3:0000:0000:8a2e:0370:7334""#)
    );

    let b64 = vld::string().base64();
    println!("[OK] base64: {:?}", b64.parse(r#""SGVsbG8=""#));
    println!("[ERR] base64: {:?}", b64.parse(r#""not base64!""#));

    let iso_date = vld::string().iso_date();
    println!("[OK] iso_date: {:?}", iso_date.parse(r#""2024-01-15""#));
    println!("[ERR] iso_date: {:?}", iso_date.parse(r#""2024-1-5""#));

    let iso_dt = vld::string().iso_datetime();
    println!(
        "[OK] iso_datetime: {:?}",
        iso_dt.parse(r#""2024-01-15T10:30:00Z""#)
    );

    let iso_t = vld::string().iso_time();
    println!("[OK] iso_time: {:?}", iso_t.parse(r#""10:30:00""#));
    println!("[ERR] iso_time: {:?}", iso_t.parse(r#""25:00""#));

    let host = vld::string().hostname();
    println!("[OK] hostname: {:?}", host.parse(r#""example.com""#));
    println!("[ERR] hostname: {:?}", host.parse(r#""-invalid.com""#));

    // ---------------------------------------------------------------------------
    // 5. Transforms
    // ---------------------------------------------------------------------------
    println!("\n--- Transforms ---");

    let trimmed = vld::string().trim().to_lowercase();
    match trimmed.parse(r#""  Hello World  ""#) {
        Ok(v) => println!("[OK] trim+lowercase: \"{}\"", v),
        Err(e) => println!("[ERR] trim+lowercase: {}", e),
    }

    let len_schema = vld::string().transform(|s| s.len());
    match len_schema.parse(r#""hello""#) {
        Ok(v) => println!("[OK] string->len: {}", v),
        Err(e) => println!("[ERR] string->len: {}", e),
    }

    // ---------------------------------------------------------------------------
    // 6. Modifiers
    // ---------------------------------------------------------------------------
    println!("\n--- Modifiers ---");

    let optional = vld::string().optional();
    println!("[OK] optional(null): {:?}", optional.parse("null").unwrap());
    println!(
        "[OK] optional(\"hi\"): {:?}",
        optional.parse(r#""hi""#).unwrap()
    );

    let with_default = vld::string().with_default("fallback".to_string());
    println!(
        "[OK] default(null): {:?}",
        with_default.parse("null").unwrap()
    );
    println!(
        "[OK] default(\"ok\"): {:?}",
        with_default.parse(r#""ok""#).unwrap()
    );

    let nullish = vld::string().nullish();
    println!("[OK] nullish(null): {:?}", nullish.parse("null").unwrap());
    println!(
        "[OK] nullish(\"hi\"): {:?}",
        nullish.parse(r#""hi""#).unwrap()
    );

    let catch = vld::string().min(10).catch("default-value".to_string());
    println!(
        "[OK] catch(\"short\"): {:?}",
        catch.parse(r#""short""#).unwrap()
    );
    println!(
        "[OK] catch(\"long enough!\"): {:?}",
        catch.parse(r#""long enough!""#).unwrap()
    );

    // ---------------------------------------------------------------------------
    // 7. Arrays and tuples
    // ---------------------------------------------------------------------------
    println!("\n--- Arrays and tuples ---");

    let arr = vld::array(vld::number().int().positive())
        .min_len(1)
        .max_len(5);
    match arr.parse("[1, 2, 3]") {
        Ok(v) => println!("[OK] array: {:?}", v),
        Err(e) => println!("[ERR] array: {}", e),
    }

    match arr.parse("[]") {
        Ok(v) => println!("[OK] empty array: {:?}", v),
        Err(e) => println!("[ERR] empty array: {}", e),
    }

    // Tuple schema (string, number)
    let tuple_schema = (vld::string(), vld::number().int());
    match tuple_schema.parse(r#"["hello", 42]"#) {
        Ok(v) => println!("[OK] tuple(string, int): {:?}", v),
        Err(e) => println!("[ERR] tuple: {}", e),
    }

    // Tuple schema (string, number, boolean)
    let tuple3 = (vld::string(), vld::number(), vld::boolean());
    match tuple3.parse(r#"["hi", 3.14, true]"#) {
        Ok(v) => println!("[OK] tuple3: {:?}", v),
        Err(e) => println!("[ERR] tuple3: {}", e),
    }

    // ---------------------------------------------------------------------------
    // 8. Record (dictionary)
    // ---------------------------------------------------------------------------
    println!("\n--- Record ---");

    let rec = vld::record(vld::number().int().positive())
        .min_keys(1)
        .max_keys(3);
    match rec.parse(r#"{"a": 1, "b": 2, "c": 3}"#) {
        Ok(map) => println!("[OK] record: {:?}", map),
        Err(e) => println!("[ERR] record: {}", e),
    }

    match rec.parse(r#"{}"#) {
        Ok(_) => println!("[OK] empty record"),
        Err(e) => println!("[ERR] empty record: {}", e),
    }

    // ---------------------------------------------------------------------------
    // 9. Union
    // ---------------------------------------------------------------------------
    println!("\n--- Union ---");

    let string_or_int = vld::union(vld::string(), vld::number().int());
    match string_or_int.parse(r#""hello""#) {
        Ok(v) => println!("[OK] union(string|int) <- \"hello\": left={}", v.is_left()),
        Err(e) => println!("[ERR] union: {}", e),
    }
    match string_or_int.parse("42") {
        Ok(v) => println!("[OK] union(string|int) <- 42: right={}", v.is_right()),
        Err(e) => println!("[ERR] union: {}", e),
    }
    match string_or_int.parse("true") {
        Ok(_) => println!("[OK] union accepted bool?"),
        Err(e) => println!("[ERR] union(string|int) <- true: {}", e),
    }

    let triple = vld::union3(vld::string(), vld::number().int(), vld::boolean());
    match triple.parse("true") {
        Ok(_) => println!("[OK] union3(string|int|bool) <- true: accepted"),
        Err(e) => println!("[ERR] union3: {}", e),
    }

    // Literal union — discriminated
    let status = vld::union(vld::literal("active"), vld::literal("inactive"));
    println!(
        "[OK] literal union \"active\": {:?}",
        status.parse(r#""active""#)
    );
    println!(
        "[ERR] literal union \"unknown\": {:?}",
        status.parse(r#""unknown""#)
    );

    // union! macro — variadic union of 2–6 schemas
    println!("\n--- union! macro ---");

    // 2 schemas
    let u2 = vld::union!(vld::string(), vld::number());
    println!(
        "[OK] union!(string, number) <- \"hi\":  {:?}",
        u2.parse(r#""hi""#).is_ok()
    );
    println!(
        "[OK] union!(string, number) <- 42:    {:?}",
        u2.parse("42").is_ok()
    );
    println!(
        "[ERR] union!(string, number) <- true: {:?}",
        u2.parse("true").is_err()
    );

    // 3 schemas
    let u3 = vld::union!(vld::string(), vld::number(), vld::boolean());
    println!("[OK] union!(s,n,b) <- true: {:?}", u3.parse("true").is_ok());

    // 4 schemas
    let u4 = vld::union!(
        vld::string(),
        vld::number(),
        vld::boolean(),
        vld::number().int(),
    );
    println!("[OK] union!(s,n,b,i) <- 7:     {:?}", u4.parse("7").is_ok());
    println!(
        "[ERR] union!(s,n,b,i) <- null: {:?}",
        u4.parse("null").is_err()
    );

    // 5 schemas
    let u5 = vld::union!(
        vld::string(),
        vld::number(),
        vld::boolean(),
        vld::number().int(),
        vld::literal("special"),
    );
    println!(
        "[OK] union!(5) <- \"special\": {:?}",
        u5.parse(r#""special""#).is_ok()
    );

    // ---------------------------------------------------------------------------
    // 10. Pipe and preprocess
    // ---------------------------------------------------------------------------
    println!("\n--- Pipe and preprocess ---");

    let pipe_schema = vld::string()
        .transform(|s| s.len())
        .pipe(vld::number().min(3.0));

    match pipe_schema.parse(r#""hello""#) {
        Ok(v) => println!("[OK] pipe string->len->min(3): {}", v),
        Err(e) => println!("[ERR] pipe: {}", e),
    }
    match pipe_schema.parse(r#""hi""#) {
        Ok(v) => println!("[OK] pipe \"hi\": {}", v),
        Err(e) => println!("[ERR] pipe \"hi\" (len=2 < 3): {}", e),
    }

    let preprocess_schema = vld::preprocess(
        |v| match v.as_str() {
            Some(s) => serde_json::json!(s.trim()),
            None => v.clone(),
        },
        vld::string().min(1),
    );
    match preprocess_schema.parse(r#""  hello  ""#) {
        Ok(v) => println!("[OK] preprocess trim: \"{}\"", v),
        Err(e) => println!("[ERR] preprocess: {}", e),
    }
    match preprocess_schema.parse(r#""   ""#) {
        Ok(v) => println!("[OK] preprocess trim: \"{}\"", v),
        Err(e) => println!("[ERR] preprocess trim(\"   \") -> empty: {}", e),
    }

    // ---------------------------------------------------------------------------
    // 11. Dynamic object
    // ---------------------------------------------------------------------------
    println!("\n--- Dynamic object ---");

    let obj = vld::object()
        .field("name", vld::string().min(1))
        .field("score", vld::number().min(0.0).max(100.0))
        .strict();

    match obj.parse(r#"{"name": "Alice", "score": 95.5}"#) {
        Ok(map) => println!("[OK] object: {:?}", map),
        Err(e) => println!("[ERR] object: {}", e),
    }

    match obj.parse(r#"{"name": "Bob", "score": 50, "extra": true}"#) {
        Ok(map) => println!("[OK] strict object: {:?}", map),
        Err(e) => println!("[ERR] strict object (extra field): {}", e),
    }

    // ---------------------------------------------------------------------------
    // 12. schema! macro — successful parse
    // ---------------------------------------------------------------------------
    println!("\n--- schema! macro (success) ---");

    let json = r#"{
        "name": "Alex",
        "email": "alex@example.com",
        "age": 30,
        "role": "admin",
        "tags": ["rust", "zod"],
        "address": {
            "city": "New York",
            "zip": "100001"
        }
    }"#;

    match User::parse(json) {
        Ok(user) => {
            println!("[OK] User: {:#?}", user);
        }
        Err(e) => {
            println!("[ERR] User: {}", e);
        }
    }

    // ---------------------------------------------------------------------------
    // 13. schema! macro — validation errors (all at once)
    // ---------------------------------------------------------------------------
    println!("\n--- schema! macro (errors) ---");

    let bad_json = r#"{
        "name": "A",
        "email": "not-email",
        "age": 10,
        "role": "hacker",
        "tags": ["", "ok"],
        "address": {
            "city": "",
            "zip": "short"
        }
    }"#;

    match User::parse(bad_json) {
        Ok(user) => println!("[OK] User: {:#?}", user),
        Err(e) => {
            println!("Found {} validation errors:", e.issues.len());
            for issue in &e.issues {
                let path: String = issue.path.iter().map(|p| p.to_string()).collect();
                println!("  {} -> {}", path, issue.message);
            }

            // Pretty output
            println!("\n--- prettify_error ---");
            println!("{}", prettify_error(&e));

            // Flat structure
            println!("\n--- flatten_error ---");
            let flat = flatten_error(&e);
            for (field, msgs) in &flat.field_errors {
                println!("  {}: {:?}", field, msgs);
            }

            // Error tree
            println!("\n--- treeify_error ---");
            let tree = treeify_error(&e);
            print_tree(&tree, 0);
        }
    }

    // ---------------------------------------------------------------------------
    // 14. Reading from file (VldInput for Path)
    // ---------------------------------------------------------------------------
    println!("\n--- Reading from file ---");

    // Paths relative to the working directory (cargo run runs from the project root)
    let valid_file = std::path::Path::new("examples/user.json");
    let invalid_file = std::path::Path::new("examples/user_invalid.json");

    // Valid file — parse directly from Path
    println!("File: {}", valid_file.display());
    match User::parse(valid_file) {
        Ok(user) => println!("[OK] User from file: {:#?}", user),
        Err(e) => println!("[ERR] User from file:\n{}", prettify_error(&e)),
    }

    // ---------------------------------------------------------------------------
    // 15. parse_lenient — get the struct even when there are errors
    // ---------------------------------------------------------------------------
    println!("\n--- parse_lenient (invalid file) ---");
    println!("File: {}", invalid_file.display());

    let result = User::parse_lenient(invalid_file).unwrap();

    // Inspect the ParseResult object
    println!("\nStruct (invalid fields -> Default):");
    println!("{:#?}", result.value);

    println!("\nPer-field results:");
    for field in result.fields() {
        println!("  {}", field);
    }

    println!(
        "\nTotal: {} valid, {} with errors",
        result.valid_count(),
        result.error_count()
    );

    println!("\nValid fields only:");
    for f in result.valid_fields() {
        println!("  {}", f);
    }
    println!("Error fields only:");
    for f in result.error_fields() {
        println!("  {}", f);
    }

    // Display trait on ParseResult
    println!("\nParseResult Display:");
    print!("{}", result);

    // ---------------------------------------------------------------------------
    // 16. prettify_error — detailed error output for strict parse
    // ---------------------------------------------------------------------------
    println!("--- Result (prettify_error) ---");
    match User::parse(invalid_file) {
        Ok(user) => println!("[OK] {:#?}", user),
        Err(e) => println!("{}", prettify_error(&e)),
    }

    // ---------------------------------------------------------------------------
    // 17. save_to_file — save the ParseResult whenever you want
    // ---------------------------------------------------------------------------
    println!("\n--- save_to_file (from ParseResult) ---");

    let save_path = std::path::Path::new("examples/user_saved.json");
    match result.save_to_file(save_path) {
        Ok(()) => println!("[OK] Saved to {}", save_path.display()),
        Err(e) => println!("[ERR] Write error: {}", e),
    }

    // You can also get the JSON string without writing to a file
    println!("\nJSON string:");
    println!("{}", result.to_json_string().unwrap());

    // Or consume the struct out of the result
    let user = result.into_value();
    println!("\nExtracted struct: {:?}", user);

    // Missing file — IO error
    let missing = std::path::Path::new("examples/no_such_file.json");
    match User::parse(missing) {
        Ok(_) => println!("[OK] missing file — unexpected success"),
        Err(e) => println!("\n[ERR] Missing file: {}", e),
    }

    // ---------------------------------------------------------------------------
    // 18. Single-field extraction — parse entire schema, then pick one field
    // ---------------------------------------------------------------------------
    println!("\n--- Single-field extraction ---");

    // A) Strict parse — get the struct, access any field directly
    let valid_json = r#"{
        "name": "Alex",
        "email": "alex@example.com",
        "age": 30,
        "role": "admin",
        "tags": ["rust"],
        "address": { "city": "New York", "zip": "100001" }
    }"#;

    let user = User::parse(valid_json).unwrap();
    println!("user.name  = {}", user.name);
    println!("user.email = {}", user.email);
    println!("user.age   = {:?}", user.age);
    println!("user.role  = {}", user.role);

    // B) Lenient parse — even if some fields are invalid, extract the ones that passed
    let partial_json = r#"{
        "name": "X",
        "email": "not-email",
        "age": 25,
        "role": "admin",
        "tags": [""],
        "address": { "city": "London", "zip": "1" }
    }"#;

    let result = User::parse_lenient(partial_json).unwrap();

    // Access the whole struct (invalid fields fall back to Default)
    println!(
        "\nLenient — user.name  = {:?} (defaulted, original was invalid)",
        result.value.name
    );
    println!(
        "Lenient — user.age   = {:?} (valid, kept as-is)",
        result.value.age
    );
    println!(
        "Lenient — user.role  = {:?} (valid, kept as-is)",
        result.value.role
    );

    // Use .field("name") to check a specific field's validation result
    if let Some(name_result) = result.field("name") {
        println!("\nField 'name': {}", name_result);
    }
    if let Some(age_result) = result.field("age") {
        println!("Field 'age':  {}", age_result);
    }
    if let Some(email_result) = result.field("email") {
        println!("Field 'email': {}", email_result);
    }

    // Check if a specific field is valid/invalid
    let name_ok = result.field("name").is_some_and(|f| f.is_ok());
    let age_ok = result.field("age").is_some_and(|f| f.is_ok());
    println!("\nname valid? {} | age valid? {}", name_ok, age_ok);

    // ---------------------------------------------------------------------------
    // 19. .or() / .and() chain syntax
    // ---------------------------------------------------------------------------
    println!("\n--- .or() / .and() ---");

    let string_or_int = vld::string().or(vld::number().int());
    println!(
        "[OK] or(\"hello\"): {:?}",
        string_or_int.parse(r#""hello""#).unwrap().is_left()
    );
    println!(
        "[OK] or(42): {:?}",
        string_or_int.parse("42").unwrap().is_right()
    );

    let bounded = vld::string().min(3).and(vld::string().max(10));
    println!(
        "[OK] and(\"hello\"): {:?}",
        bounded.parse(r#""hello""#).is_ok()
    );
    println!("[ERR] and(\"hi\"): {:?}", bounded.parse(r#""hi""#).is_err());

    // ---------------------------------------------------------------------------
    // 20. custom() schema
    // ---------------------------------------------------------------------------
    println!("\n--- custom() ---");

    let even = vld::custom(|v: &serde_json::Value| {
        let n = v.as_i64().ok_or_else(|| "Expected integer".to_string())?;
        if n % 2 == 0 {
            Ok(n)
        } else {
            Err("Must be even".to_string())
        }
    });
    println!("[OK] custom(4): {:?}", even.parse("4").unwrap());
    println!("[ERR] custom(5): {:?}", even.parse("5").is_err());

    // ---------------------------------------------------------------------------
    // 21. JSON Schema generation
    // ---------------------------------------------------------------------------
    println!("\n--- to_json_schema ---");

    let str_schema = vld::string().min(2).max(50).email();
    println!(
        "string: {}",
        serde_json::to_string_pretty(&str_schema.to_json_schema()).unwrap()
    );

    let num_schema = vld::number().int().min(0).max(100);
    println!(
        "int: {}",
        serde_json::to_string_pretty(&num_schema.to_json_schema()).unwrap()
    );

    let obj_schema = vld::object()
        .field("name", vld::string())
        .field("age", vld::number())
        .strict();
    println!(
        "object: {}",
        serde_json::to_string_pretty(&obj_schema.to_json_schema()).unwrap()
    );

    // ---------------------------------------------------------------------------
    // 22. JSON Schema / OpenAPI generation (trait-based)
    // ---------------------------------------------------------------------------
    println!("\n--- JSON Schema / OpenAPI ---");

    // Individual schema → JSON Schema
    let str_js = vld::string().min(2).max(50).email().json_schema();
    println!("[1] String JSON Schema:");
    println!("{}", serde_json::to_string_pretty(&str_js).unwrap());

    let arr_js = vld::array(vld::number().int().positive())
        .min_len(1)
        .json_schema();
    println!("[2] Array JSON Schema:");
    println!("{}", serde_json::to_string_pretty(&arr_js).unwrap());

    // Optional wraps with oneOf: [inner, {type: null}]
    let opt_js = vld::string().email().optional().json_schema();
    println!("[3] Optional String JSON Schema:");
    println!("{}", serde_json::to_string_pretty(&opt_js).unwrap());

    // Object with field_schema — includes nested field schemas
    let obj_js = vld::object()
        .field_schema("email", vld::string().email().min(5))
        .field_schema("score", vld::number().min(0.0).max(100.0))
        .strict()
        .json_schema();
    println!("[4] Object with field schemas:");
    println!("{}", serde_json::to_string_pretty(&obj_js).unwrap());

    // Union generates oneOf
    let union_js = vld::union(vld::string(), vld::number()).json_schema();
    println!("[5] Union JSON Schema:");
    println!("{}", serde_json::to_string_pretty(&union_js).unwrap());

    // Describe adds description
    let desc_js = vld::string()
        .min(1)
        .describe("User display name")
        .json_schema();
    println!("[6] Described Schema:");
    println!("{}", serde_json::to_string_pretty(&desc_js).unwrap());

    // schema! macro → full struct JSON Schema
    let user_js = User::json_schema();
    println!("[7] User struct JSON Schema:");
    println!("{}", serde_json::to_string_pretty(&user_js).unwrap());

    // schema! macro → OpenAPI 3.1 document
    let openapi_doc = User::to_openapi_document();
    println!("[8] User OpenAPI 3.1 document:");
    println!("{}", serde_json::to_string_pretty(&openapi_doc).unwrap());

    // Multi-schema OpenAPI document
    let multi_doc = vld::json_schema::to_openapi_document_multi(&[
        ("User", User::json_schema()),
        ("Address", Address::json_schema()),
    ]);
    println!("[9] Multi-schema OpenAPI document:");
    println!("{}", serde_json::to_string_pretty(&multi_doc).unwrap());

    // ---------------------------------------------------------------------------
    // 23. Custom error messages
    // ---------------------------------------------------------------------------
    println!("\n--- Custom error messages ---");

    // 1) _msg variants: set message per check at definition time
    let schema = vld::string()
        .min_msg(3, "Name must be at least 3 characters")
        .email_msg("Please enter a valid email address");
    let err = schema.parse(r#""ab""#).unwrap_err();
    println!("[1] _msg variants:");
    for issue in &err.issues {
        println!("     {}", issue.message);
    }

    // 2) type_error: custom message for wrong JSON type
    let schema = vld::string().type_error("This field requires text, not a number");
    let err = schema.parse("42").unwrap_err();
    println!("[2] type_error:  {}", err.issues[0].message);

    let schema = vld::number().type_error("Age must be a number");
    let err = schema.parse(r#""hello""#).unwrap_err();
    println!("     number:     {}", err.issues[0].message);

    let schema = vld::number().int().int_error("Please enter a whole number");
    let err = schema.parse("3.5").unwrap_err();
    println!("     int_error:  {}", err.issues[0].message);

    // 3) with_messages: bulk override by check key
    let schema = vld::string()
        .min(5)
        .max(100)
        .email()
        .with_messages(|key| match key {
            "too_small" => Some("Too short!".into()),
            "too_big" => Some("Too long!".into()),
            "invalid_email" => Some("Bad email format!".into()),
            _ => None,
        });
    let err = schema.parse(r#""ab""#).unwrap_err();
    println!("[3] with_messages (string):");
    for issue in &err.issues {
        println!("     {}", issue.message);
    }

    // 4) with_messages on numbers — e.g., for translation
    let schema = vld::number()
        .min(1.0)
        .max(100.0)
        .with_messages(|key| match key {
            "too_small" => Some("Значение должно быть не менее 1".into()),
            "too_big" => Some("Значение не должно превышать 100".into()),
            _ => None,
        });
    let err = schema.parse("200").unwrap_err();
    println!("[4] with_messages (number, RU): {}", err.issues[0].message);

    let err = schema.parse("-5").unwrap_err();
    println!("     negative: {}", err.issues[0].message);

    // 5) with_messages on int — including not_int
    let schema = vld::number()
        .int()
        .min(1)
        .max(10)
        .with_messages(|key| match key {
            "too_small" => Some("Minimum is 1".into()),
            "too_big" => Some("Maximum is 10".into()),
            "not_int" => Some("Decimals not allowed".into()),
            _ => None,
        });
    let err = schema.parse("3.5").unwrap_err();
    println!("[5] int with_messages: {}", err.issues[0].message);

    // 6) Combining type_error + with_messages + in an object
    let obj = vld::object()
        .field(
            "name",
            vld::string()
                .min(2)
                .type_error("Name must be a string")
                .with_messages(|key| match key {
                    "too_small" => Some("Name is too short".into()),
                    _ => None,
                }),
        )
        .field(
            "age",
            vld::number()
                .int()
                .min(18)
                .type_error("Age must be a number")
                .with_messages(|key| match key {
                    "too_small" => Some("Must be 18 or older".into()),
                    _ => None,
                }),
        );
    let err = obj.parse(r#"{"name": "A", "age": 5}"#).unwrap_err();
    println!("[6] Object with custom messages:");
    for issue in &err.issues {
        let path: String = issue.path.iter().map(|p| p.to_string()).collect();
        println!("     {}: {}", path, issue.message);
    }

    // ---------------------------------------------------------------------------
    // 23. Validate existing Rust values (schema-level)
    // ---------------------------------------------------------------------------
    println!("\n--- Validate existing Rust values ---");

    // Validate a Vec against an array schema
    let arr_schema = vld::array(vld::number().int().positive())
        .min_len(1)
        .max_len(5);
    let good_vec = vec![1, 2, 3];
    let bad_vec = vec![-1, 0, 5];
    println!(
        "[1] vec![1,2,3] valid:   {}",
        arr_schema.is_valid(&good_vec)
    );
    println!("    vec![-1,0,5] valid:  {}", arr_schema.is_valid(&bad_vec));
    if let Err(e) = arr_schema.validate(&bad_vec) {
        println!("    errors: {}", e);
    }

    // Validate a String against a string schema
    let email_schema = vld::string().email();
    let good_email = "user@example.com".to_string();
    let bad_email = "not-an-email".to_string();
    println!("[2] email valid:   {}", email_schema.is_valid(&good_email));
    println!("    bad email:     {}", email_schema.is_valid(&bad_email));

    // Validate a number
    let age_schema = vld::number().int().min(18).max(120);
    println!("[3] age 25 valid:  {}", age_schema.is_valid(&25));
    println!("    age 10 valid:  {}", age_schema.is_valid(&10));

    // Validate a HashMap as an object
    let mut map = std::collections::HashMap::new();
    map.insert("a", 10);
    map.insert("b", 20);
    let record_schema = vld::record(vld::number().int().positive());
    println!("[4] record valid:  {}", record_schema.is_valid(&map));

    // ---------------------------------------------------------------------------
    // 24. Validate struct instances (schema! macro)
    // ---------------------------------------------------------------------------
    println!("\n--- Validate struct instances ---");

    // Parse a valid user first
    let user = User::parse(
        r#"{
        "name": "Alice",
        "email": "alice@example.com",
        "age": 30,
        "address": {"city": "NYC", "zip": "123456"}
    }"#,
    )
    .unwrap();
    println!("[1] Valid user: User::is_valid = {}", User::is_valid(&user));

    // Construct a struct manually with invalid data
    let bad_user = User {
        name: "A".to_string(),          // too short (min 2)
        email: "not-email".to_string(), // invalid email
        age: Some(10),                  // under 18
        role: "hacker".to_string(),     // not in enum
        nickname: "ok".to_string(),     // too short (min 3)
        tags: vec![],
        address: Address {
            city: "X".to_string(),  // ok (min 1)
            zip: "123".to_string(), // too short (len 6)
        },
    };
    println!(
        "[2] Bad user: User::is_valid = {}",
        User::is_valid(&bad_user)
    );
    if let Err(e) = User::validate(&bad_user) {
        println!("    errors:");
        for issue in &e.issues {
            let path: String = issue.path.iter().map(|p| p.to_string()).collect();
            println!("      {}: {}", path, issue.message);
        }
    }

    // You can also validate any Serialize value against the schema
    let raw_json = serde_json::json!({
        "name": "Bob",
        "email": "bob@test.com",
        "address": {"city": "LA", "zip": "654321"}
    });
    println!("[3] JSON value valid: {}", User::is_valid(&raw_json));

    // ---------------------------------------------------------------------------
    // 25. impl_rules! — attach validation to a plain struct
    // ---------------------------------------------------------------------------
    println!("\n--- impl_rules! (plain struct with validation) ---");

    // A regular Rust struct — no Serialize or Debug required.
    struct Product {
        name: String,
        price: f64,
        quantity: i64,
        sku: String,
        tags: Vec<String>,
    }

    // Attach validation rules via macro — generates .validate() and .is_valid()
    vld::impl_rules!(Product {
        name     => vld::string().min(2).max(100)
                        .with_messages(|k| match k {
                            "too_small" => Some("Product name is too short".into()),
                            _ => None,
                        }),
        price    => vld::number().positive().max(1_000_000.0),
        quantity => vld::number().int().non_negative().max(99999),
        sku      => vld::string().starts_with("SKU-").min(7).max(20),
        tags     => vld::array(vld::string().min(1).max(30)).max_len(10),
    });

    // Valid product
    let good = Product {
        name: "Widget Pro".into(),
        price: 29.99,
        quantity: 100,
        sku: "SKU-12345".into(),
        tags: vec!["electronics".into(), "sale".into()],
    };
    println!("[1] Good product valid: {}", good.is_valid());

    // Invalid product
    let bad = Product {
        name: "X".into(),
        price: -5.0,
        quantity: -1,
        sku: "NOPE".into(),
        tags: vec!["".into()],
    };
    println!("[2] Bad product valid:  {}", bad.is_valid());
    if let Err(e) = bad.validate() {
        for issue in &e.issues {
            let path: String = issue.path.iter().map(|p| p.to_string()).collect();
            println!("     {}: {}", path, issue.message);
        }
    }

    // Fix and re-validate
    let mut fixed = Product {
        name: "Widget".into(),
        price: 9.99,
        quantity: 5,
        sku: "NOPE".into(),
        tags: vec![],
    };
    println!("[3] Partially fixed: {}", fixed.is_valid());
    fixed.sku = "SKU-99887".into();
    println!("    After SKU fix:   {}", fixed.is_valid());

    println!("\n=== Done ===");
}

fn print_tree(tree: &vld::format::ErrorTree, indent: usize) {
    let pad = "  ".repeat(indent);
    for err in &tree.errors {
        println!("{}* {}", pad, err);
    }
    for (key, sub) in &tree.properties {
        println!("{}{}: ", pad, key);
        print_tree(sub, indent + 1);
    }
    for (i, item) in tree.items.iter().enumerate() {
        if let Some(sub) = item {
            println!("{}[{}]:", pad, i);
            print_tree(sub, indent + 1);
        }
    }
}
