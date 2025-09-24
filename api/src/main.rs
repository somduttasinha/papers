use aws_config::BehaviorVersion;
use aws_sdk_s3::Client;
use aws_sdk_s3::config::Builder as S3ConfigBuilder;
use aws_sdk_s3::primitives::ByteStream;
use axum::body::{Body, Bytes};
use axum::extract::{Query, State, multipart};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{PgConnection, QueryDsl, RunQueryDsl, TextExpressionMethods};
use dotenvy::dotenv;
use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};
use std::collections::HashMap;
use std::env;
use std::ops::DerefMut;
use std::sync::Arc;
use tantivy::collector::{Count, TopDocs};
use tantivy::directory::MmapDirectory;
use tantivy::query::QueryParser;
use tantivy::schema::{Schema, Value, STORED, STRING, TEXT};
use tantivy::{Document, IndexReader, TantivyDocument, Term};
use tantivy::{Index, IndexWriter, ReloadPolicy};
use tempfile::NamedTempFile;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio_util::io::ReaderStream;
use tower_http::cors::Any;
use tower_http::cors::CorsLayer;

use crate::s3::S3Client;
use crate::schema::documents;

mod models;
mod s3;
mod schema;
mod utils;

type PgPool = Pool<ConnectionManager<PgConnection>>;

static INDEX_PATH_RAW: &str = "tmp/index";
struct AppState {
    index: Index,
    schema: Schema,
    writer: Mutex<IndexWriter>,
    reader: IndexReader,
    db_pool: PgPool,
    s3_client: Mutex<S3Client>,
}

#[tokio::main]
async fn main() -> tantivy::Result<()> {
    let pool = establish_connection(); // set up database

    let base = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let mut cfg = S3ConfigBuilder::from(&base);

    if let Ok(endpoint) = env::var("AWS_ENDPOINT_URL") {
        cfg = cfg.endpoint_url(endpoint);
    }

    if env::var("AWS_S3_FORCE_PATH_STYLE").as_deref() == Ok("true") {
        cfg = cfg.force_path_style(true);
    }

    let client = Client::from_conf(cfg.build());

    let s3_client = S3Client::new(client, "papers-dev".to_string());

    match s3_client.ensure_bucket().await {
        Ok(_) => println!("Bucket exists"),
        Err(e) => println!("Error with bucket existing: {}", e),
    }

    let index_dir = std::path::Path::new(INDEX_PATH_RAW);

    let index_dir = MmapDirectory::open(index_dir)?;

    let mut schema_builder = Schema::builder();

    schema_builder.add_text_field("title", TEXT | STORED);

    schema_builder.add_text_field("id", STRING | STORED);

    schema_builder.add_text_field("body", TEXT);

    let schema = schema_builder.build();

    let index = Index::open_or_create(index_dir, schema.clone())?;

    let mut index_writer: IndexWriter = index.writer(50_000_000)?;

    index_writer.commit()?;

    let reader = index.reader()?;

    let state = Arc::new(AppState {
        index,
        schema,
        writer: Mutex::<tantivy::IndexWriter>::new(index_writer),
        reader: reader,
        db_pool: pool,
        s3_client: Mutex::<S3Client>::new(s3_client),
    });

    let document_routes: Router<()> = Router::new()
        .route("/", get(get_all_docs))
        .route("/download/{id}", get(download_doc))
        .route("/preview/{id}", get(preview_doc))
        .route("/upload", post(save_and_upsert))
        .route("/delete/{id}", delete(delete_doc))
        .with_state(Arc::clone(&state));

    let search_routes: Router<()> = Router::new()
        .route("/", get(find_matches))
        .with_state(Arc::clone(&state));

    let api_routes = Router::new()
        .nest("/search", search_routes)
        .nest("/docs", document_routes);

    let app = Router::new().nest("/api", api_routes).layer(
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any),
    );

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();

    println!("Listening on port 8080");
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

async fn download_doc(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let s3_client = &mut state.s3_client.lock().await;
    let out = s3_client
        .get_object(format!("{}/document.pdf", id).as_ref())
        .await
        .expect("Expected URL");

    let content_type = out.content_type.unwrap_or("application/pdf".to_string());

    let content_length = out.content_length;

    let stream = out.body.into_async_read();
    let stream = ReaderStream::new(stream);

    let axum_body = Body::from_stream(stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(
            axum::http::header::CONTENT_DISPOSITION,
            HeaderValue::from_str("attachment; filename=\"document.pdf\"").unwrap(),
        )
        .header(axum::http::header::CONTENT_TYPE, content_type)
        .header(
            axum::http::header::CONTENT_LENGTH,
            content_length.expect("Expected content length").to_string(),
        )
        .body(axum_body)
        .unwrap()
}

async fn preview_doc(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let s3_client = &mut state.s3_client.lock().await;
    let out = s3_client
        .get_object(format!("{}/document.pdf", id).as_ref())
        .await
        .expect("Expected URL");

    let content_type = out.content_type.unwrap_or("application/pdf".to_string());

    let content_length = out.content_length;

    let stream = out.body.into_async_read();
    let stream = ReaderStream::new(stream);

    let axum_body = Body::from_stream(stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(
            axum::http::header::CONTENT_DISPOSITION,
            HeaderValue::from_static(r#"inline; filename="preview.pdf""#),
        )
        .header(axum::http::header::CONTENT_TYPE, content_type)
        .header(
            axum::http::header::CONTENT_LENGTH,
            content_length.expect("Expected content length").to_string(),
        )
        .body(axum_body)
        .unwrap()
}

/// We get the multipart form data as a stream of fields, to avoid overloading RAM for large files,
/// we will save to disk as we receive them and build the tantivy::document in memory, we only
/// commit once we have all the data for atomicity.
async fn save_and_upsert(State(state): State<Arc<AppState>>, mut multipart: multipart::Multipart) {
    let tmp = NamedTempFile::new().expect("Expected a tempfile"); // created on disk
    let path: &std::path::Path = tmp.path();

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

                    s3_client
                        .upload_object(
                            "application/pdf",
                            &format!("{}/document.pdf", id),
                            ByteStream::from_path(path)
                                .await
                                .expect("Failed to get bytes from path"),
                        )
                        .await
                        .expect("Failed to upload to s3");

                    s3_client
                        .upload_object(
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

                    let mut conn = state.db_pool.get().expect("Failed to get db connection");

                    diesel::insert_into(documents::table)
                        .values(&new_doc)
                        .execute(&mut conn)
                        .expect("Failed to insert into db");
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

    let mut index_writer = state.writer.lock().await;
    index_writer.add_document(doc).unwrap();
    index_writer.commit().unwrap();
}

async fn find_matches(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Vec<String>> {
    let index = &state.index;
    let schema = &state.schema;

    let reader = &state.reader;

    reader.reload().unwrap();

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

async fn delete_doc(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let s3_client = &mut state.s3_client.lock().await;
    match s3_client
        .delete_object(format!("{}/document.pdf", &id).as_ref())
        .await
    {
        Ok(_) => println!("Deleted document.pdf"),
        Err(e) => println!("Error deleting document.pdf: {}", e),
    }

    match s3_client
        .delete_object(format!("{}/thumbnail.png", &id).as_ref())
        .await
    {
        Ok(_) => println!("Deleted thumbnail.png"),
        Err(e) => println!("Error deleting thumbnail.png: {}", e),
    }

    let mut conn = state.db_pool.get().expect("Failed to get db connection");

    diesel::delete(documents::table.filter(documents::id.like(&id)))
        .execute(&mut conn)
        .expect("Failed to delete from db");

    let schema = &state.schema;

    let id_field = schema.get_field("id").expect("Expected an id field");

    let mut index_writer = state.writer.lock().await;

    let term = Term::from_field_text(id_field, &id);

    index_writer.delete_term(term);

    index_writer
        .commit()
        .expect("Failed to commit index deletion");

    let reader = &state.reader;
    reader.reload().unwrap();

    (StatusCode::OK, "Deleted document")
}

async fn get_all_docs(State(state): State<Arc<AppState>>) -> Json<Vec<crate::models::Document>> {
    let mut conn = state.db_pool.get().expect("Failed to get db connection");
    let s3_client = state.s3_client.lock().await;
    let mut docs: Vec<crate::models::Document> = documents::table.load(conn.deref_mut()).unwrap();

    for doc in docs.iter_mut() {
        let url = s3_client
            .get_object_url(format!("{}/thumbnail.png", doc.id).as_ref(), 60 * 60)
            .await
            .expect("Expected URL");

        doc.thumbnail_url = url.clone();
        println!("URL: {}", url.clone());
    }
    return Json(docs);
}

fn establish_connection() -> PgPool {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let pool = PgPool::builder()
        .build(manager)
        .expect("Failed to create pool");
    pool
}
