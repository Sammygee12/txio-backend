use crate::model::workspace::Workspace;
use crate::utils::error::AppError;
use mongodb::bson::{doc, oid::ObjectId};
use mongodb::options::IndexOptions;
use mongodb::{Collection as MongoCollection, Database, IndexModel};

#[derive(Clone)]
pub struct WorkspaceRepository {
    collection: MongoCollection<Workspace>,
}

impl WorkspaceRepository {
    pub fn new(db: &Database) -> Self {
        let collection = db.collection("workspaces");
        Self { collection }
    }

    pub async fn ensure_indexes(&self) -> Result<(), AppError> {
        let user_id_index = IndexModel::builder()
            .keys(doc! { "user_id": 1 })
            .options(
                IndexOptions::builder()
                    .name(Some("workspaces_user_id_idx".to_string()))
                    .build(),
            )
            .build();

        self.collection
            .create_index(user_id_index, None)
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }

    pub async fn save(&self, new_workspace: &Workspace) -> Result<Workspace, AppError> {
        let result = self.collection.insert_one(new_workspace, None).await?;
        let mut created = new_workspace.clone();
        created.id = result.inserted_id.as_object_id();
        Ok(created)
    }

    pub async fn find_all_by_user(&self, user_id: ObjectId) -> Result<Vec<Workspace>, AppError> {
        let mut cursor = self
            .collection
            .find(doc! { "user_id": user_id }, None)
            .await?;

        let mut workspaces = Vec::new();
        while cursor.advance().await? {
            let workspace: Workspace = cursor.deserialize_current().map_err(AppError::Database)?;
            workspaces.push(workspace);
        }

        workspaces.sort_by_key(|left| left.created_at);

        Ok(workspaces)
    }

    pub async fn find_by_id(&self, id: ObjectId) -> Result<Workspace, AppError> {
        let result = self.collection.find_one(doc! { "_id": id }, None).await?;

        result.ok_or_else(|| AppError::NotFound(format!("Workspace not found with id: {id}")))
    }
}
