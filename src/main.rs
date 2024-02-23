#[macro_use]
extern crate diesel;

mod schema;
mod pagination;

use std::collections::HashMap;
use crate::schema::{users};
use std::{env, fmt};
use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::io::Write;
use axum::{routing::{get, post}, http::StatusCode, response::IntoResponse, Json, Router, async_trait};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::Duration;
use anyhow::anyhow;
use axum::extract::{FromRef, FromRequestParts, Query, State};
use axum::http::header::HOST;
use diesel::{insert_into, PgConnection, QueryDsl, RunQueryDsl, sql_query, ExpressionMethods};
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::sql_types::Jsonb;
use dotenv::dotenv;
use uuid::Uuid;
use axum::extract::{Path};
use axum::http::request::Parts;
use chrono::{Local, NaiveDateTime};
use diesel::pg::{Pg, PgValue};
use diesel::serialize::IsNull;
use tokio::time::sleep;
use tokio::task;
use diesel::sql_types::BigInt;
use log::{debug, error};
use serde::de::DeserializeOwned;
use tokio::net::TcpListener;


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
impl<B> FromRequestParts<B> for HostHeader where
    B: Send + Sync, {
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, _state: &B) -> Result<Self, Self::Rejection> {
        let headers = parts.headers.clone();
        Ok(Self(headers[HOST].to_str().map_err(internal_error)?.to_string()))
    }
}


type MyDbState = Arc<DbState>;

#[async_trait]
impl<B> FromRequestParts<B> for DbConn
    where
        MyDbState: FromRef<B>,
        B: Send + Sync, {
    type Rejection = (StatusCode, String);

    async fn from_request_parts(_parts: &mut Parts, state: &B) -> Result<Self, Self::Rejection> {
        // let Extension(db_pool) = Extension::<Arc<DbState>>::from_request_parts(req).await
        //     .map_err(internal_error)?;
        let db_pool = MyDbState::from_ref(state);

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


// https://github.com/tokio-rs/axum/discussions/930
// https://docs.rs/axum/latest/axum/extract/index.html#applying-multiple-extractors
// https://docs.rs/axum/latest/axum/struct.Extension.html
async fn req_conn(State(db_pool): State<Arc<DbState>>) -> String {
    let _conn = db_pool.pool.get().unwrap();
    "OK".to_string()
}

async fn nested_async() -> String {
    task::spawn(async { inner_async().await; });

    "OUTER".to_string()
}

#[derive(QueryableByName, Default)]
struct MyQuery {
    #[diesel(sql_type = BigInt)]
    result: i64,
}

pub fn extract_conn(conn: DbConn) -> PooledPgConn {
    conn.0
}

async fn play_with_raw_query(conn: DbConn) -> String {
    let r = sql_query("select count(1) as result;").get_result::<MyQuery>(extract_conn(conn).deref_mut()).unwrap().result;
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


async fn get_current_time() -> Json<NaiveDateTime> {
    Json(chrono::offset::Local::now().naive_local())
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
    let resp_str = format!("{:?} / {:?} / {:?} / {:?}", id, host, payload, all_users(extract_conn(conn).deref_mut()));

    let resp = MyResponse {
        r: true,
        d: Some(resp_str),
        e: None,
    };

    Json(resp)
}

async fn find_user_by_id(conn: DbConn, Path(id): Path<String>) -> Json<MyResponse<TypedUser<ReqWxMessageData4KeywordTemplate>>> {
    let typed_user = TypedUser::<ReqWxMessageData4KeywordTemplate>::find_typed_user_by_id(&id, &mut extract_conn(conn)).unwrap();
    let resp = MyResponse {
        r: true,
        d: Some(typed_user),
        e: None,
    };

    Json(resp)
}

// https://stackoverflow.com/questions/61179070/rust-chrono-parse-date-string-parseerrornotenough-and-parseerrortooshort
async fn get_users_by_page(Query(params): Query<HashMap<String, String>>, conn: DbConn) -> Json<MyResponse<(Vec<TypedUser<ReqWxMessageData4KeywordTemplate>>, i64, i64)>> {
    // let start_date = params.get("start_from").unwrap().as_str();
    // println!("start_date -> {}", start_date);

    let some_page = params.get("page");
    let some_page_size = params.get("page_size").clone();

    let param_some_page = if some_page.is_some() { Some(some_page.unwrap().parse::<i64>().unwrap()) } else { None };
    let param_some_page_size = if some_page_size.is_some() { Some(some_page_size.unwrap().parse::<i64>().unwrap()) } else { None };

    let page_params = PageParams {
        page: param_some_page,
        page_size: param_some_page_size,
        start_from: None,
        // start_from: Some(NaiveDateTime::parse_from_str(start_date, "%Y-%m-%d %H:%M:%S").unwrap()),
    };


    println!("page: {:?}, page_size: {:?}", &page_params.page, &page_params.page_size);

    let r = paginate_users(&page_params, &mut extract_conn(conn));

    println!("get_users_by_page -> {:?}", r);

    let resp = MyResponse {
        r: true,
        d: Some(r.unwrap()),
        // d: None,
        e: None,
    };

    Json(resp)
}


// https://stackoverflow.com/questions/60717746/how-to-accept-an-async-function-as-an-argument
pub async fn call_async<F, T, U>(f: F, arg: String) -> U
    where
        F: Fn(String) -> T,
        T: Future<Output=U> + Send,
        U: Send,
{
    // tokio::spawn(f(1));
    f(arg).await
}

// can't be &String here!
async fn async_foo(arg: String) -> ReqWxMessageDataValue {
    ReqWxMessageDataValue { value: arg.clone() }
}

async fn req_async() -> Json<MyResponse<ReqWxMessageDataValue>> {
    // let r = call_async(async_foo, &"foo".to_string()).await;
    let r = call_async(async_foo, "42".to_string()).await;
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


fn db_create_typed_user<T: Debug + Serialize + DeserializeOwned + Clone,
    W: Clone + Debug + Serialize + DeserializeOwned + 'static>(db_conn: DbConn, in_user: &TypedUser<T>) -> TypedUser<W> {
    use crate::schema::users::dsl::*;

    insert_into(users)
        .values(in_user.clone())
        .get_result::<TypedUser<W>>(extract_conn(db_conn).deref_mut())
        .unwrap()
}


fn _db_create_user(conn: &mut PgConnection, in_user: &User) -> User {
    use crate::schema::users::dsl::*;

    insert_into(users)
        .values(in_user.clone())
        .get_result::<User>(conn)
        .unwrap()
}


async fn delete_user_by_id(conn: DbConn, Path(id): Path<String>) -> impl IntoResponse {
    let to_delete_user = TypedUser::<String>::db_delete_typed_user::<String>(&mut extract_conn(conn), &id).unwrap();

    (StatusCode::OK, Json(to_delete_user))
}


async fn create_with_typed_user(
    conn: DbConn,
    Json(payload): Json<ReqTypedUser<ReqWxMessageData4KeywordTemplate>>) -> impl IntoResponse {
    let mut data: HashMap<String, String> = HashMap::new();

    data.insert("foo".to_string(), "1".to_string());
    data.insert("bar".to_string(), "1".to_string());

    let meta: TypedMeta<ReqWxMessageData4KeywordTemplate> = TypedMeta {
        meta: None,
        data: Some(data),
    };

    // insert your application logic here
    let in_user = TypedUser {
        id: uuid(),
        username: payload.username.unwrap(),
        created_at: Some(Local::now().naive_local()),
        meta: Some(meta),
    };

    let created_user = db_create_typed_user::<ReqWxMessageData4KeywordTemplate, String>(conn, &in_user);

    let out_user = ReqTypedUser {
        id: Some(created_user.id),
        username: Some(created_user.username),
        created_at: Some(created_user.created_at.unwrap().to_string()),
        meta: created_user.meta.clone(),
    };

    (StatusCode::CREATED, Json(out_user))
}


async fn create_user(
    conn: DbConn,
    Json(payload): Json<ReqUser>,
) -> impl IntoResponse {
    let mut data: HashMap<String, String> = HashMap::new();

    data.insert("foo".to_string(), "1".to_string());
    data.insert("bar".to_string(), "1".to_string());

    let meta = TypedMeta {
        meta: Some(ReqWxMessageData4KeywordTemplate {
            first: ReqWxMessageDataValue { value: "first".to_string() },
            remark: ReqWxMessageDataValue { value: "remark".to_string() },
        }),
        data: Some(data),
    };


    // insert your application logic here
    let in_user = TypedUser {
        id: uuid(),
        username: payload.username.unwrap(),
        created_at: Some(Local::now().naive_local()),
        meta: Some(meta),
    };

    let created_user = db_create_typed_user::<ReqWxMessageData4KeywordTemplate, ReqWxMessageData4KeywordTemplate>(conn, &in_user);

    let out_user = ReqTypedUser {
        id: Some(created_user.id),
        username: Some(created_user.username),
        created_at: Some(created_user.created_at.unwrap().to_string()),
        meta: created_user.meta.clone(),
    };

    (StatusCode::CREATED, Json(out_user))
}


#[derive(Deserialize, Serialize, Clone, thiserror::Error)]
pub struct HtyErr {
    pub code: HtyErrCode,
    pub reason: Option<String>,
}

impl fmt::Display for HtyErr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl fmt::Debug for HtyErr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}", self.code, self.reason.clone().get_or_insert("".to_string()))
    }
}

impl PartialEq for HtyErr {
    fn eq(&self, other: &Self) -> bool {
        self.code == other.code && self.reason == other.reason
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub enum HtyErrCode {
    DbErr,
    InternalErr,
    CommonError,
    WebErr,
    JwtErr,
    WxErr,
    NullErr,
    NotFoundErr,
    NotEqualErr,
    AuthenticationFailed,
    ConflictErr,
}

impl fmt::Display for HtyErrCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl PartialEq for HtyErrCode {
    fn eq(&self, other: &Self) -> bool {
        self.to_string() == other.to_string()
    }
}

#[derive(
// AsExpression,
Debug,
Serialize,
Deserialize)]
#[serde(bound = "")]
struct ReqTypedUser<T: Debug + DeserializeOwned + Serialize + Clone> {
    id: Option<String>,
    username: Option<String>,
    created_at: Option<String>,
    meta: Option<TypedMeta<T>>,
}


#[derive(AsExpression, FromSqlRow, Debug, Default, Serialize, Deserialize, PartialEq, Clone)]
#[diesel(sql_type = Jsonb)]
#[serde(bound = "")]
pub struct TypedMeta<T: Debug + DeserializeOwned + Serialize + Clone> {
    pub meta: Option<T>,
    pub data: Option<HashMap<String, String>>,
}


#[derive(
// AsExpression,
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
#[diesel(table_name = users)]
#[serde(bound = "")]
pub struct TypedUser<T: Debug + DeserializeOwned + Serialize + Clone> {
    id: String,
    username: String,
    created_at: Option<NaiveDateTime>,
    meta: Option<TypedMeta<T>>,
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReqWxMessageData4KeywordTemplate {
    pub first: ReqWxMessageDataValue,
    pub remark: ReqWxMessageDataValue,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReqWxMessageDataValue {
    pub value: String,
}


impl<T: Debug + DeserializeOwned + Serialize + Clone + 'static> TypedUser<T> {
    pub fn db_delete_typed_user<U: Debug + DeserializeOwned + Serialize + Clone + 'static>(conn: &mut PgConnection, id_user: &String) -> anyhow::Result<TypedUser<U>> {
        let to_delete = TypedUser::find_typed_user_by_id(id_user, conn)?;

        use crate::schema::users::dsl::*;
        match diesel::delete(users.find(id_user)).execute(conn) {
            Ok(_) => Ok(to_delete),
            Err(e) => Err(anyhow!(HtyErr {
    code: HtyErrCode::DbErr,
    reason: Some(e.to_string()),
    })),
        }
    }

    pub fn find_typed_user_by_id(id_user: &String, conn: &mut PgConnection) -> anyhow::Result<TypedUser<T>> {
        // use crate::schema::users::dsl::*;
        // use crate::schema::users::dsl::*;
        match users::table.filter(users::id.eq(id_user))
            .select(users::all_columns).first::<TypedUser<T>>(conn)
        {
            Ok(user) => Ok(user),
            Err(e) => Err({
                error!("find_by_id / err -> {:?}", e);
                anyhow!(HtyErr {
                    code: HtyErrCode::DbErr,
                    reason: Some(e.to_string()),
                })
            }),
        }
    }
}

#[derive(
Debug,
Serialize,
Deserialize)]
struct ReqUser {
    id: Option<String>,
    username: Option<String>,
    created_at: Option<String>,
    meta: Option<Meta>,
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
#[diesel(table_name = users)]
struct User {
    id: String,
    username: String,
    created_at: Option<NaiveDateTime>,
    meta: Option<Meta>,
}


#[derive(AsExpression, FromSqlRow, Debug, Default, Serialize, Deserialize, PartialEq, Clone)]
#[diesel(sql_type = Jsonb)]
pub struct Meta {
    pub meta: Option<String>,
    pub data: Option<HashMap<String, String>>,
}


impl_jsonb_boilerplate!(Meta);

#[macro_export]
macro_rules! impl_jsonb_boilerplate {
    ($name: ident) => {
        impl ::diesel::deserialize::FromSql<::diesel::sql_types::Jsonb, ::diesel::pg::Pg>
            for $name
        {
            fn from_sql(bytes: PgValue) -> diesel::deserialize::Result<Self> {
                let value = <::serde_json::Value as ::diesel::deserialize::FromSql<
                    ::diesel::sql_types::Jsonb,
                    ::diesel::pg::Pg,
                >>::from_sql(bytes)?;
                Ok(::serde_json::from_value(value)?)
            }
        }

        impl ::diesel::serialize::ToSql<::diesel::sql_types::Jsonb, Pg> for $name {
            fn to_sql<'b>(
                &'b self,
                out: &mut ::diesel::serialize::Output<'b, '_, Pg>,
            ) -> ::diesel::serialize::Result {
                out.write_all(&[1])?;
                ::serde_json::to_writer(out, &::serde_json::to_value(self)?)
                    .map(|_| IsNull::No)
                    .map_err(Into::into)
            }
        }
    };
}

// ------------------------------------------------------------------------------------------------

#[macro_export]
macro_rules! impl_typed_jsonb_boilerplate {
    ($name: ident) => {
        impl <T: Debug + Serialize + DeserializeOwned + Clone> ::diesel::deserialize::FromSql<::diesel::sql_types::Jsonb, ::diesel::pg::Pg>
            for $name<T>
        {
            fn from_sql(bytes: PgValue) -> diesel::deserialize::Result<Self> {
                let value = <::serde_json::Value as ::diesel::deserialize::FromSql<
                    ::diesel::sql_types::Jsonb,
                    ::diesel::pg::Pg,
                >>::from_sql(bytes)?;
                Ok(::serde_json::from_value(value)?)
            }
        }

        impl<T> ::diesel::serialize::ToSql<::diesel::sql_types::Jsonb, Pg> for $name<T>
            where
                T: Debug + Serialize + DeserializeOwned + Clone,
        {
            fn to_sql<'b>(
                &'b self,
                out: &mut ::diesel::serialize::Output<'b, '_, Pg>,
            ) -> ::diesel::serialize::Result {
                out.write_all(&[1])?;
                ::serde_json::to_writer(out, &::serde_json::to_value(self)?)
                    .map(|_| IsNull::No)
                    .map_err(Into::into)
            }
        }
    };
}

impl_typed_jsonb_boilerplate!(TypedMeta);

fn all_users(conn: &mut PgConnection) -> Vec<User> {
    use crate::schema::users::dsl::*;
    users.load::<User>(conn).unwrap()
}

#[derive(Debug, Deserialize)]
pub struct PageParams {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub start_from: Option<NaiveDateTime>,
}

fn paginate_users<T: Debug + DeserializeOwned + Serialize + Clone + 'static>(params: &PageParams, conn: &mut PgConnection) -> anyhow::Result<(Vec<TypedUser<T>>, i64, i64)> {
    use crate::pagination::*;
    use diesel::prelude::*;

    let mut _query = users::table.into_boxed();

    debug!("paginate_users -> params: {:?}", params);

    let r = _query
        .order(users::created_at.desc())
        // .filter(users::created_at.ge(params.start_from.unwrap()))
        // .load_with_pagination(&conn, params.page, params.page_size)?;
        .paginate(params.page.clone())
        .per_page(params.page_size.clone())
        .load_and_count_pages::<TypedUser<T>>(conn);

    debug!("paginate_users -> {:?}", r);

    let (_users, _total_pages, _total) = r?;

    debug!("users: {:?} / total_pages: {:?} / total: {:?}", _users, _total_pages, _total);
    Ok((_users, _total_pages, _total))
}

#[derive(QueryableByName, Default, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SqlUser {
    #[diesel(sql_type = diesel::sql_types::Varchar)]
    upper_username: String,
    #[diesel(sql_type = diesel::sql_types::Nullable < diesel::sql_types::Jsonb >)]
    meta: Option<Meta>,
    #[diesel(sql_type = diesel::sql_types::Int4)]
    len_username: i32,
}

pub async fn find_all_sql_users(conn: DbConn) -> Json<MyResponse<Vec<SqlUser>>> {
    let sql_users = raw_find_all_sql_users(&mut extract_conn(conn)).unwrap();
    let resp = MyResponse {
        r: true,
        d: Some(sql_users),
        e: None,
    };
    Json(resp)
}

fn raw_find_all_sql_users(conn: &mut PgConnection) -> anyhow::Result<Vec<SqlUser>> {
    let q = format!("SELECT UPPER(username) as upper_name, meta, LEN(username) as len_username FROM users");
    let res = sql_query(q.clone()).load(conn)?;
    debug!("raw_find_all_sql_users -> res: {:?}", res);
    Ok(res)
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
        .route("/req_async", get(req_async))
        // `POST /users` goes to `create_user`
        .route("/users", post(create_user))
        .route("/typed_users", post(create_with_typed_user))
        .route("/find_user_by_id/:id", get(find_user_by_id))
        .route("/delete_user_by_id/:id", get(delete_user_by_id))
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
        .route("/current_time", get(get_current_time))
        .route("/req_conn", get(req_conn))
        .route("/find_all_sql_users", get(find_all_sql_users))
        .with_state(shared_db_state);
    // .layer(Extension(shared_db_state));


    // run our app with hyper
    // `axum::Server` is a re-export of `hyper::Server`
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    // tracing::debug!("listening on {}", addr);

    let listener = TcpListener::bind(&addr).await.unwrap();
    debug!("listening on {}", addr);

    let _ = axum::serve(listener, app.into_make_service()).await.unwrap();
}