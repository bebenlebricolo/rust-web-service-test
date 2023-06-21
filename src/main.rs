#![feature(decl_macro)]
use std::{io::Cursor, path::PathBuf, sync::Arc};

use rocket::{
    get, post,
    http::{Header, Status},
    response::Redirect,
    routes, uri, Response, State,
    outcome::Outcome,
    request::{self, FromRequest},
    response::{ Responder},
    FromForm, Request,
};
use rocket_contrib::json::Json;

use serde::{Serialize, Deserialize};
use utoipa::{openapi::{self, security::{OAuth2, Implicit, Scopes, Flow, Password, AuthorizationCode}}, OpenApi, ToSchema};
use utoipa_swagger_ui::Config;
use utoipa::{
    openapi::security::{ApiKey, ApiKeyValue, SecurityScheme},
    Modify,
};

fn main() {
    // #[derive(Debug)]
    #[derive(OpenApi)]
    #[openapi(
        paths(hello),
        components(
            // Required schemas need to be declared here
            // (otherwise openapi spec does not contain it and
            // types won't be advertised, nor be parsed from endpoints)
            schemas(InputParams)
        ),
        security(
            (),
            ("my_auth" = ["read:items", "edit:items"]),
            ("api_oauth2_flow" = ["edit:items", "read:items"])
        ),
        modifiers(&SecurityAddon)
    )]
    struct ApiDoc;

    struct SecurityAddon;

    impl Modify for SecurityAddon {
        fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
            let components = openapi.components.as_mut().unwrap(); // we can unwrap safely since there already is components registered.
            components.add_security_scheme( "api_oauth2_flow",
                           SecurityScheme::OAuth2(OAuth2::new([Flow::Password(
                            Password::with_refresh_url(
                                "https://localhost/oauth/token",
                                Scopes::from_iter([
                                    ("edit:items", "edit my items"),
                                    ("read:items", "read my items")
                                ]),
                                "https://localhost/refresh/token"
                            )),
                            Flow::AuthorizationCode(
                                AuthorizationCode::new(
                                "https://accounts.google.com/o/oauth2/auth",
                                "https://oauth2.googleapis.com/token",
                                Scopes::from_iter([
                                    ("https://www.googleapis.com/auth/cloud-platform", "Cloud platform access"),
                                    ("https://www.googleapis.com/auth/userinfo.email", "User email access"),
                                    ("https://www.googleapis.com/auth/userinfo.profile", "User profile access"),
                                    ("openid", "OpenID, required to generate an openId Jwt token")
                                ])),
                           ),
                        ])))
        }
    }

    rocket::ignite()
        .manage(Arc::new(Config::from("/api-docs/openapi.json")))
        .manage(ApiDoc::openapi())
        .mount("/", routes![hello, serve_api_doc, serve_swagger, redirect])
        .launch();
}

#[get("/swagger/<tail..>")]
fn serve_swagger(tail: PathBuf, config: State<Arc<Config>>) -> Response<'static> {
    match utoipa_swagger_ui::serve(tail.as_os_str().to_str().unwrap(), config.clone()) {
        Ok(file) => file
            .map(|file| {
                Response::build()
                    .sized_body(Cursor::new(file.bytes.to_vec()))
                    .header(Header::new("Content-Type", file.content_type))
                    .finalize()
            })
            .unwrap_or_else(|| Response::build().status(Status::NotFound).finalize()),
        Err(error) => {
            let error = error.to_string();
            let len = error.len() as u64;

            Response::build()
                .raw_body(rocket::response::Body::Sized(Cursor::new(error), len))
                .status(Status::InternalServerError)
                .finalize()
        }
    }
}

#[get("/api-docs/openapi.json")]
fn serve_api_doc(openapi: State<utoipa::openapi::OpenApi>) -> Response<'static> {
    let json_string = serde_json::to_string(openapi.inner()).unwrap();
    let len = json_string.len() as u64;

    Response::build()
        .raw_body(rocket::response::Body::Sized(Cursor::new(json_string), len))
        .header(Header::new("Content-Type", "application/json"))
        .finalize()
}

// Add proper redirection for use cases where user directly connects to api's root
#[get("/")]
fn redirect() -> Redirect {
    return Redirect::to(uri!(serve_swagger : "index.html"));
}


#[derive(serde::Serialize, serde::Deserialize, ToSchema)]
pub struct InputParams {
    name : String,
    age : u8
}

// Todo operation error.
#[derive(Serialize, ToSchema, Responder, Debug)]
pub enum  ParsingError {
    /// When there is conflict creating a new todo.
    #[response(status = 500)]
    InternalServerError(String),

    /// When todo item is not found from storage.
    #[response(status = 200)]
    Ok(String),
}

// impl<'r> FromRequest<'r,'r> for InputParams {
//     type Error = ParsingError;
//     fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
//         request.
//         let params = InputParams::from(request);
//         return redirect::rocket::Outcome::Success(params);
//     }
// }

#[utoipa::path(
    tag = "JsonData - Hello handlers",
    responses(
        (status = 200, description = "Hello response for given value", body = String, content_type = "text/plain", example = json!("Hello John !")),
        (status = 404, description = "resource missing"),
        (status = "5XX", description = "server error"),
        (status = StatusCode::INTERNAL_SERVER_ERROR, description = "internal server error"),
        (status = IM_A_TEAPOT, description = "happy easter")
    ),
    request_body(content = InputParams, description = "Say hello by value", content_type = "application/json"),
    security(
        (),
        ("my_auth" = ["read:items", "edit:items"]),
        ("token_jwt" = [])
    )
)]
#[post("/hello", data="<input>")]
fn hello(input: Json<InputParams>) -> String {
    format!("Hello {}, age : {}!", input.name, input.age)
}
