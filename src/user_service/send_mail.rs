use reqwest::Error;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct EmailData {
    #[serde(rename = "headers")]
    headers: Headers,
    #[serde(rename = "senderAddress")]
    sender_address: String,
    #[serde(rename = "content")]
    content: Content,
    #[serde(rename = "recipients")]
    recipients: Recipients,
    #[serde(rename = "attachments")]
    attachments: Vec<Attachment>,
    #[serde(rename = "replyTo")]
    reply_to: Vec<EmailRecipient>,
    #[serde(rename = "userEngagementTrackingDisabled")]
    user_engagement_tracking_disabled: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Headers {
    #[serde(rename = "ClientCorrelationId")]
    client_correlation_id: String,
    #[serde(rename = "ClientCustomHeaderName")]
    client_custom_header_name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Content {
    #[serde(rename = "subject")]
    subject: String,
    #[serde(rename = "plainText")]
    plain_text: String,
    #[serde(rename = "html")]
    html: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Recipients {
    #[serde(rename = "to")]
    to: Vec<EmailRecipient>,
    #[serde(rename = "cc")]
    cc: Vec<EmailRecipient>,
    #[serde(rename = "bcc")]
    bcc: Vec<EmailRecipient>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EmailRecipient {
    #[serde(rename = "address")]
    address: String,
    #[serde(rename = "displayName")]
    display_name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Attachment {
    #[serde(rename = "name")]
    name: String,
    #[serde(rename = "contentType")]
    content_type: String,
    #[serde(rename = "contentInBase64")]
    content_in_base64: String,
}
// "https://contoso.westus.communications.azure.com/emails:send?api-version=2023-03-31"
#[allow(dead_code)]
pub async fn send_email(
    mail_svc_url: &str,
    token: &str,
    validation_url: &str,
    to_address: &str,
    name: &str,
) -> Result<(), Error> {
    let json_body = build_json_doc(validation_url, to_address, name).unwrap();
    let client = reqwest::Client::new();
    let res = client
        .post(mail_svc_url)
        .header("Content-Type", "application/json")
        .header("Authorization", token)
        .body(json_body)
        .send()
        .await?;

    if res.status().is_success() {
        println!("Email sent successfully.");
    } else {
        let error_msg = format!("Failed to send email: {}", res.status());
        println!("{}", error_msg);
    }

    Ok(())
}

pub fn build_json_doc(
    url: &str,
    to_address: &str,
    name: &str,
) -> Result<String, serde_json::Error> {
    let body = format!("You have registered as a user of the Catan Services.  Click on this url to validate your email.\n{}", url);
    let email_data = EmailData {
        headers: Headers {
            client_correlation_id: "1".to_string(),
            client_custom_header_name: "".to_string(),
        },
        sender_address: "no_replay@longshotdev.com".to_string(),

        content: Content {
            subject: "Catan Register User".to_string(),
            plain_text: body.clone(),
            html: format!(
                "<html><head><title>Validate Email</title></head><body><h1>{}</h1></body></html>",
                body
            ),
        },
        recipients: Recipients {
            to: vec![EmailRecipient {
                address: to_address.to_string(),
                display_name: name.to_string(),
            }],
            cc: vec![],
            bcc: vec![],
        },
        attachments: vec![],
        reply_to: vec![EmailRecipient {
            address: "no_reply@longshotdev.com".to_string(),
            display_name: "Dot Not Reply".to_string(),
        }],
        user_engagement_tracking_disabled: false,
    };

    serde_json::to_string(&email_data)
}
