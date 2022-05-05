#[macro_use]
extern crate diesel;

mod schema;
mod pagination;

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
use chrono::{Local, NaiveDateTime, NaiveDate};
use tokio::time::sleep;
use tokio::task;
use diesel::sql_types::BigInt;


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
        .route("/users", get(get_users_by_page))
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

#[derive(QueryableByName, Default)]
struct MyQuery {
    #[sql_type = "BigInt"] result: i64,
}

async fn play_with_raw_query(conn: DbConn) -> String {
    let r = sql_query("select count(1) as result;").get_result::<MyQuery>(&conn.0).unwrap().result;
    println!("{}", r);
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

async fn get_users_by_page(Query(params): Query<HashMap<String, String>>, conn: DbConn) -> Json<MyResponse<(Vec<User>, i64, i64)>> {
    let page_params = Params {
        page: Some(params.get("page").unwrap().parse::<i64>().unwrap()),
        page_size: Some(params.get("page_size").unwrap().parse::<i64>().unwrap()),
    };


    println!("page: {:?}, page_size: {:?}", &page_params.page, &page_params.page_size);

    let r = paginate_users(&page_params, &conn).unwrap();

    let resp = MyResponse {
        r: true,
        d: Some(r),
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
        created_at: Some(Local::now().naive_local()),
    };

    let created_user = db_create_user(&conn, &in_user);

    let out_user = ReqUser {
        id: Some(created_user.id),
        username: Some(created_user.username),
        created_at: Some(created_user.created_at.unwrap().to_string()),
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
    created_at: Option<String>,
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
    created_at: Option<NaiveDateTime>,
}

fn all_users(conn: &PgConnection) -> Vec<User> {
    use crate::schema::users::dsl::*;
    users.load::<User>(conn).unwrap()
}

#[derive(Debug, Deserialize)]
pub struct Params {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}


fn paginate_users(params: &Params, conn: &PgConnection) -> anyhow::Result<(Vec<User>, i64, i64)> {
    use crate::pagination::LoadPaginated;
    use crate::diesel::QueryDsl;
    use diesel::prelude::*;

    let mut _query = users::table.into_boxed();

    // let (_users, _total_pages, _total) = (_query
    //     .filter(users::created_at.ge(NaiveDate::from_ymd(2016, 7, 8).and_hms(9, 10, 11)))
    //     .order(users::created_at.desc())
    //     .load(conn)?, 0, 0);

    let (_users, _total_pages, _total) = _query
        .order(users::created_at.desc())
        .filter(users::created_at.ge(NaiveDate::from_ymd(2016, 7, 8).and_hms(9, 10, 11)))
        .load_with_pagination(&conn, params.page, params.page_size)?;

    Ok((_users, _total_pages, _total))
}

