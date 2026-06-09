use actix_session::Session;
use actix_web::{Error, FromRequest, error::ErrorInternalServerError, web::Data};
use futures_util::{FutureExt, future::LocalBoxFuture};

use crate::services::users::{User, UsersService};

#[derive(Debug)]
pub struct AuthenticatedUser(pub Option<User>);

impl FromRequest for AuthenticatedUser {
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;
    fn from_request<'a>(
        req: &actix_web::HttpRequest,
        payload: &mut actix_web::dev::Payload,
    ) -> Self::Future {
        let req = req.clone();
        let mut payload = payload.take();

        async move {
            let users_service = req
                .app_data::<Data<UsersService>>()
                .cloned()
                .ok_or_else(|| ErrorInternalServerError(""))?;
            let session = Session::from_request(&req, &mut payload).await?;

            if let Some(email) = session.get::<String>("email")? {
                let user = users_service.find_user_by_email(email).await;
                user.map(AuthenticatedUser)
                    .map_err(ErrorInternalServerError)
            } else {
                Ok(AuthenticatedUser(None))
            }
        }
        .boxed_local()
    }
}
