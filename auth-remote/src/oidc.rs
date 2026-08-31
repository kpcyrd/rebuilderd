use actix_web::cookie::Cookie;
use openidconnect::core::{
    CoreAuthDisplay, CoreAuthPrompt, CoreAuthenticationFlow, CoreClient, CoreErrorResponseType,
    CoreGenderClaim, CoreJsonWebKey, CoreJweContentEncryptionAlgorithm, CoreJwsSigningAlgorithm,
    CoreProviderMetadata, CoreRevocableToken, CoreTokenType,
};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, EmptyAdditionalClaims,
    EmptyExtraTokenFields, EndpointMaybeSet, EndpointNotSet, EndpointSet, IdTokenFields, IssuerUrl,
    Nonce, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, RevocationErrorResponseType, Scope,
    StandardErrorResponse, StandardTokenIntrospectionResponse, StandardTokenResponse,
    TokenResponse, reqwest,
};
use rand::distr::{Alphanumeric, SampleString};
use rebuilderd_common::errors::*;
use std::num::NonZero;
use tokio::sync::Mutex;
use url::Url;

const VERIFY_LRU_SIZE: NonZero<usize> = NonZero::new(2048).unwrap();
pub const COOKIE_NAME: &str = "oidc_login";

// The design of the openidconnect crate is a little bit insane
type Client = openidconnect::Client<
    EmptyAdditionalClaims,
    CoreAuthDisplay,
    CoreGenderClaim,
    CoreJweContentEncryptionAlgorithm,
    CoreJsonWebKey,
    CoreAuthPrompt,
    StandardErrorResponse<CoreErrorResponseType>,
    StandardTokenResponse<
        IdTokenFields<
            EmptyAdditionalClaims,
            EmptyExtraTokenFields,
            CoreGenderClaim,
            CoreJweContentEncryptionAlgorithm,
            CoreJwsSigningAlgorithm,
        >,
        CoreTokenType,
    >,
    StandardTokenIntrospectionResponse<EmptyExtraTokenFields, CoreTokenType>,
    CoreRevocableToken,
    StandardErrorResponse<RevocationErrorResponseType>,
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

pub struct Oidc {
    http: reqwest::Client,
    client: Client,
    verify: Mutex<lru::LruCache<String, (CsrfToken, Nonce, PkceCodeVerifier)>>,
}

impl Oidc {
    pub async fn auth_url(&self) -> (url::Url, Cookie<'_>) {
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        let (auth_url, csrf_state, nonce) = self
            .client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("openid".to_owned()))
            .set_pkce_challenge(pkce_challenge)
            .url();

        let cookie = Alphanumeric.sample_string(&mut rand::rng(), 16);
        self.verify
            .lock()
            .await
            .put(cookie.clone(), (csrf_state, nonce, pkce_verifier));
        let cookie = Cookie::new(COOKIE_NAME, cookie);

        (auth_url, cookie)
    }

    pub async fn verify(&self, cookie: &Cookie<'_>, code: &str, state: &str) -> bool {
        let Some((csrf_token, nonce, pkce_verifier)) = self.verify.lock().await.pop(cookie.value())
        else {
            return false;
        };

        if state != csrf_token.secret().as_str() {
            return false;
        }

        let code = AuthorizationCode::new(code.to_string());

        let Ok(request) = self.client.exchange_code(code) else {
            return false;
        };

        let Ok(token_response) = request
            .set_pkce_verifier(pkce_verifier)
            .request_async(&self.http)
            .await
        else {
            return false;
        };

        let Some(id_token) = token_response.id_token() else {
            return false;
        };

        let Ok(claims) = id_token.claims(&self.client.id_token_verifier(), &nonce) else {
            return false;
        };

        let subject = claims.subject().as_str();
        dbg!(subject);

        true
    }
}

pub async fn client(
    client_id: String,
    client_secret: String,
    issuer: Url,
    redirect_url: Url,
) -> Result<Oidc> {
    let http = reqwest::ClientBuilder::new()
        // Following redirects opens the client up to SSRF vulnerabilities.
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let client_id = ClientId::new(client_id);
    let client_secret = ClientSecret::new(client_secret);

    let issuer = IssuerUrl::from_url(issuer);
    let redirect_url = RedirectUrl::from_url(redirect_url);

    let provider_metadata = CoreProviderMetadata::discover_async(issuer, &http).await?;

    let client: Client =
        CoreClient::from_provider_metadata(provider_metadata, client_id, Some(client_secret))
            .set_redirect_uri(redirect_url);

    Ok(Oidc {
        http,
        client,
        verify: Mutex::new(lru::LruCache::new(VERIFY_LRU_SIZE)),
    })
}
