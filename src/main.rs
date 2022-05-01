#[macro_use]
extern crate diesel;

mod schema;

use std::collections::HashMap;
use crate::schema::{users};
use std::env;
use axum::{routing::{get, post}, http::StatusCode, response::IntoResponse, Json, Router, async_trait};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;
use axum::extract::{Extension, FromRequest, Query, RequestParts};
use axum::http::header::HOST;
use diesel::{insert_into, PgConnection, RunQueryDsl, sql_query};
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use dotenv::dotenv;
use uuid::Uuid;
use axum::extract::{Path};
use diesel::sql_types::Integer;
use tokio::time::sleep;
use tokio::task;

pub type PgPool = Pool<PgConnMgr>;
pub type PgConnMgr = ConnectionManager<PgConnection>;
pub type PooledPgConn = PooledConnection<PgConnMgr>;

pub struct DbConn(pub PooledPgConn);

pub fn pool(db_url: &String) -> PgPool {
    let manager = PgConnMgr::new(db_url);
    let max_size = env::var("POOL_SIZE")
        .expect("POOL_SIZE must be set")
        .parse::<u32>()
        .unwrap();

    Pool::builder()
        .max_size(max_size)
        .build(manager)
        .expect("DB Connection Pool Build Error!")
}


pub fn get_conn(pool: &PgPool) -> PooledPgConn {
    match pool.get() {
        Ok(conn) => conn,
        Err(_) => panic!("DB Connection Error!"),
    }
}

pub struct DbState {
    pool: PgPool,
}


pub fn uuid() -> String {
    Uuid::new_v4().to_string()
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MyResponse<T> {
    pub r: bool,
    // result
    pub d: Option<T>,
    // data
    pub e: Option<String>, // err
}


fn internal_error<E>(err: E) -> (StatusCode, String)
    where
        E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}


#[derive(Debug, Serialize, Deserialize)]
pub struct HostHeader(pub String);

impl Deref for HostHeader {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[async_trait]
impl<B> FromRequest<B> for HostHeader where B: Send, {
    type Rejection = (StatusCode, String);

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let headers = req.headers();
        Ok(Self(headers[HOST].to_str().map_err(internal_error)?.to_string()))
    }
}


#[async_trait]
impl<B> FromRequest<B> for DbConn
    where B: Send, {
    type Rejection = (StatusCode, String);

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let Extension(db_pool) = Extension::<Arc<DbState>>::from_request(req).await
            .map_err(internal_error)?;

        let conn = db_pool.pool.get().map_err(internal_error)?;

        Ok(Self(conn))
    }
}

impl Deref for DbConn {
    type Target = PgConnection;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    // todo: replace with this logger later
    // initialize tracing
    // tracing_subscriber::fmt::init();

    let db_state = DbState {
        pool: pool(&env::var("DATABASE_URL").unwrap()),
    };

    let shared_db_state = Arc::new(db_state);

    // build our application with a route
    let app = Router::new()
        // `GET /` goes to `root`
        .route("/", get(root))
        // `POST /users` goes to `create_user`
        .route("/users", post(create_user))
        .route("/get_host", get(get_host))
        .route("/my_resp", get(my_resp))
        .route("/path/:id", get(path))
        .route("/path2/:path_id", get(path2))
        .route("/post_with_path/:id", post(post_with_path))
        .route("/raw_string_post", post(raw_string_post))
        .route("/mix/:id", post(mix))
        .route("/query", get(query))
        .route("/nested_async", get(nested_async))
        .route("/play_with_raw_query", get(play_with_raw_query))
        .layer(Extension(shared_db_state));

    // run our app with hyper
    // `axum::Server` is a re-export of `hyper::Server`
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    // tracing::debug!("listening on {}", addr);
    log::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn nested_async() -> String {
    task::spawn(async { inner_async().await; });

    "OUTER".to_string()
}

async fn play_with_raw_query(conn: DbConn) -> String {
    print!("{:?}", sql_query("select 1").load::<Integer>(&conn.0).unwrap());
    "OK".to_string()
}

async fn inner_async() {
    sleep(Duration::from_millis(2000)).await;
    println!("INNER");
}

async fn path(Path(id): Path<String>) -> String {
    if id.is_empty() {
        "<NONE>".to_string()
    } else {
        id
    }
}

async fn path2(Path(path_id): Path<String>) -> String {
    if path_id.is_empty() {
        "<NONE>".to_string()
    } else {
        path_id
    }
}

async fn post_with_path(Path(path_id): Path<String>) -> String {
    path_id
}

async fn raw_string_post(data: String) -> String {
    data
}

// basic handler that responds with a static string
async fn root() -> &'static str {
    "Hello, World!"
}

async fn get_host(host: HostHeader) -> String {
    host.clone()
}

async fn sql() -> String {
    "SQL".to_string()
}

async fn my_resp() -> Json<MyResponse<String>> {
    let resp = MyResponse {
        r: true,
        d: Some("payload".to_string()),
        e: Some("error payload".to_string()),
    };

    Json(resp)
}


async fn mix(Path(id): Path<String>,
             host: HostHeader,
             conn: DbConn,
             Json(payload): Json<ReqUser>) -> Json<MyResponse<String>> {
    let resp_str = format!("{:?} / {:?} / {:?} / {:?}", id, host, payload, all_users(&conn));

    let resp = MyResponse {
        r: true,
        d: Some(resp_str),
        e: None,
    };

    Json(resp)
}


async fn query(Query(params): Query<HashMap<String, String>>) -> Json<MyResponse<String>> {
    let resp_str = format!("{:?}", params);
    let resp = MyResponse {
        r: true,
        d: Some(resp_str),
        e: None,
    };

    Json(resp)
}

fn db_create_user(conn: &PgConnection, in_user: &User) -> User {
    use crate::schema::users::dsl::*;

    insert_into(users)
        .values(in_user.clone())
        .get_result::<User>(conn)
        .unwrap()
}

async fn create_user(
    conn: DbConn,
    Json(payload): Json<ReqUser>,
) -> impl IntoResponse {

    // insert your application logic here
    let in_user = User {
        id: uuid(),
        username: payload.username.unwrap(),
    };

    let created_user = db_create_user(&conn, &in_user);

    let out_user = ReqUser {
        id: Some(created_user.id),
        username: Some(created_user.username),
    };

    (StatusCode::CREATED, Json(out_user))
}

#[derive(
Debug,
Serialize,
Deserialize)]
struct ReqUser {
    id: Option<String>,
    username: Option<String>,
}

#[derive(
Identifiable,
PartialEq,
Serialize,
Deserialize,
Queryable,
Insertable,
Debug,
Clone,
AsChangeset,
)]
#[table_name = "users"]
struct User {
    id: String,
    username: String,
}

fn all_users(conn: &PgConnection) -> Vec<User> {
    use crate::schema::users::dsl::*;
    users.load::<User>(conn).unwrap()
}

