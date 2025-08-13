use axum::body::Bytes;
use diesel::{Connection, PgConnection, RunQueryDsl, SelectableHelper};
use dotenvy::dotenv;
use std::collections::HashMap;
use std::env;
use std::io::stdout;
use std::ops::DerefMut;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tantivy::directory::MmapDirectory;
use tantivy::query::QueryParser;
use tantivy::{Document, TantivyDocument};
use tokio::process::Command;
use tokio::sync::Mutex;

use axum::extract::{Query, State, multipart};
use axum::routing::{get, get_service, post};
use axum::{Json, Router};
use tantivy::collector::{Count, TopDocs};
use tantivy::schema::{STORED, Schema, TEXT, Value};
use tantivy::{Index, IndexWriter, ReloadPolicy};
use tower_http::cors::Any;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use crate::schema::documents;

mod models;
mod schema;
mod utils;

struct AppState {
    index: Index,
    schema: Schema,
    writer: Mutex<IndexWriter>,
    connection: Mutex<PgConnection>,
}

#[tokio::main]
async fn main() -> tantivy::Result<()> {
    let connection = establish_connection();

    let index_path_raw = "tmp/index";
    let index_dir = Path::new(&index_path_raw);

    let index_dir = MmapDirectory::open(index_dir)?;

    let mut schema_builder = Schema::builder();

    let _ = schema_builder.add_text_field("title", TEXT | STORED);

    let _ = schema_builder.add_text_field("id", STORED);

    let _ = schema_builder.add_text_field("body", TEXT);

    let schema = schema_builder.build();

    let index = Index::open_or_create(index_dir, schema.clone())?;

    let mut index_writer: IndexWriter = index.writer(50_000_000)?;

    index_writer.commit()?;

    let state = Arc::new(AppState {
        index,
        schema,
        writer: Mutex::<tantivy::IndexWriter>::new(index_writer),
        connection: Mutex::<PgConnection>::new(connection),
    });

    let thumbnails_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tmp")
        .join("thumbnails");

    println!("Thumbnails path: {}", thumbnails_path.display());

    let static_routes: Router<()> =
        Router::new().nest_service("/thumbnails", get_service(ServeDir::new(thumbnails_path)));

    let get_documents_routes: Router<()> = Router::new()
        .route("/", get(get_all_docs))
        .with_state(Arc::clone(&state));

    let upload_routes: Router<()> = Router::new()
        .route("/doc", post(save_and_upsert))
        .with_state(Arc::clone(&state));

    let search_routes: Router<()> = Router::new()
        .route("/", get(find_matches))
        .with_state(Arc::clone(&state));

    let api_routes = Router::new()
        .nest("/upload", upload_routes)
        .nest("/search", search_routes)
        .nest("/docs", get_documents_routes)
        .nest("/static", static_routes);

    let app = Router::new().nest("/api", api_routes).layer(
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any),
    );

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();

    axum::serve(listener, app).await.unwrap();

    Ok(())
}

/// We get the multipart form data as a stream of fields, to avoid overloading RAM for large files,
/// we will save to disk as we receive them and build the tantivy::document in memory, we only
/// commit once we have all the data for atomicity.
async fn save_and_upsert(State(state): State<Arc<AppState>>, mut multipart: multipart::Multipart) {
    let mut doc = TantivyDocument::new();
    let schema = &state.schema;

    let title_field = schema.get_field("title").expect("Expected a title field");
    let id_field = schema.get_field("id").expect("Expected an id field");
    let body_field = schema.get_field("body").expect("Expected a body field");

    let mut set_field = false;

    let id = uuid::Uuid::new_v4().to_string();
    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap().to_string();
        let filename = field.file_name().unwrap().to_string();

        if !set_field {
            doc.add_text(title_field, &filename);
            doc.add_text(id_field, id.clone());
            set_field = true;
        }

        let data: Bytes = field.bytes().await.unwrap();

        println!("Length of `{}` is {} bytes", name, data.len());
        if name == "file" {
            let path = format!("tmp/docs/{}", filename);
            // get absolute path
            tokio::fs::write(&path, &data).await.unwrap();

            let contents =
                utils::pdf_to_string(&Path::new(&format!("tmp/docs/{}", filename))).await;

            match utils::export_pdf_to_jpegs(
                &filename.strip_suffix(".pdf").unwrap().to_string(),
                &path,
                None,
            ) {
                Ok(image_path) => {
                    let new_doc = crate::models::Document {
                        id: id.clone(),
                        title: filename,
                        body: contents.clone(),
                        thumbnail_url: image_path,
                    };

                    let conn = &mut state.connection.lock().await;

                    diesel::insert_into(documents::table)
                        .values(&new_doc)
                        .returning(crate::models::Document::as_returning())
                        .get_result(conn.deref_mut())
                        .expect("Error saving new doc");
                }
                Err(e) => println!("Failed to export to jpegs: {}", e),
            }

            println!(
                "Managed to get the contents: {}",
                contents
                    .split_whitespace()
                    .take(10)
                    .collect::<Vec<&str>>()
                    .join(" ")
            );

            doc.add_text(body_field, contents);
        }
    }

    let mut index_writer = state.writer.lock().await; // by using await we know that we aren't
    // blocking the runtime
    index_writer.add_document(doc).unwrap();
    index_writer.commit().unwrap();
}

async fn find_matches(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Vec<String>> {
    let index = &state.index;
    let schema = &state.schema;

    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommitWithDelay)
        .try_into()
        .unwrap();

    let searcher = reader.searcher();

    let title = schema.get_field("title").expect("Expected a title field");
    let body = schema.get_field("body").expect("Expected a body field");

    let query_term = params.get("query").unwrap();

    println!("Query term: {}", query_term);

    // let query = utils::simple_fuzzy_query(title, body, &query_term).unwrap();
    let mut query_parser = QueryParser::for_index(&index, vec![title, body]);

    query_parser.set_conjunction_by_default();

    let query = query_parser.parse_query(&query_term).unwrap();

    let (top_docs, _) = searcher
        .search(&query, &(TopDocs::with_limit(5), Count))
        .unwrap();

    for (score, doc_address) in &top_docs {
        let retrieved_doc: TantivyDocument = searcher.doc(doc_address.to_owned()).unwrap();
        println!("score {score:?} doc {}", retrieved_doc.to_json(&schema));
    }

    // use functional programming to return a vector of the document titles

    let top_docs = top_docs
        .iter()
        .map(|(_score, doc_address)| searcher.doc(*doc_address).unwrap())
        .collect::<Vec<TantivyDocument>>();

    return Json(
        top_docs
            .iter()
            .filter_map(|doc| match doc.get_first(title) {
                Some(title) => Some(title.as_str().expect("Expected a title").to_string()),
                None => None,
            })
            .collect(),
    );
}

async fn get_all_docs(State(state): State<Arc<AppState>>) -> Json<Vec<crate::models::Document>> {
    let conn = &mut state.connection.lock().await;
    let docs = documents::table.load(conn.deref_mut()).unwrap();
    return Json(docs);
}

fn establish_connection() -> PgConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}
