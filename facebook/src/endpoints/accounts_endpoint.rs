use edges::Account;
use error::FacebookError;
use facebook_client::FacebookClientInner;
use paging::Paging;
use std::rc::Rc;

pub struct AccountsEndpoint {
    pub client: Rc<FacebookClientInner>,
}

impl AccountsEndpoint {
    pub fn list(&self) -> Result<Paging<Account>, FacebookError> {
        let client = reqwest::Client::new();

        if let Some(access_token) = self.client.user_access_token.as_ref() {
            let mut resp = client
                .get(&format!("{}/me/accounts", &self.client.base_url))
                .header("Authorization", format!("Bearer {}", &access_token))
                .send()?;
            //        let status = resp.status();
            let value: serde_json::Value = resp.json()?;
            println!("{:?}", value.clone().to_string());
            let results: Paging<Account> = serde_json::from_value(value)?;
            Ok(results)
        } else {
            Err(FacebookError::Unauthorized)
        }
    }
}
