/**
 * this module implements the WebApi to create the database/collection, list all the users, and to create/find/delete
 * a User document in CosmosDb
 */
use crate::cosmos_db::cosmosdb::UserDb;
use crate::shared::models::{PartialUser, User, ServiceResponse};
use crate::shared::utility::{COLLECTION_NAME, DATABASE_NAME};
use actix_web::{web, HttpResponse};
use azure_core::StatusCode;




/**
 *  this will get a list of all documents.  Note this does *not* do pagination. This would be a reasonable next step to
 *  show in the sample
 */
pub async fn list_users() -> HttpResponse {
    //
    //  this match should always succeed as it is tested in main()
    let userdb = UserDb::new(DATABASE_NAME, COLLECTION_NAME).await;

    // Get list of users
    match userdb.list().await {
        Ok(users) => HttpResponse::Ok()
            .content_type("application/json")
            .json(users),
        Err(err) => {
            let response = ServiceResponse {
                message: format!("Failed to retrieve user list: {}", err),
                status: StatusCode::NotFound,
                body: "".to_owned(),
            };
            HttpResponse::NotFound()
                .content_type("application/json")
                .json(response)
        }
    }
}
/**
 *  this will get a list of all documents.  Note this does *not* do pagination. This would be a reasonable next step to
 *  show in the sample
 */
pub async fn find_user_by_id(id: web::Path<String>) -> HttpResponse {
    //
    //  this match should always succeed as it is tested in main()
    let userdb = UserDb::new(DATABASE_NAME, COLLECTION_NAME).await;

    // Get list of users
    match userdb.find_user(&id).await {
        Ok(user) => HttpResponse::Ok()
            .content_type("application/json")
            .json(user),
        Err(err) => {
            let response = ServiceResponse {
                message: format!("Failed to find user: {}", err),
                status: StatusCode::NotFound,
                body: "".to_owned(),
            };
            HttpResponse::NotFound()
                .content_type("application/json")
                .json(response)
        }
    }
}
/**
 * this sets up CosmosDb to make the sample run. the only prereq is the secrets set in
 * .devconainter/required-secrets.json, this API will call setupdb. this just calls the setupdb api and deals with errors
 */
pub async fn setup() -> HttpResponse {
    let userdb = UserDb::new(DATABASE_NAME, COLLECTION_NAME).await;
    match userdb.setupdb().await {
        Ok(..) => {
            let response = ServiceResponse {
                message: format!(
                    "database: {} collection: {} \ncreated",
                    DATABASE_NAME, COLLECTION_NAME
                ),
                status: StatusCode::Ok,
                body: "".to_owned(),
            };
            HttpResponse::Ok()
                .content_type("application/json")
                .json(response)
        }
        Err(err) => {
            let response = ServiceResponse {
                message: format!("Failed to create database/collection: {}", err),
                status: StatusCode::BadRequest,
                body: "".to_owned(),
            };
            HttpResponse::BadRequest()
                .content_type("application/json")
                .json(response)
        }
    }
}

/**
 *  this creates a user.  it uses web forms to collect the data from the client.  Note that if you are using PostMan
 *  to call this API, set the form data in 'x-www-form-urlencoded', *not* in 'form-data', as that will fail with a
 *  hard-to-figure-out error in actix_web deserialize layer.
 */
pub async fn create(user_req: web::Form<PartialUser>) -> HttpResponse {
    let pp: PartialUser = user_req.into_inner();
    let user: User = pp.into();
    let userdb = UserDb::new(DATABASE_NAME, COLLECTION_NAME).await;
    match userdb.create_user(user.clone()).await {
        Ok(..) => {
            HttpResponse::Ok()
                .content_type("application/json")
                .json(user)
        }
        Err(err) => {
            let response = ServiceResponse {
                message: format!("Failed to add user to collection: {}", err),
                status: StatusCode::BadRequest,
                body: "".to_owned(),
            };
            HttpResponse::BadRequest()
                .content_type("application/json")
                .json(response)
        }
    }
}

pub async fn delete(id: web::Path<String>) -> HttpResponse {
    let userdb = UserDb::new(DATABASE_NAME, COLLECTION_NAME).await;
    match userdb.delete_user(&id).await {
        Ok(..) => {
            let response = ServiceResponse {
                message: format!("deleted user with id: {}", id),
                status: StatusCode::Ok,
                body: "".to_owned(),
            };
            HttpResponse::Ok()
                .content_type("application/json")
                .json(response)
        }
        Err(err) => {
            let response = ServiceResponse {
                message: format!("Failed to delete user: {}", err),
                status: StatusCode::BadRequest,
                body: "".to_owned(),
            };
            HttpResponse::BadRequest()
                .content_type("application/json")
                .json(response)
        }
    }
}
