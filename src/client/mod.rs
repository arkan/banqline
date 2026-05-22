pub mod error;
mod types;
pub use types::*;

use std::sync::Arc;

use anyhow::{Context, Result};
use reqwest::Client as ReqwestClient;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::client::error::ClientError;

const DEFAULT_BASE_URL: &str = "https://api.enablebanking.com";

pub type JwtProvider = Arc<dyn Fn() -> Result<String> + Send + Sync>;

pub struct Client {
    base_url: String,
    http: ReqwestClient,
    jwt_fn: Option<JwtProvider>,
}

impl Client {
    pub fn new(base_url: Option<String>, jwt_fn: Option<JwtProvider>) -> Self {
        Client {
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
            http: ReqwestClient::new(),
            jwt_fn,
        }
    }

    async fn add_auth(&self, req: reqwest::RequestBuilder) -> Result<reqwest::RequestBuilder> {
        match &self.jwt_fn {
            Some(fn_) => {
                let token = fn_().context("obtain jwt")?;
                Ok(req.header("Authorization", format!("Bearer {}", token)))
            }
            None => Ok(req),
        }
    }

    async fn do_get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let req = self.http.get(&url);
        let req = self.add_auth(req).await?;

        let resp = req.send().await.context("execute request")?;
        let status = resp.status();
        let resp_body = resp.text().await.context("read response body")?;

        if !status.is_success() {
            return Err(ClientError::Api {
                method: "GET".into(),
                path: path.to_string(),
                status: status.as_u16(),
                body: resp_body,
            }
            .into());
        }

        serde_json::from_str(&resp_body).context("decode response")
    }

    async fn do_get_query<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let req = self.http.get(&url).query(params);
        let req = self.add_auth(req).await?;

        let resp = req.send().await.context("execute request")?;
        let status = resp.status();
        let resp_body = resp.text().await.context("read response body")?;

        if !status.is_success() {
            return Err(ClientError::Api {
                method: "GET".into(),
                path: path.to_string(),
                status: status.as_u16(),
                body: resp_body,
            }
            .into());
        }

        serde_json::from_str(&resp_body).context("decode response")
    }

    async fn do_post<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let req = self.http.post(&url).json(body);
        let req = self.add_auth(req).await?;

        let resp = req.send().await.context("execute request")?;
        let status = resp.status();
        let resp_body = resp.text().await.context("read response body")?;

        if !status.is_success() {
            return Err(ClientError::Api {
                method: "POST".into(),
                path: path.to_string(),
                status: status.as_u16(),
                body: resp_body,
            }
            .into());
        }

        serde_json::from_str(&resp_body).context("decode response")
    }

    pub async fn list_aspsps(&self, country: &str) -> Result<Vec<Aspsp>> {
        let path = format!("/aspsps?country={}", country);
        let resp: ListAspspsResponse = self.do_get(&path).await.context("list aspsps")?;
        Ok(resp.aspsps)
    }

    pub async fn authorize(&self, req: &AuthRequest) -> Result<AuthResponse> {
        self.do_post("/auth", req).await.context("authorize")
    }

    pub async fn create_session(&self, code: &str) -> Result<Session> {
        let payload = serde_json::json!({ "code": code });
        self.do_post("/sessions", &payload)
            .await
            .context("create session")
    }

    pub async fn get_session(&self, session_id: &str) -> Result<Session> {
        let path = format!("/sessions/{}", session_id);
        self.do_get(&path).await.context("get session")
    }

    pub async fn get_account_details(&self, account_id: &str) -> Result<Account> {
        let path = format!("/accounts/{}/details", account_id);
        self.do_get(&path).await.context("get account details")
    }

    pub async fn get_balances(&self, account_id: &str) -> Result<Vec<Balance>> {
        let path = format!("/accounts/{}/balances", account_id);
        let resp: BalancesResponse = self.do_get(&path).await.context("get balances")?;
        Ok(resp.balances)
    }

    pub async fn get_transactions(
        &self,
        account_id: &str,
        opts: &TransactionOpts,
    ) -> Result<TransactionList> {
        let path = format!("/accounts/{}/transactions", account_id);

        let mut params: Vec<(&str, &str)> = Vec::new();
        if let Some(ref v) = opts.date_from {
            params.push(("date_from", v));
        }
        if let Some(ref v) = opts.date_to {
            params.push(("date_to", v));
        }
        if let Some(ref v) = opts.status {
            params.push(("transaction_status", v));
        }
        if let Some(ref v) = opts.continuation_key {
            params.push(("continuation_key", v));
        }

        if params.is_empty() {
            self.do_get(&path).await.context("get transactions")
        } else {
            self.do_get_query(&path, &params)
                .await
                .context("get transactions")
        }
    }

    pub async fn get_all_transactions(
        &self,
        account_id: &str,
        opts: &TransactionOpts,
    ) -> Result<Vec<Transaction>> {
        let mut opts = opts.clone();
        let mut all = Vec::new();
        let mut seen_continuation_keys = std::collections::HashSet::new();

        loop {
            let result = self.get_transactions(account_id, &opts).await?;
            all.extend(result.transactions);

            match result.continuation_key {
                Some(key) if !key.is_empty() => {
                    if !seen_continuation_keys.insert(key.clone()) {
                        anyhow::bail!("repeated continuation key while fetching transactions");
                    }
                    opts.continuation_key = Some(key);
                }
                _ => break,
            }
        }

        Ok(all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::thread;

    fn transaction(id: &str) -> serde_json::Value {
        json!({
            "entry_reference": format!("entry-{id}"),
            "transaction_id": id,
            "transaction_amount": { "amount": "1.00", "currency": "EUR" },
            "booking_date": "2026-05-08",
            "value_date": "2026-05-08",
            "transaction_date": "2026-05-08",
            "remittance_information": [format!("txn {id}")],
            "creditor": { "name": "Creditor" },
            "debtor": { "name": "Debtor" },
            "status": "BOOK",
            "credit_debit_indicator": "DBIT",
            "note": ""
        })
    }

    fn spawn_server(
        responses: Vec<serde_json::Value>,
    ) -> (String, thread::JoinHandle<Vec<String>>) {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let base_url = format!("http://{}", server.server_addr());
        let handle = thread::spawn(move || {
            let mut urls = Vec::new();
            for body in responses {
                let request = server.recv().unwrap();
                urls.push(request.url().to_string());
                request
                    .respond(tiny_http::Response::from_string(body.to_string()))
                    .unwrap();
            }
            urls
        });
        (base_url, handle)
    }

    #[tokio::test]
    async fn get_all_transactions_paginates_until_empty_continuation() {
        let (base_url, handle) = spawn_server(vec![
            json!({ "transactions": [transaction("t1")], "continuation_key": "next+page" }),
            json!({ "transactions": [transaction("t2")], "continuation_key": "" }),
        ]);
        let client = Client::new(Some(base_url), None);

        let txns = client
            .get_all_transactions("acc-001", &TransactionOpts::default())
            .await
            .unwrap();

        assert_eq!(txns.len(), 2);
        assert_eq!(txns[0].transaction_id, "t1");
        assert_eq!(txns[1].transaction_id, "t2");
        let urls = handle.join().unwrap();
        assert_eq!(urls[0], "/accounts/acc-001/transactions");
        assert_eq!(
            urls[1],
            "/accounts/acc-001/transactions?continuation_key=next%2Bpage"
        );
    }

    #[tokio::test]
    async fn get_all_transactions_rejects_repeated_continuation_key() {
        let (base_url, handle) = spawn_server(vec![
            json!({ "transactions": [transaction("t1")], "continuation_key": "same" }),
            json!({ "transactions": [transaction("t2")], "continuation_key": "same" }),
        ]);
        let client = Client::new(Some(base_url), None);

        let err = client
            .get_all_transactions("acc-001", &TransactionOpts::default())
            .await
            .unwrap_err();

        assert!(err.to_string().contains("repeated continuation key"));
        let urls = handle.join().unwrap();
        assert_eq!(urls.len(), 2);
    }

    #[test]
    fn transaction_list_accepts_missing_continuation_key() {
        let decoded: TransactionList = serde_json::from_value(json!({
            "transactions": [transaction("t1")]
        }))
        .unwrap();

        assert!(decoded.continuation_key.is_none());
    }

    #[test]
    fn transaction_list_accepts_null_continuation_key() {
        let decoded: TransactionList = serde_json::from_value(json!({
            "transactions": [transaction("t1")],
            "continuation_key": null
        }))
        .unwrap();

        assert!(decoded.continuation_key.is_none());
    }

    #[test]
    fn transaction_accepts_null_optional_fields() {
        let decoded: TransactionList = serde_json::from_value(json!({
            "transactions": [{
                "entry_reference": null,
                "transaction_id": "t1",
                "transaction_amount": { "amount": "1.00", "currency": "EUR" },
                "booking_date": null,
                "value_date": null,
                "transaction_date": null,
                "remittance_information": null,
                "creditor": null,
                "debtor": { "name": null },
                "status": "BOOK",
                "credit_debit_indicator": "DBIT",
                "note": null
            }]
        }))
        .unwrap();

        let txn = &decoded.transactions[0];
        assert_eq!(txn.entry_reference, "");
        assert_eq!(txn.booking_date, "");
        assert!(txn.remittance_info.is_empty());
        assert_eq!(txn.creditor.name, "");
        assert_eq!(txn.debtor.name, "");
        assert_eq!(txn.note, "");
    }
}
