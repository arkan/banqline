use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("api {method} {path}: status {status}: {body}")]
    Api {
        method: String,
        path: String,
        status: u16,
        body: String,
    },

    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
}
