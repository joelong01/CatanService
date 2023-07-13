
#[macro_export]
macro_rules! serialize_as_array2 {
    ($key:ty, $value:ty) => {
        fn serialize_as_array<S>(data: &HashMap<$key, $value>, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let values: Vec<$value> = data.values().cloned().collect();
            values.serialize(serializer)
        }
    };
}
#[macro_export]
macro_rules! deserialize_from_array {
    ($key:ty, $value:ty) => {
        fn deserialize_from_array<'de, D>(deserializer: D) -> Result<HashMap<$key, $value>, D::Error>
        where
            D: Deserializer<'de>,
        {
            let values: Vec<$value> = Vec::deserialize(deserializer)?;
            let mut map = HashMap::new();

            for value in values {
                map.insert(value.key.clone(), value);
            }

            Ok(map)
        }
    };
}

#[macro_export]
macro_rules! log_return_err {
    ( $e:expr ) => {{
        log::error!("\t{}\n {:#?}", $e, $e);
        return Err($e);
    }};
}

#[macro_export]
macro_rules! serialize_as_array {
    ($key:ty, $value:ty) => {
        fn serialize_as_array_impl<S>(data: &HashMap<$key, $value>, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let values: Vec<$value> = data.values().cloned().collect();
            values.serialize(serializer)
        }

        serialize_as_array_impl
    };
}

