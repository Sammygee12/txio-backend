use crate::model::rpc::RpcLog;
use crate::utils::error::AppError;
use mongodb::bson::doc;
use mongodb::options::IndexOptions;
use mongodb::{Collection, Database, IndexModel};

#[derive(Clone)]
pub struct RpcRepository {
    collection: Collection<RpcLog>,
}

impl RpcRepository {
    pub fn new(db: &Database) -> Self {
        let collection = db.collection("rpc_logs");
        Self { collection }
    }

    pub async fn ensure_indexes(&self) -> Result<(), AppError> {
        let user_id_index = IndexModel::builder()
            .keys(doc! { "user_id": 1 })
            .options(
                IndexOptions::builder()
                    .name(Some("rpc_logs_user_id_idx".to_string()))
                    .build(),
            )
            .build();

        self.collection
            .create_index(user_id_index, None)
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }

    pub async fn save(&self, log: &RpcLog) -> Result<(), AppError> {
        self.collection.insert_one(log, None).await?;
        Ok(())
    }

    pub async fn find_by_user_id(
        &self,
        user_id: mongodb::bson::oid::ObjectId,
        limit: i64,
    ) -> Result<Vec<RpcLog>, AppError> {
        use mongodb::bson::doc;
        use mongodb::options::FindOptions;

        let filter = doc! { "user_id": user_id };
        let opts = FindOptions::builder()
            .sort(doc! { "_id": -1 })
            .limit(Some(limit))
            .build();

        let mut cursor = self.collection.find(filter, Some(opts)).await?;

        let mut logs = Vec::new();
        while cursor.advance().await? {
            let log = cursor.deserialize_current()?;
            logs.push(log);
        }

        Ok(logs)
    }

    pub async fn count_all(&self) -> Result<u64, AppError> {
        let count = self.collection.count_documents(None, None).await?;
        Ok(count)
    }

    pub async fn find_recent(&self, limit: i64) -> Result<Vec<RpcLog>, AppError> {
        use mongodb::bson::doc;
        use mongodb::options::FindOptions;

        let opts = FindOptions::builder()
            .sort(doc! { "_id": -1 })
            .limit(Some(limit))
            .build();

        let mut cursor = self.collection.find(None, Some(opts)).await?;
        let mut logs = Vec::new();
        while cursor.advance().await? {
            let log = cursor.deserialize_current()?;
            logs.push(log);
        }

        Ok(logs)
    }
}
