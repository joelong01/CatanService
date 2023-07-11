use rand::{rngs::StdRng, Rng, SeedableRng};
use std::cell::RefCell;

/*
 *  I didn't want to use GUIDs for the unique ID.  We need an ID that can be quickly generated, is unique,
 *  and works if multiple threads are creating documents.  I'm using Rand() seeded per thread.
 */

//macro for get_id
thread_local! {
    static RNG: RefCell<StdRng> = RefCell::new(StdRng::from_entropy());
}
pub fn get_id() -> String {
    format!("unique_id{}", RNG.with(|rng| rng.borrow_mut().gen::<u64>()))
}

#[macro_export]
macro_rules! log_return_err {
    ( $e:expr ) => {{
        log::error!("\t{}\n {:#?}", $e, $e);
        return Err($e);
    }};
}

pub trait SerializerKeyTrait {
    fn serialize_key(&self) -> Result<String, serde_json::error::Error>;
}

#[macro_export]
macro_rules! KeySerializer {
    ($struct_name:ident { $($field:ident),* $(,)? }) => {
        impl SerializerKeyTrait for $struct_name {
            fn serialize_key(&self) -> Result<String, serde_json::error::Error> {
                let mut map = serde_json::Map::new();
                $(
                    map.insert(stringify!($field).to_string(), serde_json::to_value(&self.$field)?);
                )*
                let output = serde_json::to_string(&map)?;
                Ok(output.replace("{", "[").replace("}", "]"))
            }
        }

        impl Serialize for $struct_name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                match self.serialize_key() {
                    Ok(serialized) => serializer.serialize_str(&serialized),
                    Err(_) => Err(ser::Error::custom(concat!("Failed to serialize ", stringify!($struct_name)))),
                }
            }
        }
    };
}

pub trait DeserializeKeyTrait: Sized {
    fn deserialize_key(input: &str) -> Result<Self, serde_json::Error>;
}

#[macro_export]
macro_rules! DeserializeKey {
    ($struct_name:ident { $($field:ident),* $(,)? }) => {
        impl DeserializeKeyTrait for $struct_name {
            fn deserialize_key(input: &str) -> Result<Self, serde_json::Error> {
                let value: Value = serde_json::from_str(input)?;
                let map = value.as_object().ok_or_else(|| {
                    serde_json::Error::custom("Failed to deserialize key: invalid format")
                })?;

                $(
                    let $field = serde_json::from_value(map.get(stringify!($field))
                        .ok_or_else(|| serde_json::Error::custom(format!("Missing field: {}", stringify!($field))))?
                        .clone())?;
                )*

                Ok(Self { $($field),* })
            }
        }

        impl<'de> Deserialize<'de> for $struct_name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                // put curly braces back
                let input = String::deserialize(deserializer)?.replace("[", "{").replace("]", "}");
                Self::deserialize_key(&input).map_err(SerdeError::custom)
            }
        }
    };
}
