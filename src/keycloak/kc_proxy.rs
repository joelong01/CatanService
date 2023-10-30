#![allow(dead_code)]
use core::panic;
use jsonwebtoken::{decode, Algorithm, DecodingKey, TokenData, Validation};
use reqwest::{Client, Method, RequestBuilder};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fmt::Debug;

use crate::{full_info, shared::shared_models::ServiceError};

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
    pub fn set_auth_token(&mut self, auth_token: Option<String>) {
        self.auth_token = auth_token.clone();
    }
    pub fn set_realm(&mut self, realm: &str) {
        self.realm = realm.to_string();
    }

    ///
    ///  send the actual request to the KeyCloak service.  this will take care of authentication (using the self.auth_token)
    ///  it also creates the proper headers and body for the call, and parses the return values.
    ///  todo: properly implement token refresh.
    pub async fn send_request<B, T>(
        &self,
        method: Method,
        url: &str,
        headers: Option<&HashMap<String, String>>,
        params: Option<&[(&str, &str)]>,
        body: Option<B>,
    ) -> Result<T, ServiceError>
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
                return Err(ServiceError::new_internal_server_fault(
                    "Unsupported HTTP method",
                ))
            }
        };

        // Process bodies and headers for PUT and POST methods
        if method == Method::PUT || method == Method::POST {
            if let Some(form_params) = params {
                request = request.form(form_params);
                request = request.header("Content-Type", "application/x-www-form-urlencoded");
            } else if let Some(body_content) = body {
                let serialized_body = serde_json::to_string(&body_content).map_err(|e| {
                    ServiceError::new_json_error(
                        &format!("Error serializing request body for {:#?}", &body_content),
                        &e,
                    )
                })?;

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

        let response = request.send().await.map_err(|e| {
            ServiceError::new_reqwest_error(&format!("Error sending request: {}", url), &e)
        })?;

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
                    return Err(ServiceError::new_http_error(
                        &format!("{:#?}", status),
                        status,
                    ));
                }
            }
            return Err(ServiceError::new_internal_server_fault(
                "Expected body, but found none",
            ));
        }

        if status.is_success() {
            let parsed_body = serde_json::from_slice::<T>(&body).map_err(|e| {
                ServiceError::new_json_error(
                    &format!("Error deserializing response for {:#?}", &body),
                    &e,
                )
            })?;

            Ok(parsed_body)
        } else {
            Err(ServiceError::new_http_error(
                &format!("{:#?}", body),
                status,
            ))
        }
    }

    pub async fn post<B, T>(
        &self,
        url: &str,
        headers: Option<&HashMap<String, String>>,
        params: Option<&[(&str, &str)]>,
        body: Option<B>,
    ) -> Result<T, ServiceError>
    where
        B: Serialize + Debug,
        T: DeserializeOwned + 'static + Default,
    {
        self.send_request(Method::POST, url, headers, params, body)
            .await
    }
    pub async fn put<B, T>(
        &self,
        url: &str,
        headers: Option<&HashMap<String, String>>,
        params: Option<&[(&str, &str)]>,
        body: Option<B>,
    ) -> Result<T, ServiceError>
    where
        B: Serialize + Debug,
        T: DeserializeOwned + 'static + Default,
    {
        self.send_request(Method::PUT, url, headers, params, body)
            .await
    }
    pub async fn get<T>(
        &self,
        url: &str,
        headers: Option<&HashMap<String, String>>,
        params: Option<&[(&str, &str)]>,
    ) -> Result<T, ServiceError>
    where
        T: DeserializeOwned + 'static + Default,
    {
        self.send_request::<(), T>(Method::GET, url, headers, params, None)
            .await
    }

    ///
    /// Logs the user into the KeyCloak service and returns the login data

    pub async fn login(
        &self,
        user_name: &str,
        password: &str,
    ) -> Result<KeyCloakLoginData, ServiceError> {
        let url = format!(
            "{}/auth/realms/{}/protocol/openid-connect/token",
            &self.host_name, &self.realm,
        );
        let params = [
            ("grant_type", "password"),
            ("client_id", "admin-cli"),
            ("username", user_name),
            ("password", password),
        ];

        let login_data = self
            .post::<(), KeyCloakLoginData>(&url, None, Some(&params), None)
            .await?;

        Ok(login_data)
    }
    pub async fn create_user(&self, user_data: &UserCreateRequest) -> Result<(), ServiceError> {
        let url = format!(
            "{}/auth/admin/realms/{}/users",
            &self.host_name, &self.realm
        );
        let _ = self
            .post::<&UserCreateRequest, ()>(&url, None, None, Some(user_data))
            .await?;
        Ok(())
    }

    pub async fn get_user_profile(
        &self,
        user_name: &str,
    ) -> Result<Vec<UserResponse>, ServiceError> {
        let url = format!(
            "{}/auth/admin/realms/{}/users?username={}",
            &self.host_name, &self.realm, &user_name
        );
        let response = self.get::<Vec<UserResponse>>(&url, None, None).await?;
        Ok(response)
    }

    pub async fn set_password(&self, user_id: &str, password: &str) -> Result<(), ServiceError> {
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

    pub async fn get_keys(&self) -> Result<KeyCloakKeyResponse, ServiceError> {
        let url = format!(
            "{}/auth/realms/{}/protocol/openid-connect/certs",
            &self.host_name, &self.realm
        );
        let response = self.get::<KeyCloakKeyResponse>(&url, None, None).await?;
        Ok(response)
    }

    pub async fn get_keycloak_public_key(&self) -> Result<String, ServiceError> {
        let url = format!("{}/auth/realms/{}", &self.host_name, &self.realm);

        let value = self.get::<Value>(&url, None, None).await?;
        let pk = value["public_key"].as_str().unwrap();
        Ok(pk.to_string())
    }

    pub async fn get_groups(&self) -> Result<String, ServiceError> {
        let url = format!("{}/auth/realms/{}", &self.host_name, &self.realm);

        let value = self.get::<Value>(&url, None, None).await?;
        let pk = value["public_key"].as_str().unwrap();
        Ok(pk.to_string())
    }

    pub async fn get_roles(&self) -> Result<Vec<KeyCloakRole>, ServiceError> {
        let url = format!(
            "{}/auth/admin/realms/{}/roles",
            &self.host_name, &self.realm
        );
        let roles: Vec<KeyCloakRole> = self.get(&url, None, None).await?;
        Ok(roles)
    }

    pub async fn add_user_to_role(
        &self,
        user_id: &str,
        roles: &Vec<&KeyCloakRole>,
    ) -> Result<(), ServiceError> {
        let url = format!(
            "{}/auth/admin/realms/{}/users/{}/role-mappings/realm",
            &self.host_name, &self.realm, &user_id
        );
        let _ = self
            .post::<&Vec<&KeyCloakRole>, ()>(&url, None, None, Some(roles))
            .await?;
        Ok(())
    }

    pub fn validate_token(
        user_token: &str,
        public_key: &str,
    ) -> Result<KeyCloakClaims, ServiceError> {
        let pem_key = format!(
            "-----BEGIN PUBLIC KEY-----\n{}\n-----END PUBLIC KEY-----",
            public_key
        );
        // Construct the decoding key from the provided public key string
        let decoding_key = DecodingKey::from_rsa_pem(pem_key.as_bytes())
            .map_err(|e| ServiceError::new_std_error("DecodingKey::from_rsa_pem", &e))?;

        // Define validation criteria
        let mut validation = Validation::new(Algorithm::RS256);
        validation.algorithms = vec![Algorithm::RS256];

        // Attempt to decode the token
        let token_data: TokenData<Value> = decode(&user_token, &decoding_key, &validation)
            .map_err(|e| ServiceError::new_std_error("decode error:", &e))?;
        full_info!("{:#?}", &token_data.claims);
        let key_cloak_claims = serde_json::from_value(token_data.claims)
            .map_err(|e| ServiceError::new_json_error("deserializing key cloak claims", &e))?;
        // Return the claims

        Ok(key_cloak_claims)
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
