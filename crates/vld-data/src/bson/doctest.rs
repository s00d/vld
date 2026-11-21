use bson::{doc, oid::ObjectId};

// An ObjectId is `{"$oid": "<24 hex>"}` in relaxed Extended JSON.
vld::schema! {
    #[derive(Debug)]
    pub struct ObjectIdSchema {
        pub oid: String as "$oid" => vld::string().len(24),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct UserSchema {
        pub id: ObjectIdSchema as "_id" => vld::nested!(ObjectIdSchema),
        pub name: String => vld::string().min(1),
        pub age: i64 => vld::number().int().min(0),
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct User {
    #[serde(rename = "_id")]
    id: ObjectId,
    name: String,
    age: i64,
}

let user = User { id: ObjectId::new(), name: "Ada".into(), age: 36 };
// Validate, then hand the document to `collection.insert_one(doc)`.
let doc = to_document::<UserSchema, _>(&user).unwrap();

// After `find_one`: validate, then deserialize.
let back: User = from_document::<UserSchema, _>(doc).unwrap();
assert_eq!(back.name, "Ada");

let bad = doc! { "_id": ObjectId::new(), "name": "", "age": -1 };
assert!(validate_document::<UserSchema>(&bad).is_err());
