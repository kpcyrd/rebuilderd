use openidconnect::core::{
    CoreClient, CoreGenderClaim, CoreIdTokenClaims, CoreIdTokenVerifier, CoreProviderMetadata,
    CoreResponseType,
};
use openidconnect::{
    AdditionalClaims, AuthenticationFlow, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    IssuerUrl, Nonce, OAuth2TokenResponse, RedirectUrl, Scope, UserInfoClaims,
};
use rebuilderd_common::errors::*;
use url::Url;

pub struct Oidc {
    http: reqwest::Client,
}

pub fn client(
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

    Ok(Oidc { http })
}
