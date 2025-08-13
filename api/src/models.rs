use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Insertable, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::documents)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Document {
    pub id: String,
    pub title: String,
    pub body: String,
    pub thumbnail_url: String,
}
