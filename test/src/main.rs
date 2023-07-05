use reqwest::{header, Response, StatusCode, Url};
use serde_json::json;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let base_url = Url::parse("https://localhost:8080/api/v1/")?;

    // Initialize HTTP client with disabled SSL verification
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();

    // Perform setup
    let setup_url = base_url.join("setup")?;
    let mut result = client
        .post(setup_url)
        .header("is_test", "true")
        .send()
        .await?;
    let mut pass = 0;
    let mut fail = 0;

    match check_response(
        result,
        StatusCode::OK,
        "{\"message\":\"database: user-test-db collection: user-test-collection created\",\"status\":200,\"body\":\"\"}".to_string()
    ).await {
        Ok(true) => pass += 1,
        Ok(false) | Err(_) => {
            fail += 1;
            println!("unexpected error setting up the database");
        }
    }

    // Perform user registration
    let register_url = base_url.join("users")?;
    let mut user_data = json!({
        "password": "1223very long password!",
        "email": "testi@example.com",
        "first_name": "Doug",
        "last_name": "Smith",
        "display_name": "Dougy",
        "picture_url": "https://www.facebook.com/photo/?fbid=10152713241860783&set=a.485603425782",
        "foreground_color": "#000000",
        "background_color": "#FFFFFFF",
        "games_played": 10,
        "games_won": 1
    });
    result = client
        .post(register_url)
        .header("is_test", "true")
        .header(header::CONTENT_TYPE, "application/json")
        .json(&user_data)
        .send()
        .await?;

    if result.status() != StatusCode::OK {
        panic!("Registration failed with status: {:?}", result.status());
    }

    let response_body = result.text().await?;

    let response_json: serde_json::Value = serde_json::from_str(&response_body)?;

    // Extract and store the id field
    let user_id = response_json["id"].as_str().unwrap().to_owned();
    user_data["id"] = json!(user_id);
    user_data["partition_key"] = json!(1);
    user_data["password_hash"] = json!(null);
    user_data["password"] = json!(null);

    if user_data != response_json {
        println!("unexpected failure in registering user");
        fail += 1;
    } else {
        pass += 1;
    }

    println!("pass: {} fail: {}", pass, fail);

    Ok(())
}

async fn check_response(
    response: Response,
    expected_code: StatusCode,
    expected_value: String,
) -> Result<bool, reqwest::Error> {
    if response.status() != expected_code {
        return Ok(false);
    }

    let body = response.text().await?;

    if body != expected_value {
        return Ok(false);
    }

    Ok(true)
}
