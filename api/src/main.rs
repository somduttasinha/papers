use aws_config::BehaviorVersion;
use aws_sdk_s3::Client;
use aws_sdk_s3::config::Builder as S3ConfigBuilder;
use aws_sdk_s3::primitives::ByteStream;
use axum::body::Bytes;
use diesel::{Connection, PgConnection, RunQueryDsl, SelectableHelper};
use dotenvy::dotenv;
use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};
use std::collections::HashMap;
use std::env;
use std::ops::DerefMut;
use std::path::Path;
use std::sync::Arc;
use tantivy::directory::MmapDirectory;
use tantivy::query::QueryParser;
use tantivy::{Document, TantivyDocument};
use tempfile::NamedTempFile;
use tokio::process::Command;
use tokio::sync::Mutex;

use axum::extract::{Query, State, multipart};
use axum::routing::{get, post};
use axum::{Json, Router};
use tantivy::collector::{Count, TopDocs};
use tantivy::schema::{STORED, Schema, TEXT, Value};
use tantivy::{Index, IndexWriter, ReloadPolicy};
use tower_http::cors::Any;
use tower_http::cors::CorsLayer;

use crate::schema::documents;

mod models;
mod s3;
mod schema;
mod utils;

struct AppState {
    index: Index,
    schema: Schema,
    writer: Mutex<IndexWriter>,      // thread safe index writer
    connection: Mutex<PgConnection>, // thread safe db connection
    s3_client: Mutex<Client>,        // thread safe s3 client
}

#[tokio::main]
async fn main() -> tantivy::Result<()> {
    let connection = establish_connection(); // set up database

    let base = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let mut cfg = S3ConfigBuilder::from(&base);

    if let Ok(endpoint) = env::var("AWS_ENDPOINT_URL") {
        cfg = cfg.endpoint_url(endpoint);
    }

    if env::var("AWS_S3_FORCE_PATH_STYLE").as_deref() == Ok("true") {
        cfg = cfg.force_path_style(true);
    }

    let s3_client = Client::from_conf(cfg.build());

    s3::ensure_bucket(&s3_client, "papers-dev")
        .await
        .expect("Expected to create bucket");

    let index_path_raw = "tmp/index";
    let index_dir = Path::new(&index_path_raw);

    let index_dir = MmapDirectory::open(index_dir)?;

    let mut schema_builder = Schema::builder();

    schema_builder.add_text_field("title", TEXT | STORED);

    schema_builder.add_text_field("id", STORED);

    schema_builder.add_text_field("body", TEXT);

    let schema = schema_builder.build();

    let index = Index::open_or_create(index_dir, schema.clone())?;

    let mut index_writer: IndexWriter = index.writer(50_000_000)?;

    index_writer.commit()?;

    let state = Arc::new(AppState {
        index,
        schema,
        writer: Mutex::<tantivy::IndexWriter>::new(index_writer),
        connection: Mutex::<PgConnection>::new(connection),
        s3_client: Mutex::<Client>::new(s3_client),
    });

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
        .nest("/docs", get_documents_routes);

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
    let tmp = NamedTempFile::new().expect("Expected a tempfile"); // created on disk
    let path = tmp.path();

    println!("Path: {}", path.display());

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
            tokio::fs::write(&path, &data).await.unwrap();

            let contents = utils::pdf_to_string(path).await;

            match utils::export_pdf_to_jpegs(&path, None) {
                Ok(img_buf) => {
                    let s3_client = &mut state.s3_client.lock().await;

                    let mut buf = Vec::new();

                    PngEncoder::new(&mut buf)
                        .write_image(
                            img_buf.as_raw(),
                            img_buf.width(),
                            img_buf.height(),
                            ExtendedColorType::Rgb8,
                        )
                        .unwrap();

                    s3::upload_object(
                        &s3_client,
                        "papers-dev",
                        "application/pdf",
                        &format!("{}/document.pdf", id),
                        ByteStream::from_path(path)
                            .await
                            .expect("Failed to get bytes from path"),
                    )
                    .await
                    .expect("Failed to upload to s3");

                    s3::upload_object(
                        &s3_client,
                        "papers-dev",
                        "image/png",
                        &format!("{}/thumbnail.png", id),
                        ByteStream::from(buf),
                    )
                    .await
                    .expect("Failed to upload to s3");

                    let new_doc = crate::models::Document {
                        id: id.clone(),
                        title: filename.clone(),
                        body: contents.clone(),
                        thumbnail_url: String::from(""), // TODO: this will be a presigned-url
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
    let s3_client = state.s3_client.lock().await;
    let mut docs: Vec<crate::models::Document> = documents::table.load(conn.deref_mut()).unwrap();
    // for each document, get a presigned url

    for doc in docs.iter_mut() {
        let url = s3::get_object(
            &s3_client,
            "papers-dev",
            format!("{}/thumbnail.png", doc.id).as_ref(),
            60 * 60,
        )
        .await
        .expect("Expected URL");

        doc.thumbnail_url = url.clone();
        println!("URL: {}", url.clone());
    }
    return Json(docs);
}

fn establish_connection() -> PgConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}
