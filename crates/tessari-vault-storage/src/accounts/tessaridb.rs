//! Accounts on TessariDB: the `accounts` table.

use async_trait::async_trait;
use tessari_vault_infrastructure::tessaridb::{Database, Place, Value};
use tessari_vault_types::{AccountId, Email};

use super::AccountEntity;
use super::repository::AccountRepository;
use crate::access::bind;
use crate::{Result, answers};

pub(crate) struct TessaridbAccountRepository {
    database: Database,
}

impl TessaridbAccountRepository {
    pub(crate) const fn new(database: Database) -> Self {
        Self { database }
    }
}

#[async_trait]
impl AccountRepository for TessaridbAccountRepository {
    async fn get(&self, place: &Place, id: &AccountId) -> Result<Option<AccountEntity>> {
        let answers = self
            .database
            .read_at(
                place,
                "SELECT * FROM accounts:$id;",
                vec![bind("id", id.as_str())],
            )
            .await?;
        answers::single(answers, "account")
    }

    async fn find_by_email(&self, place: &Place, email: &Email) -> Result<Option<AccountEntity>> {
        let answers = self
            .database
            .read_at(
                place,
                "SELECT * FROM accounts WHERE email = $email LIMIT 1 USING INDEX by_email;",
                vec![bind("email", email.as_str())],
            )
            .await?;
        answers::single(answers, "account")
    }

    async fn get_many(&self, place: &Place, ids: &[AccountId]) -> Result<Vec<AccountEntity>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids = Value::Array(ids.iter().map(|id| Value::from(id.as_str())).collect());
        let answers = self
            .database
            .read_at(
                place,
                "SELECT * FROM accounts WHERE account_id IN $ids;",
                vec![("ids".to_owned(), ids)],
            )
            .await?;
        answers::records(answers, "account")
    }

    async fn create_admin(
        &self,
        place: &Place,
        id: &AccountId,
        email: &Email,
        name: &str,
    ) -> Result<()> {
        self.database
            .run_at(
                place,
                "CREATE accounts:$id = { account_id: $id, email: $email, name: $name, state: 'active', \
                 platform_admin: true, created: time::now() };",
                vec![bind("id", id.as_str()), bind("email", email.as_str()), bind("name", name)],
            )
            .await?;
        Ok(())
    }

    async fn make_admin(&self, place: &Place, id: &AccountId) -> Result<()> {
        self.database
            .run_at(
                place,
                "UPDATE accounts:$id SET platform_admin = true;",
                vec![bind("id", id.as_str())],
            )
            .await?;
        Ok(())
    }
}
