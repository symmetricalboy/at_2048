use crate::idb::{
    DB_NAME, SESSIONS_STORE, STATE_STORE, clear_store, object_delete, object_get, transaction_put,
};
/// Storage impls to persis OAuth sessions if you are not using the memory stores
/// https://github.com/bluesky-social/statusphere-example-app/blob/main/src/auth/storage.ts
use atrium_api::types::string::Did;
use atrium_common::store::Store;
use atrium_oauth::store::session::SessionStore;
use atrium_oauth::store::state::StateStore;
use indexed_db_futures::database::Database;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::error::Error as StdError;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use wasm_bindgen_futures::spawn_local;
use futures::channel::oneshot;

#[derive(Debug)]
pub enum AuthStoreError {
    InvalidSession,
    NoSessionFound,
    DatabaseError(String),
}

impl Display for AuthStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSession => write!(f, "Invalid session"),
            Self::NoSessionFound => write!(f, "No session found"),
            Self::DatabaseError(err) => write!(f, "Database error: {}", err),
        }
    }
}

impl StdError for AuthStoreError {}

///Persistent session store in sqlite
impl SessionStore for IndexDBSessionStore {}

pub struct IndexDBSessionStore {
    // db: Database,
}

impl IndexDBSessionStore {
    pub fn new() -> Self {
        Self {}
    }
}

impl<K, V> Store<K, V> for IndexDBSessionStore
where
    K: Debug + Eq + Hash + Send + Sync + 'static + From<Did> + AsRef<str>,
    V: Debug + Clone + Send + Sync + 'static + Serialize + DeserializeOwned,
{
    type Error = AuthStoreError;
    async fn get(&self, key: &K) -> Result<Option<V>, Self::Error> {
        let (tx, rx) = oneshot::channel();
        let did_owned = key.as_ref().to_string();

        spawn_local(async move {
            let result = async {
                let db = match Database::open(DB_NAME).await {
                    Ok(db) => db,
                    Err(e) => {
                        return Err(AuthStoreError::DatabaseError(format!("DB open error: {:?}", e)));
                    }
                };
                match object_get::<V>(db.clone(), SESSIONS_STORE, &did_owned).await {
                    Ok(Some(session)) => Ok(Some(session)),
                    Ok(None) => Err(AuthStoreError::NoSessionFound),
                    Err(e) => Err(AuthStoreError::DatabaseError(format!("DB get error: {:?}", e))),
                }
            }
            .await;
            if tx.send(result).is_err() {
                log::error!("SessionStore: Failed to send 'get' result through oneshot channel");
            }
        });
        rx.await.map_err(|_| AuthStoreError::DatabaseError("SessionStore: Oneshot channel canceled for 'get'".to_string()))?
    }

    async fn set(&self, key: K, value: V) -> Result<(), Self::Error> {
        let (tx, rx) = oneshot::channel();
        let did_owned = key.as_ref().to_string();

        spawn_local(async move {
            let result = async {
                let db = match Database::open(DB_NAME).await {
                    Ok(db) => db,
                    Err(e) => {
                        return Err(AuthStoreError::DatabaseError(format!("DB open error: {:?}", e)));
                    }
                };
                transaction_put(db.clone(), &value, SESSIONS_STORE, Some(did_owned))
                    .await
                    .map_err(|e| AuthStoreError::DatabaseError(format!("DB set error: {:?}", e)))
            }
            .await;
            if tx.send(result).is_err() {
                log::error!("SessionStore: Failed to send 'set' result through oneshot channel");
            }
        });
        rx.await.map_err(|_| AuthStoreError::DatabaseError("SessionStore: Oneshot channel canceled for 'set'".to_string()))?
    }

    async fn del(&self, key: &K) -> Result<(), Self::Error> {
        let (tx, rx) = oneshot::channel();
        let key_owned = key.as_ref().to_string();

        spawn_local(async move {
            let result = async {
                let db = match Database::open(DB_NAME).await {
                    Ok(db) => db,
                    Err(e) => {
                        return Err(AuthStoreError::DatabaseError(format!("DB open error: {:?}", e)));
                    }
                };
                object_delete(db.clone(), SESSIONS_STORE, &key_owned)
                    .await
                    .map_err(|e| AuthStoreError::DatabaseError(format!("DB del error: {:?}", e)))
            }
            .await;
            if tx.send(result).is_err() {
                log::error!("SessionStore: Failed to send 'del' result through oneshot channel");
            }
        });
        rx.await.map_err(|_| AuthStoreError::DatabaseError("SessionStore: Oneshot channel canceled for 'del'".to_string()))?
    }

    async fn clear(&self) -> Result<(), Self::Error> {
        let (tx, rx) = oneshot::channel();
        spawn_local(async move {
            let result = async {
                let db = match Database::open(DB_NAME).await {
                    Ok(db) => db,
                    Err(e) => {
                        return Err(AuthStoreError::DatabaseError(format!("DB open error: {:?}", e)));
                    }
                };
                clear_store(db.clone(), SESSIONS_STORE)
                    .await
                    .map_err(|e| AuthStoreError::DatabaseError(format!("DB clear error: {:?}", e)))
            }
            .await;
            if tx.send(result).is_err() {
                log::error!("SessionStore: Failed to send 'clear' result through oneshot channel");
            }
        });
        rx.await.map_err(|_| AuthStoreError::DatabaseError("SessionStore: Oneshot channel canceled for 'clear'".to_string()))?
    }
}

impl StateStore for IndexDBStateStore {}

pub struct IndexDBStateStore {}

impl IndexDBStateStore {
    pub fn new() -> Self {
        Self {}
    }
}

impl<K, V> Store<K, V> for IndexDBStateStore
where
    K: Debug + Eq + Hash + Send + Sync + 'static + From<Did> + AsRef<str>,
    V: Debug + Clone + Send + Sync + 'static + Serialize + DeserializeOwned,
{
    type Error = AuthStoreError;
    async fn get(&self, key: &K) -> Result<Option<V>, Self::Error> {
        let (tx, rx) = oneshot::channel();
        let key_owned = key.as_ref().to_string();

        spawn_local(async move {
            let result = async {
                let db = match Database::open(DB_NAME).await {
                    Ok(db) => db,
                    Err(e) => {
                        return Err(AuthStoreError::DatabaseError(format!("DB open error: {:?}", e)));
                    }
                };
                match object_get::<V>(db.clone(), STATE_STORE, &key_owned).await {
                    Ok(Some(session)) => Ok(Some(session)),
                    Ok(None) => Err(AuthStoreError::NoSessionFound),
                    Err(e) => Err(AuthStoreError::DatabaseError(format!("DB get error: {:?}", e))),
                }
            }
            .await;
            if tx.send(result).is_err() {
                log::error!("StateStore: Failed to send 'get' result through oneshot channel");
            }
        });
        rx.await.map_err(|_| AuthStoreError::DatabaseError("StateStore: Oneshot channel canceled for 'get'".to_string()))?
    }

    async fn set(&self, key: K, value: V) -> Result<(), Self::Error> {
        let (tx, rx) = oneshot::channel();
        let key_owned = key.as_ref().to_string();

        spawn_local(async move {
            let result = async {
                let db = match Database::open(DB_NAME).await {
                    Ok(db) => db,
                    Err(e) => {
                        return Err(AuthStoreError::DatabaseError(format!("DB open error: {:?}", e)));
                    }
                };
                transaction_put(db.clone(), &value, STATE_STORE, Some(key_owned))
                    .await
                    .map_err(|e| AuthStoreError::DatabaseError(format!("DB set error: {:?}", e)))
            }
            .await;
            if tx.send(result).is_err() {
                log::error!("StateStore: Failed to send 'set' result through oneshot channel");
            }
        });
        rx.await.map_err(|_| AuthStoreError::DatabaseError("StateStore: Oneshot channel canceled for 'set'".to_string()))?
    }

    async fn del(&self, key: &K) -> Result<(), Self::Error> {
        let (tx, rx) = oneshot::channel();
        let key_owned = key.as_ref().to_string();

        spawn_local(async move {
            let result = async {
                let db = match Database::open(DB_NAME).await {
                    Ok(db) => db,
                    Err(e) => {
                        return Err(AuthStoreError::DatabaseError(format!("DB open error: {:?}", e)));
                    }
                };
                object_delete(db.clone(), STATE_STORE, &key_owned)
                    .await
                    .map_err(|e| AuthStoreError::DatabaseError(format!("DB del error: {:?}", e)))
            }
            .await;
            if tx.send(result).is_err() {
                log::error!("StateStore: Failed to send 'del' result through oneshot channel");
            }
        });
        rx.await.map_err(|_| AuthStoreError::DatabaseError("StateStore: Oneshot channel canceled for 'del'".to_string()))?
    }

    async fn clear(&self) -> Result<(), Self::Error> {
        let (tx, rx) = oneshot::channel();
        spawn_local(async move {
            let result = async {
                let db = match Database::open(DB_NAME).await {
                    Ok(db) => db,
                    Err(e) => {
                        return Err(AuthStoreError::DatabaseError(format!("DB open error: {:?}", e)));
                    }
                };
                clear_store(db.clone(), STATE_STORE)
                    .await
                    .map_err(|e| AuthStoreError::DatabaseError(format!("DB clear error: {:?}", e)))
            }
            .await;
            if tx.send(result).is_err() {
                log::error!("StateStore: Failed to send 'clear' result through oneshot channel");
            }
        });
        rx.await.map_err(|_| AuthStoreError::DatabaseError("StateStore: Oneshot channel canceled for 'clear'".to_string()))?
    }
}
