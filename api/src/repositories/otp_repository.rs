use crate::model::otp::OTP;
use crate::utils::error::AppError;
use chrono::Duration as ChronoDuration;
use mongodb::bson::doc;
use mongodb::options::{FindOneAndReplaceOptions, IndexOptions};
use mongodb::{Collection, Database, IndexModel};
use std::time::Duration as StdDuration;

/// MongoDB TTL for OTP documents — purges rows even if the app never verifies them.
const OTP_TTL_SECONDS: u64 = 600;

#[derive(Clone)]
pub struct OTPRepository {
    collection: Collection<OTP>,
}

impl OTPRepository {
    pub fn new(db: &Database) -> Self {
        let collection = db.collection("otps");
        Self { collection }
    }

    pub async fn ensure_indexes(&self) -> Result<(), AppError> {
        let ttl_index = IndexModel::builder()
            .keys(doc! { "created_at": 1 })
            .options(
                IndexOptions::builder()
                    .name(Some("otps_created_at_ttl".to_string()))
                    .expire_after(StdDuration::from_secs(OTP_TTL_SECONDS))
                    .build(),
            )
            .build();

        let unique_email_index = IndexModel::builder()
            .keys(doc! { "email": 1 })
            .options(
                IndexOptions::builder()
                    .name(Some("otps_email_unique".to_string()))
                    .unique(true)
                    .build(),
            )
            .build();

        self.deduplicate_by_email().await?;

        self.collection.create_index(ttl_index, None).await?;
        self.collection
            .create_index(unique_email_index, None)
            .await?;
        Ok(())
    }

    async fn deduplicate_by_email(&self) -> Result<(), AppError> {
        let pipeline = vec![
            doc! {
                "$group": {
                    "_id": "$email",
                    "count": { "$sum": 1 },
                    "ids": { "$push": "$_id" }
                }
            },
            doc! {
                "$match": {
                    "count": { "$gt": 1 }
                }
            },
        ];

        let mut cursor = self.collection.aggregate(pipeline, None).await?;
        while cursor.advance().await? {
            let doc = cursor.deserialize_current().map_err(AppError::Database)?;
            if let Ok(ids) = doc.get_array("ids") {
                let delete_ids: Vec<_> = ids.iter().skip(1).cloned().collect();
                if !delete_ids.is_empty() {
                    self.collection
                        .delete_many(doc! { "_id": { "$in": delete_ids } }, None)
                        .await?;
                }
            }
        }

        Ok(())
    }

    pub async fn save(&self, otp: &OTP) -> Result<OTP, AppError> {
        let result = self.collection.insert_one(otp, None).await?;

        let mut otp_with_id = otp.clone();
        if let Some(inserted_id) = result.inserted_id.as_object_id() {
            otp_with_id.id = Some(inserted_id);
        }

        Ok(otp_with_id)
    }

    pub async fn upsert_otp(&self, otp: &OTP, cooldown_seconds: i64) -> Result<OTP, AppError> {
        let cooldown_cutoff = otp.created_at - ChronoDuration::seconds(cooldown_seconds);

        let filter = doc! {
            "email": &otp.email,
            "created_at": { "$lte": mongodb::bson::DateTime::from_millis(cooldown_cutoff.timestamp_millis()) }
        };

        let options = FindOneAndReplaceOptions::builder().upsert(true).build();

        match self
            .collection
            .find_one_and_replace(filter, otp, options)
            .await
        {
            Ok(_) => Ok(otp.clone()),
            Err(e) => {
                if is_duplicate_key_error(&e) {
                    Err(AppError::BadRequest(
                        "OTP request rate limit exceeded. Please try again later.".to_string(),
                    ))
                } else {
                    Err(AppError::Database(e))
                }
            }
        }
    }

    pub async fn find_by_email(&self, email: &str) -> Result<OTP, AppError> {
        let otp = self
            .collection
            .find_one(doc! { "email": email }, None)
            .await?
            .ok_or(AppError::NotFound("OTP not found for email".to_string()))?;

        Ok(otp)
    }

    pub async fn update_failed_attempts(
        &self,
        email: &str,
        failed_attempts: i32,
    ) -> Result<(), AppError> {
        self.collection
            .update_one(
                doc! { "email": email },
                doc! { "$set": { "failed_attempts": failed_attempts } },
                None,
            )
            .await?;
        Ok(())
    }

    pub async fn delete_by_email(&self, email: &str) -> Result<(), AppError> {
        self.collection
            .delete_many(doc! { "email": email }, None)
            .await?;
        Ok(())
    }
}

fn is_duplicate_key_error(err: &mongodb::error::Error) -> bool {
    if let mongodb::error::ErrorKind::Command(ref cmd_err) = *err.kind {
        if cmd_err.code == 11000 || cmd_err.code == 11001 || cmd_err.code == 12582 {
            return true;
        }
    }
    if let mongodb::error::ErrorKind::Write(mongodb::error::WriteFailure::WriteError(
        ref write_err,
    )) = *err.kind
    {
        if write_err.code == 11000 || write_err.code == 11001 || write_err.code == 12582 {
            return true;
        }
    }
    let err_str = err.to_string();
    err_str.contains("E11000")
        || err_str.contains("duplicate key")
        || err_str.contains("DuplicateKey")
}
