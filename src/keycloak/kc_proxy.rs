#![allow(dead_code)]
use jsonwebtoken::{decode, Algorithm, DecodingKey, TokenData, Validation};
use reqwest::{Client, Method, RequestBuilder};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

use std::collections::HashMap;
use std::{fmt, fmt::Debug, fmt::Display, fmt::Formatter};
///
/// this macro is used in the tests to generate an info log message that includes file an line number
macro_rules! full_info {
    ($($arg:tt)*) => {
        {
            let formatted_msg = format!($($arg)*);
            let cleaned_msg = crate::macros::format_log_message(&formatted_msg);
            log::info!(target: &format!("{}:{}:", file!(), line!()), "{}", cleaned_msg);
        }
    };
}
///
/// These are the kinds of errors that the Proxy returns
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone, strum_macros::Display)]
pub enum KeyCloakErrorType {
    ReqwestError,
    KeyCloakServiceError,
    KeyCloakProxyError,
    JsonError,
    WebTokenError,
}
///
/// the error struct returned by the Proxy
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct KeyCloakError {
    pub error_type: KeyCloakErrorType,
    pub message: String,
}
impl Display for KeyCloakError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // Convert the struct to a pretty-printed JSON string.
        let json = serde_json::to_string_pretty(self).map_err(|_| fmt::Error)?;
        write!(f, "{}", json)
    }
}
impl KeyCloakError {
    pub fn new(error_type: KeyCloakErrorType, msg: &str) -> KeyCloakError {
        Self {
            error_type,
            message: msg.to_owned(),
        }
    }
}
// reqwest error to KeyCloakError
impl From<reqwest::Error> for KeyCloakError {
    fn from(err: reqwest::Error) -> Self {
        Self {
            error_type: KeyCloakErrorType::ReqwestError,
            message: format!("{:#?}", err),
        }
    }
}
// json error (usually parsing) to KeyCloakError
impl From<serde_json::Error> for KeyCloakError {
    fn from(err: serde_json::Error) -> Self {
        Self {
            error_type: KeyCloakErrorType::JsonError,
            message: format!("{:#?}", err),
        }
    }
}
// jwt error (usually decode) to KeyCloakError
impl From<jsonwebtoken::errors::Error> for KeyCloakError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        Self {
            error_type: KeyCloakErrorType::WebTokenError,
            message: format!("{:#?}", err),
        }
    }
}

///
/// the Proxy for calling KeyCloak - the best samples are the tests below
pub struct KeyCloakProxy {
    pub host_name: String,
    pub auth_token: Option<String>,
    pub realm: String,
}

impl KeyCloakProxy {
    pub fn new(host_name: &str, realm: &str) -> KeyCloakProxy {
        Self {
            host_name: host_name.to_owned(),
            auth_token: None,
            realm: realm.to_owned(),
        }
    }

    ///  the proxy calls are essentially stateless, which means you can create a proxy, set the auth_token, make a call
    ///  and then set a different auth_token for the next call.  doing it this way avoids having to pass it in to all
    ///  authenticated calls.
    pub fn set_auth_token(&mut self, auth_token: Option<String>) {
        self.auth_token = auth_token.clone();
    }

    ///
    /// allow realms to be changed dynamically
    pub fn set_realm(&mut self, realm: &str) {
        self.realm = realm.to_string();
    }

    /// Sends an HTTP request to the KeyCloak service.
    ///
    /// This function is a utility for making authenticated requests to the KeyCloak service. It supports various HTTP
    /// methods, handles serialization and deserialization of request and response bodies, and manages optional headers,
    /// URL parameters, and request bodies.
    ///
    /// The authentication token, if available in `self.auth_token`, is automatically attached as an Authorization header.
    ///
    /// # Arguments
    ///
    /// * `method` - The HTTP method (e.g., GET, POST).
    /// * `url` - The target URL.
    /// * `headers` - Optional headers for the request.
    /// * `params` - Optional URL parameters.
    /// * `body` - Optional request body, serialized to JSON for PUT and POST.
    ///
    /// # Returns
    ///
    /// * `Ok(T)` - Successful response with parsed body.
    /// * `Err(KeyCloakError)` - Error during request or parsing response.
    ///
    /// # Todo
    ///
    /// * Implement token refresh mechanism.
    ///
    /// # Panics
    ///
    /// * May panic if response body is expected but not found.
    async fn send_request<B, T>(
        &self,
        method: Method,
        url: &str,
        headers: Option<&HashMap<String, String>>,
        params: Option<&[(&str, &str)]>,
        body: Option<B>,
    ) -> Result<T, KeyCloakError>
    where
        B: Serialize + Debug,
        // Default needs to be here so that we can return Ok(()) when the response body is empty by design
        // this means that all struct that are returned in the body need to implement Default
        T: DeserializeOwned + 'static + Default,
    {
        full_info!("calling url: {}:{}", method, url);

        let client = Client::new();
        let mut request: RequestBuilder;

        match method {
            Method::GET => request = client.get(url),
            Method::POST => request = client.post(url),
            Method::PUT => request = client.put(url),
            Method::DELETE => request = client.delete(url),
            _ => {
                return Err(KeyCloakError::new(
                    KeyCloakErrorType::KeyCloakProxyError,
                    &format!("Unsupported HTTP method: {}", method),
                ))
            }
        };

        // Process bodies and headers for PUT and POST methods
        if method == Method::PUT || method == Method::POST {
            if let Some(form_params) = params {
                request = request.form(form_params);
                request = request.header("Content-Type", "application/x-www-form-urlencoded");
            } else if let Some(body_content) = body {
                let serialized_body = serde_json::to_string(&body_content)?;
                let content_length = serialized_body.as_bytes().len();
                request = request.body(serialized_body);
                request = request.header("Content-Type", "application/json");
                request = request.header("Content-Length", content_length.to_string());
            }
        }

        // Always process headers if they're present
        if let Some(header_map) = headers {
            for (key, value) in header_map {
                request = request.header(key.as_str(), value.as_str());
            }
        }

        // Auth header
        if let Some(auth_token) = &self.auth_token {
            let header_value = format!("Bearer {}", auth_token);
            request = request.header("Authorization", header_value);
        }

        let response = request.send().await?;

        full_info!("api returned status: {}", response.status());

        let status = response.status(); // Store the status code here
        let body = response
            .bytes()
            .await
            .expect("getting bytes should not fail");

        if body.is_empty() {
            if std::any::type_name::<T>() == std::any::type_name::<()>() {
                if status.is_success() {
                    return Ok(Default::default());
                } else {
                    return Err(KeyCloakError::new(
                        KeyCloakErrorType::KeyCloakServiceError,
                        &format!("Empty body. [status: {:#?}]", &status),
                    ));
                }
            }
            return Err(KeyCloakError::new(
                KeyCloakErrorType::KeyCloakServiceError,
                "Expected body, but found none",
            ));
        }

        if status.is_success() {
            let parsed_body = serde_json::from_slice::<T>(&body)?;

            Ok(parsed_body)
        } else {
            Err(KeyCloakError::new(
                KeyCloakErrorType::KeyCloakServiceError,
                &format!("[status: {:#?} body: {:#?}", status, body),
            ))
        }
    }

    /// simple wrapper of the send_request method that only adds Method::POST
    async fn post<B, T>(
        &self,
        url: &str,
        headers: Option<&HashMap<String, String>>,
        params: Option<&[(&str, &str)]>,
        body: Option<B>,
    ) -> Result<T, KeyCloakError>
    where
        B: Serialize + Debug,
        T: DeserializeOwned + 'static + Default,
    {
        self.send_request(Method::POST, url, headers, params, body)
            .await
    }

    /// simple wrapper of the send_request method that only adds Method::PUT
    async fn put<B, T>(
        &self,
        url: &str,
        headers: Option<&HashMap<String, String>>,
        params: Option<&[(&str, &str)]>,
        body: Option<B>,
    ) -> Result<T, KeyCloakError>
    where
        B: Serialize + Debug,
        T: DeserializeOwned + 'static + Default,
    {
        self.send_request(Method::PUT, url, headers, params, body)
            .await
    }
    /// simple wrapper of the send_request method that only adds Method::GET
    ///
    async fn get<T>(
        &self,
        url: &str,
        headers: Option<&HashMap<String, String>>,
        params: Option<&[(&str, &str)]>,
    ) -> Result<T, KeyCloakError>
    where
        T: DeserializeOwned + 'static + Default,
    {
        self.send_request::<(), T>(Method::GET, url, headers, params, None)
            .await
    }

    /// Log in to the KeyCloak server using the provided user name and password.
    ///
    /// This function uses the `password` grant type for OpenID Connect authentication.
    /// The function communicates with the KeyCloak server, attempting to authenticate the user
    /// and retrieve the relevant login data.
    ///
    /// # Arguments
    ///
    /// * `user_name` - The user name for the KeyCloak login.
    /// * `password` - The password corresponding to the provided user name.
    ///
    /// # Returns
    ///
    /// * `Ok(KeyCloakLoginData)` - Successful login with associated login data.
    /// * `Err(KeyCloakError)` - An error occurred during the login process.
    ///
    /// # Example
    ///
    /// ```
    /// let service = KeyCloakService::new(...);
    /// let result = service.login("admin", "password123").await;
    /// match result {
    ///     Ok(login_data) => println!("Successfully logged in with token: {}", login_data.token),
    ///     Err(e) => eprintln!("Login failed: {:?}", e),
    /// }
    /// ```
    pub async fn login(
        &self,
        user_name: &str,
        password: &str,
        client_id: Option<&str>,
        scopes: Option<&str>,
    ) -> Result<KeyCloakLoginData, KeyCloakError> {
        let url = format!(
            "{}/auth/realms/{}/protocol/openid-connect/token",
            &self.host_name, &self.realm,
        );

        // Use the provided client_id or default to "admin-cli"
        let client_id = client_id.unwrap_or("admin-cli");

        // Create base params
        let mut params: Vec<(&str, &str)> = vec![
            ("grant_type", "password"),
            ("client_id", client_id),
            ("username", user_name),
            ("password", password),
        ];

        // If scopes are provided, append to params
        if let Some(sc) = scopes {
            params.push(("scope", sc));
        }

        let login_data = self
            .post::<(), KeyCloakLoginData>(&url, None, Some(&params), None)
            .await?;

        Ok(login_data)
    }

    /// Creates a new user in KeyCloak.
    ///
    /// This function sends a POST request to the KeyCloak service to create a new user based on the provided data.
    ///
    /// # Arguments
    ///
    /// * `user_data` - A reference to the data for the user to be created.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Successful creation of the user.
    /// * `Err(KeyCloakError)` - Error during user creation or other related errors.
    pub async fn create_user(&self, user_data: &UserCreateRequest) -> Result<(), KeyCloakError> {
        let url = format!(
            "{}/auth/admin/realms/{}/users",
            &self.host_name, &self.realm
        );
        let _ = self
            .post::<&UserCreateRequest, ()>(&url, None, None, Some(user_data))
            .await?;
        Ok(())
    }

    /// Retrieves the profile of a user from KeyCloak based on their username.
    ///
    /// This function sends a GET request to the KeyCloak service to fetch the profile details
    /// of the specified user.
    ///
    /// # Arguments
    ///
    /// * `user_name` - A reference to the username of the user whose profile is to be fetched.
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<UserResponse>)` - A vector containing the profile details of the user. Multiple entries
    ///                             might be returned if there are multiple users with the same username.
    /// * `Err(KeyCloakError)` - Error during profile retrieval or other related errors.
    pub async fn get_user_profile(
        &self,
        user_name: &str,
    ) -> Result<Vec<UserResponse>, KeyCloakError> {
        let url = format!(
            "{}/auth/admin/realms/{}/users?username={}",
            &self.host_name, &self.realm, &user_name
        );
        let response = self.get::<Vec<UserResponse>>(&url, None, None).await?;
        Ok(response)
    }
    /// Sets or updates the password for a user in KeyCloak based on their user ID.
    ///
    /// This function sends a PUT request to the KeyCloak service to reset and set the password
    /// for the user specified by the user ID.
    ///
    /// # Arguments
    ///
    /// * `user_id` - A reference to the user ID of the user whose password is to be updated.
    /// * `password` - A reference to the new password for the user.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Password was successfully updated for the user.
    /// * `Err(KeyCloakError)` - Error during password update or other related errors.
    pub async fn set_password(&self, user_id: &str, password: &str) -> Result<(), KeyCloakError> {
        let url = format!(
            "{}/auth/admin/realms/{}/users/{}/reset-password",
            &self.host_name, &self.realm, &user_id
        );
        let creds = CredentialRepresentation {
            field_name: "password".to_owned(),
            temporary: false,
            value: password.to_string(),
        };
        let _ = self
            .put::<&CredentialRepresentation, ()>(&url, None, None, Some(&creds))
            .await?;
        Ok(())
    }
    /// Retrieves the public keys for the realm from KeyCloak.
    ///
    /// This function sends a GET request to the KeyCloak service to fetch the public keys
    /// associated with the specified realm. No authentication token is required for this request;
    /// however, the request will still work even if an authentication token is set.
    ///
    /// # Returns
    ///
    /// * `Ok(KeyCloakKeyResponse)` - The public keys for the realm.
    /// * `Err(KeyCloakError)` - Error during the retrieval of the public keys or other related errors.
    pub async fn get_keys(&self) -> Result<KeyCloakKeyResponse, KeyCloakError> {
        let url = format!(
            "{}/auth/realms/{}/protocol/openid-connect/certs",
            &self.host_name, &self.realm
        );
        let response = self.get::<KeyCloakKeyResponse>(&url, None, None).await?;
        Ok(response)
    }

    /// Retrieves the public key for the realm from KeyCloak.
    ///
    /// This function sends a GET request to the KeyCloak service to fetch the public key
    /// associated with the specified realm.
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The public key for the realm in string format.
    /// * `Err(KeyCloakError)` - Error during the retrieval of the public key or other related errors.
    pub async fn get_keycloak_public_key(&self) -> Result<String, KeyCloakError> {
        let url = format!("{}/auth/realms/{}", &self.host_name, &self.realm);

        let value = self.get::<Value>(&url, None, None).await?;
        let pk = value["public_key"].as_str().unwrap();
        Ok(pk.to_string())
    }

    /// Retrieves the group information for the realm from KeyCloak.
    ///
    /// This function sends a GET request to the KeyCloak service to fetch the group details
    /// associated with the specified realm.
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The group details for the realm as a string representation.
    /// * `Err(KeyCloakError)` - Error during the retrieval of the group details or other related errors.
    pub async fn get_groups_for_realm(&self) -> Result<Vec<GroupResponse>, KeyCloakError> {
        let url = format!(
            "{}/auth/admin/realms/{}/groups",
            &self.host_name, &self.realm
        );

        let response = self.get::<Vec<GroupResponse>>(&url, None, None).await?;
        Ok(response)
    }
    /// Retrieves the roles associated with a specified client or the realm if no client is provided.
    ///
    /// When provided with a `client_uuid`, this function fetches the roles specific to that client.
    /// If `client_uuid` is `None`, it fetches the roles for the entire realm.
    ///
    /// # Parameters
    /// - `client_uuid`: An optional UUID string representing the client. If `None`, roles for the realm are retrieved.
    ///
    /// # Returns
    /// - A `Result` containing a vector of `KeyCloakRole` if successful, or a `ServiceError` if there's an error.
    ///
    /// # Example Usage:
    /// ```
    /// let roles_for_client = keycloak_client.get_roles(Some("client-uuid")).await?;
    /// let roles_for_realm = keycloak_client.get_roles(None).await?;
    /// ```
    pub async fn get_roles(
        &self,
        client_uuid: Option<&str>,
    ) -> Result<Vec<KeyCloakRole>, KeyCloakError> {
        let url = match client_uuid {
            Some(uuid) => format!(
                "{}/auth/admin/realms/{}/clients/{}/roles",
                &self.host_name, &self.realm, uuid
            ),
            None => format!(
                "{}/auth/admin/realms/{}/roles",
                &self.host_name, &self.realm
            ),
        };
        let roles: Vec<KeyCloakRole> = self.get(&url, None, None).await?;
        Ok(roles)
    }

    /// Retrieves the client ID associated with a given client name from the KeyCloak server.
    ///
    /// This function constructs the URL using the `self.host_name` and `self.realm`, and makes a request to fetch the client ID.
    /// It expects a single-element array as the response from the server. The function will return an error if the structure
    /// of the returned data does not match the expected format.
    ///
    /// # Parameters
    /// - `name`: The name of the client for which the ID is to be retrieved.
    ///
    /// # Returns
    /// - A `Result` containing a string representation of the client ID if successful, or a `ServiceError` if there's an error.
    pub async fn get_client_id(&self, name: &str) -> Result<String, KeyCloakError> {
        let url = format!(
            "{}/auth/admin/realms/{}/clients?clientId={}",
            &self.host_name, &self.realm, name
        );
        let value = self.get::<Value>(&url, None, None).await?;
        if let Value::Array(arr) = &value {
            if arr.len() != 1 {
                return Err(KeyCloakError::new(
                    KeyCloakErrorType::KeyCloakProxyError,
                    "Array length is not 1.",
                ));
            }
            let first_element = &arr[0];
            if let Some(id_value) = first_element.get("id") {
                if let Value::String(id_str) = id_value {
                    return Ok(id_str.to_string());
                } else {
                    return Err(KeyCloakError::new(
                        KeyCloakErrorType::KeyCloakProxyError,
                        "'id' value is not a string.",
                    ));
                }
            } else {
                return Err(KeyCloakError::new(
                    KeyCloakErrorType::KeyCloakProxyError,
                    "'id' not found in first element.",
                ));
            }
        } else {
            return Err(KeyCloakError::new(
                KeyCloakErrorType::KeyCloakProxyError,
                "Value is not an array.",
            ));
        }
    }

    ///
    /// Assigns the specified client roles to a user within the Keycloak system.
    ///
    /// - user_id: The unique identifier of the user to whom the roles should be assigned.
    /// - client_id: The unique identifier of the client under which the roles exist.
    /// - roles: A vector of roles (KeyCloakRole) to be assigned to the user.
    ///
    /// It's important to note that this function specifically deals with client roles, not realm roles.
    /// The roles are added by making a POST request to the appropriate Keycloak endpoint.
    ///
    /// Returns Ok(()) on successful role assignment, or Err(ServiceError) on failure.
    ///
    pub async fn add_user_to_role(
        &self,
        user_id: &str,
        client_id: &str,
        roles: &Vec<&KeyCloakRole>,
    ) -> Result<(), KeyCloakError> {
        let url = format!(
            "{}/auth/admin/realms/{}/users/{}/role-mappings/clients/{}",
            &self.host_name, &self.realm, user_id, client_id
        );
        let _ = self
            .post::<&Vec<&KeyCloakRole>, ()>(&url, None, None, Some(roles))
            .await?;
        Ok(())
    }
    /// Validates and decodes a Keycloak user token using the provided public key.
    ///
    /// The function constructs a full PEM formatted public key from the public_key string, defines the token validation
    /// criteria (set to use the RS256 algorithm), and attempts to decode the user token with the constructed public key
    /// and validation criteria. If decoding is successful, the function parses and returns the claims (payload) of the
    /// token as KeyCloakClaims.
    ///
    /// # Parameters
    /// - user_token: The JWT (JSON Web Token) string representing the user's Keycloak token.
    /// - public_key: The public key string (in PEM format without header/footer) used for token validation.
    /// - audience:   Math the aud filed of the claim.  
    ///
    /// # Returns
    /// - A Result containing the token's claims (KeyCloakClaims) on successful validation and decoding, or an
    /// Err(ServiceError) if any step in the process fails.
    ///
    /// # Example Usage:
    /// ```
    /// let claims = keycloak_client.validate_token("user-token-string", "public-key-string", Some("client"))?;
    /// ```
    pub fn validate_token(
        jwt: &str,
        public_key: &str,
        client_id: &str,
    ) -> Result<Vec<String>, KeyCloakError> {
        let pem_key = format!(
            "-----BEGIN PUBLIC KEY-----\n{}\n-----END PUBLIC KEY-----",
            public_key
        );
        // Construct the decoding key from the provided public key string
        let decoding_key = DecodingKey::from_rsa_pem(pem_key.as_bytes()).map_err(|e| {
            KeyCloakError::new(
                KeyCloakErrorType::WebTokenError,
                &format!("DecodingKey::from_rsa_pem error: {:#?}", &e),
            )
        })?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&["account"]);

        let token: TokenData<Value> = decode::<Value>(&jwt, &decoding_key, &validation)?;
        let claims = KeyCloakProxy::claim_to_roles(&token.claims, client_id)?;

        Ok(claims)
    }

    /// Extracts all roles from the provided KeyCloak claims.
    ///
    /// This function retrieves roles both from the "realm_access" section and the "resource_access"
    /// section specific to the provided `client_id` and the default "account".
    ///
    /// # Arguments
    ///
    /// * `claims`: A reference to the claims provided by KeyCloak, typically in JWT format.
    /// * `client_id`: A string slice representing the client identifier for which resource roles need
    ///                to be extracted.
    ///
    /// # Returns
    ///
    /// A `Result` containing a `Vec<String>` of all the roles found. In case of an error, a `KeyCloakError`
    /// is returned.
    ///
    /// # Example
    ///
    /// ```rust
    /// let claims = get_claims_from_token(token);
    /// let client_id = "my_client_id";
    /// let roles = claim_to_roles(&claims, client_id)?;
    /// ```
    ///
    /// Note: This function assumes a specific structure for the claims and may not work with
    /// non-standard or custom KeyCloak configurations.
    pub fn claim_to_roles(claims: &Value, client_id: &str) -> Result<Vec<String>, KeyCloakError> {
        let realm_roles: Vec<String> = claims
            .get("realm_access")
            .and_then(|ra| ra.get("roles"))
            .and_then(|roles| roles.as_array())
            .map(|roles| {
                roles
                    .iter()
                    .filter_map(|r| r.as_str())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();

        let resource_roles: Vec<String> = claims
            .get("resource_access")
            .and_then(|ra| ra.as_object())
            .map_or(Vec::new(), |resources| {
                resources
                    .iter()
                    .flat_map(|(key, val)| {
                        if key == client_id || key == "account" {
                            val.get("roles")
                                .and_then(|roles| roles.as_array())
                                .map(|roles| {
                                    roles
                                        .iter()
                                        .filter_map(|r| r.as_str())
                                        .map(|s| s.to_string())
                                        .collect::<Vec<String>>()
                                })
                                .unwrap_or_else(Vec::new)
                        } else {
                            Vec::new()
                        }
                    })
                    .collect()
            });
        let mut all_roles = realm_roles;
        all_roles.extend(resource_roles);

        Ok(all_roles)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct KeyCloakLoginData {
    pub access_token: String,
    pub expires_in: u32,
    pub refresh_expires_in: u32,
    pub refresh_token: String,
    #[serde(rename = "not-before-policy")]
    pub not_before_policy: u32,
    pub session_state: String,
    pub scope: String,
}

impl Default for KeyCloakLoginData {
    fn default() -> Self {
        panic!("KeyCloakLoginData::default()...this should never happen");
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserCreateRequest {
    pub username: String,
    pub email: String,
    pub enabled: bool,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct UserResponse {
    pub id: String,
    #[serde(rename = "createdTimestamp")]
    pub created_time_stamp: u64,
    pub username: String,
    pub enabled: bool,
    pub totp: bool,
    #[serde(rename = "emailVerified")]
    pub email_verified: bool,
    pub email: String,
    #[serde(default)]
    pub attributes: HashMap<String, Vec<String>>,
    #[serde(rename = "disableableCredentialTypes")]
    pub disable_credential_types: Vec<String>,
    #[serde(rename = "requiredActions")]
    pub required_actions: Vec<String>,
    #[serde(rename = "notBefore")]
    pub not_before: u64,
    pub access: Access,
}

impl Default for UserResponse {
    fn default() -> Self {
        panic!("UserResponse::default should not be called");
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Access {
    #[serde(rename = "manageGroupMembership")]
    pub manage_group_membership: bool,
    pub view: bool,
    #[serde(rename = "mapRoles")]
    pub map_roles: bool,
    pub impersonate: bool,
    pub manage: bool,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct CredentialRepresentation {
    #[serde(rename = "type")]
    pub field_name: String,
    pub value: String,
    pub temporary: bool,
}
impl Default for CredentialRepresentation {
    fn default() -> Self {
        panic!("PasswordResetData::default should not be called");
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct KeyCloakKeyInfo {
    #[serde(rename = "kid")]
    pub key_id: String,

    #[serde(rename = "kty")]
    pub key_type: String,

    #[serde(rename = "alg")]
    pub algorithm: String,

    #[serde(rename = "use")]
    pub key_use: String,

    #[serde(rename = "n")]
    pub modulus: String,

    #[serde(rename = "e")]
    pub exponent: String,

    #[serde(rename = "x5c")]
    pub x509_cert_chain: Vec<String>,

    #[serde(rename = "x5t")]
    pub x509_cert_thumbprint: String,

    #[serde(rename = "x5t#S256")]
    pub x509_cert_thumbprint_s256: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct KeyCloakKeyResponse {
    pub keys: Vec<KeyCloakKeyInfo>,
}

impl Default for KeyCloakKeyResponse {
    fn default() -> Self {
        panic!(
            "KeyCloakKeyResponse::default should not be called.  \
            this exists because the proxy needs the trait for all returned objects"
        );
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyCloakClaims {
    #[serde(rename = "acr")]
    pub authentication_context_class: String,

    #[serde(rename = "azp")]
    pub authorized_party: String,

    #[serde(rename = "email")]
    pub user_email: String,

    #[serde(rename = "email_verified")]
    pub is_email_verified: bool,

    #[serde(rename = "exp")]
    pub expiration_time: u64,

    #[serde(rename = "iat")]
    pub issued_at: u64,

    #[serde(rename = "iss")]
    pub issuer: String,

    #[serde(rename = "jti")]
    pub jwt_id: String,

    #[serde(rename = "preferred_username")]
    pub preferred_username: String,

    #[serde(rename = "scope")]
    pub scope: String,

    #[serde(rename = "session_state")]
    session_state: String,

    #[serde(rename = "sid")]
    pub session_id: String,

    #[serde(rename = "sub")]
    pub subject: String,

    #[serde(rename = "typ")]
    pub token_type: String,
}
impl Default for KeyCloakClaims {
    fn default() -> Self {
        panic!(
            "JwtClaims::default should not be called.  \
            this exists because the proxy needs the trait for all returned objects"
        );
    }
}

pub struct KeyCloakGroup {
    pub id: String,
    pub name: String,
    pub path: String,
}

impl Default for KeyCloakGroup {
    fn default() -> Self {
        panic!(
            "KeyCloakGroup::default should not be called.  \
            this exists because the proxy needs the trait for all returned objects"
        );
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyCloakRole {
    #[serde(rename = "id")]
    pub role_id: String,

    #[serde(rename = "name")]
    pub role_name: String,

    #[serde(rename = "description", default)]
    pub role_description: Option<String>,

    #[serde(rename = "composite")]
    pub is_composite: bool,

    #[serde(rename = "clientRole")]
    pub is_client_role: bool,

    #[serde(rename = "containerId")]
    pub container_id: String,
}
impl Default for KeyCloakRole {
    fn default() -> Self {
        panic!(
            "KeyCloakRole::default should not be called.  \
            this exists because the proxy needs the trait for all returned objects"
        );
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GroupResponse {
    pub id: String,
    pub name: String,
    pub path: String,
    pub attributes: Option<HashMap<String, Vec<String>>>,
    pub sub_groups: Option<Vec<GroupResponse>>,
}
impl Default for GroupResponse {
    fn default() -> Self {
        Self {
            id: String::default(),
            name: String::default(),
            path: String::default(),
            attributes: None,
            sub_groups: None,
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    use crate::middleware::service_config::SERVICE_CONFIG;

    #[tokio::test]
    async fn test_keycloak_flow() {
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "info");
        }
        env_logger::init();

        // 1. Get Admin Token

        let mut proxy = KeyCloakProxy::new(&SERVICE_CONFIG.key_cloak_host, "master");
        let login_info = proxy
            .login(
                &SERVICE_CONFIG.key_cloak_admin_user_name,
                &SERVICE_CONFIG.key_cloak_admin_password,
                None,
                None, // default scopes
            )
            .await
            .expect("admin login shoudl work");

        let admin_token = &login_info.access_token;
        proxy.set_auth_token(Some(admin_token.to_string()));
        proxy.set_realm(&SERVICE_CONFIG.key_cloak_realm);

        // 1a: get the id for the client CLIENT_NAME
        let client_uuid = proxy
            .get_client_id(&SERVICE_CONFIG.key_cloak_client_id)
            .await
            .expect("the client should be configured in KeyCloak");

        full_info!("Client_uuid: {}", client_uuid);

        //1b. Get the roles for the keycloak "client"

        let roles = proxy
            .get_roles(Some(&client_uuid))
            .await
            .expect("the admin should be able to get the roles");

        let test_role = roles
            .iter()
            .find(|r| r.role_name == "TestUser")
            .expect("The CatanService should be configured to have a TestRole");

        full_info!("TestUser role_id: {}", test_role.role_id);

        // 2. Create Test User
        let test_user = UserCreateRequest {
            username: "test_user".to_string(),
            email: "test@user.com".to_string(),
            enabled: true,
        };

        let result = proxy.create_user(&test_user).await;
        match result {
            Ok(_) => {
                full_info!(
                    "test_user created successfully username: {}",
                    test_user.username
                )
            }
            Err(e) => {
                if e.error_type != KeyCloakErrorType::KeyCloakServiceError {
                    panic!("unexpected return from creat_user: {:#?}", e);
                }
            }
        }
        let user_response = proxy.get_user_profile(&test_user.username).await;
        let profiles = user_response.expect("get_user_profile should have succeeded");
        assert!(profiles.len() == 1);
        let profile = &profiles[0];

        let user_id = profile.id.clone();
        full_info!("user_id: {}", user_id);

        //2a Set the user password -- this isn't done via create, but via a PUT to the account
        let result = proxy
            .set_password(&user_id, &SERVICE_CONFIG.key_cloak_test_password)
            .await;
        if result.is_err() {
            full_info!("set_password returned error: {:#?}", result);
            panic!("can't continue");
        }

        // 2b. Put the user in the TestRole

        let result = proxy
            .add_user_to_role(&user_id, &client_uuid, &vec![test_role])
            .await;
        if result.is_err() {
            panic!("role assignment can't fail: {:#?}", result);
        }

        // 3. Login as Test User
        proxy.set_auth_token(None);

        let login_info = proxy
            .login(
                &profile.email,
                &SERVICE_CONFIG.key_cloak_test_password,
                Some(&SERVICE_CONFIG.key_cloak_client_id),
                None,
            )
            .await
            .expect("admin login should work");

        let test_auth_token = login_info.access_token.clone();
        assert!(test_auth_token.len() > 10);

        // 4. Verify the token using Keycloak's public key

        // 4a get the keyclock keys
        proxy.set_auth_token(None);
        let pk = proxy
            .get_keycloak_public_key()
            .await
            .expect("get public key should work");

        let claims = KeyCloakProxy::validate_token(
            &test_auth_token,
            &pk,
            &SERVICE_CONFIG.key_cloak_client_id,
        )
        .expect("test token should be valid");

        assert!(claims.contains(&"TestUser".to_string()));
        proxy.set_auth_token(Some(admin_token.to_string()));

        // 4b. get the key cloak keys...we don't use them, but make sure we can get them.

        let response = proxy.get_keys().await;
        let _key_data: KeyCloakKeyResponse = match response {
            Ok(k) => k.clone(),
            Err(service_error) => {
                panic!("error getting keys: {:#?}", service_error);
            }
        };

        // 5. (Optional) Logout User - skipping for this example

        // 6. Delete Test User
        // reqwest::Client::new()
        //     .delete(&format!(
        //         "{}/auth/admin/realms/{}/users/{}",
        //         KEYCLOAK_HOST, REALM, user_id
        //     ))
        //     .bearer_auth(admin_token)
        //     .send()
        //     .await
        //     .unwrap();
    }
}
