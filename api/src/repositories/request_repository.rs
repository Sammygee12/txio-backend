use crate::model::request::SavedRequest;
use crate::utils::error::AppError;
use mongodb::bson::{doc, oid::ObjectId};
use mongodb::options::IndexOptions;
use mongodb::{Collection, Database, IndexModel};

#[derive(Clone)]
pub struct RequestRepository {
    collection: Collection<SavedRequest>,
}

impl RequestRepository {
    pub fn new(db: &Database) -> Self {
        let collection = db.collection("saved_requests");
        Self { collection }
    }

    pub async fn ensure_indexes(&self) -> Result<(), AppError> {
        let collection_id_index = IndexModel::builder()
            .keys(doc! { "collection_id": 1 })
            .options(
                IndexOptions::builder()
                    .name(Some("saved_requests_collection_id_idx".to_string()))
                    .build(),
            )
            .build();

        self.collection
            .create_index(collection_id_index, None)
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }

    pub async fn save(&self, request: &SavedRequest) -> Result<SavedRequest, AppError> {
        let result = self.collection.insert_one(request, None).await?;
        let mut created = request.clone();
        created.id = result.inserted_id.as_object_id();
        Ok(created)
    }

    pub async fn find_all_by_collection(
        &self,
        collection_id: ObjectId,
    ) -> Result<Vec<SavedRequest>, AppError> {
        let filter = doc! { "collection_id": collection_id };
        let mut cursor = self.collection.find(filter, None).await?;

        let mut requests = Vec::new();
        while cursor.advance().await? {
            let req: SavedRequest = cursor.deserialize_current().map_err(AppError::Database)?;
            requests.push(req);
        }

        Ok(requests)
    }

    pub async fn find_by_id(&self, id: ObjectId) -> Result<SavedRequest, AppError> {
        let filter = doc! { "_id": id };
        let result = self.collection.find_one(filter, None).await?;
        result.ok_or_else(|| AppError::NotFound(format!("Request not found with id: {id}")))
    }

    pub async fn update(&self, request: &SavedRequest) -> Result<SavedRequest, AppError> {
        let id = request.id.ok_or(AppError::InternalError(
            "Cannot update request without ID".into(),
        ))?;
        let filter = doc! { "_id": id };

        self.collection.replace_one(filter, request, None).await?;
        Ok(request.clone())
    }

    pub async fn delete(&self, id: ObjectId) -> Result<(), AppError> {
        let filter = doc! { "_id": id };
        let result = self.collection.delete_one(filter, None).await?;

        if result.deleted_count == 0 {
            return Err(AppError::NotFound(format!(
                "Request not found for deletion: {id}"
            )));
        }
        Ok(())
    }

    pub async fn delete_all_by_collection(&self, collection_id: ObjectId) -> Result<(), AppError> {
        let filter = doc! { "collection_id": collection_id };
        self.collection.delete_many(filter, None).await?;
        Ok(())
    }
}
