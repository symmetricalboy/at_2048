use crate::atrium_stores::{IndexDBSessionStore, IndexDBStateStore};
use crate::resolver::ApiDNSTxtResolver;
use atrium_identity::{
    did::{CommonDidResolver, CommonDidResolverConfig, DEFAULT_PLC_DIRECTORY_URL},
    handle::{AtprotoHandleResolver, AtprotoHandleResolverConfig},
};
use atrium_oauth::{
    AtprotoClientMetadata, AtprotoLocalhostClientMetadata, AuthMethod, DefaultHttpClient,
    GrantType, KnownScope, OAuthClient, OAuthClientConfig, OAuthResolverConfig, Scope,
};
use std::sync::Arc;

pub type OAuthClientType = Arc<
    OAuthClient<
        IndexDBStateStore,
        IndexDBSessionStore,
        CommonDidResolver<DefaultHttpClient>,
        AtprotoHandleResolver<ApiDNSTxtResolver, DefaultHttpClient>,
    >,
>;

pub async fn oauth_client() -> OAuthClientType {
    // Create a new OAuth client
    let http_client = Arc::new(DefaultHttpClient::default());
    let session_store = IndexDBSessionStore::new();
    let state_store = IndexDBStateStore::new();
    let resolver = OAuthResolverConfig {
        did_resolver: CommonDidResolver::new(CommonDidResolverConfig {
            plc_directory_url: DEFAULT_PLC_DIRECTORY_URL.to_string(),
            http_client: http_client.clone(),
        }),
        handle_resolver: AtprotoHandleResolver::new(AtprotoHandleResolverConfig {
            dns_txt_resolver: ApiDNSTxtResolver::default(),
            http_client: http_client.clone(),
        }),
        authorization_server_metadata: Default::default(),
        protected_resource_metadata: Default::default(),
    };

    let origin = std::option_env!("APP_ORIGIN")
        .unwrap_or("http://127.0.0.1:8080")
        .to_string();

    match origin.contains("127.0.0.1") {
        true => {
            let config = OAuthClientConfig {
                client_metadata: AtprotoLocalhostClientMetadata {
                    redirect_uris: Some(vec![format!("{}/oauth/callback", origin)]),
                    scopes: Some(vec![
                        Scope::Known(KnownScope::Atproto),
                        Scope::Known(KnownScope::TransitionGeneric),
                    ]),
                },
                keys: None,
                state_store,
                session_store,
                resolver,
            };
            Arc::new(OAuthClient::new(config).expect("failed to create OAuth client"))
        }
        false => {
            let client_metadata = AtprotoClientMetadata {
                client_id: format!("{}/client_metadata.json", origin),
                client_uri: Some(origin.clone()),
                redirect_uris: vec![format!("{}/oauth/callback", origin)],
                token_endpoint_auth_method: AuthMethod::None,
                grant_types: vec![GrantType::AuthorizationCode, GrantType::RefreshToken],
                scopes: vec![
                    Scope::Known(KnownScope::Atproto),
                    Scope::Known(KnownScope::TransitionGeneric),
                ],
                jwks_uri: None,
                token_endpoint_auth_signing_alg: None,
            };
            let config = OAuthClientConfig {
                client_metadata,
                keys: None,
                state_store,
                session_store,
                resolver,
            };
            Arc::new(OAuthClient::new(config).expect("failed to create OAuth client"))
        }
    }
}
