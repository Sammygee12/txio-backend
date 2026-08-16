use crate::model::recipe_template::RecipeTemplate;
use crate::utils::error::AppError;
use mongodb::bson::{doc, oid::ObjectId};
use mongodb::options::IndexOptions;
use mongodb::{Collection as MongoCollection, Database, IndexModel};

#[derive(Clone)]
pub struct RecipeTemplateRepository {
    collection: MongoCollection<RecipeTemplate>,
}

impl RecipeTemplateRepository {
    pub fn new(db: &Database) -> Self {
        let collection = db.collection("recipe_templates");
        Self { collection }
    }

    pub async fn ensure_indexes(&self) -> Result<(), AppError> {
        let user_id_index = IndexModel::builder()
            .keys(doc! { "user_id": 1 })
            .options(
                IndexOptions::builder()
                    .name(Some("recipe_templates_user_id_idx".to_string()))
                    .build(),
            )
            .build();

        self.collection
            .create_index(user_id_index, None)
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }

    pub async fn save(&self, template: &RecipeTemplate) -> Result<RecipeTemplate, AppError> {
        let result = self.collection.insert_one(template, None).await?;
        let mut created = template.clone();
        created.id = result.inserted_id.as_object_id();
        Ok(created)
    }

    pub async fn find_all_by_user(
        &self,
        user_id: ObjectId,
    ) -> Result<Vec<RecipeTemplate>, AppError> {
        let filter = doc! { "user_id": user_id };
        let mut cursor = self.collection.find(filter, None).await?;

        let mut templates = Vec::new();
        while cursor.advance().await? {
            let t: RecipeTemplate = cursor.deserialize_current().map_err(AppError::Database)?;
            templates.push(t);
        }

        Ok(templates)
    }

    pub async fn find_by_id(&self, id: ObjectId) -> Result<RecipeTemplate, AppError> {
        let filter = doc! { "_id": id };
        let result = self.collection.find_one(filter, None).await?;
        result.ok_or_else(|| AppError::NotFound(format!("Recipe template not found with id: {id}")))
    }

    pub async fn delete(&self, id: ObjectId) -> Result<(), AppError> {
        let filter = doc! { "_id": id };
        let result = self.collection.delete_one(filter, None).await?;

        if result.deleted_count == 0 {
            return Err(AppError::NotFound(format!(
                "Recipe template not found for deletion: {id}"
            )));
        }
        Ok(())
    }
}
