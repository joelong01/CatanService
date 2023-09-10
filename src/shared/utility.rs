use uuid::Uuid;


/// Generates a unique user ID.
///
/// This function creates random user IDs by creating a guid
///
/// # Returns
///
/// * A unique `String` ID.
pub fn get_id() -> String {
    Uuid::new_v4().to_string()
}

