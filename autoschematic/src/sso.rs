// use oauth2::AccessToken;
// use oauth2::url::Url;
// use oauth2::{
//     AuthUrl, ClientId, DeviceAuthorizationResponse, DeviceAuthorizationUrl, EmptyExtraDeviceAuthorizationFields, Scope,
//     TokenResponse, TokenUrl, basic::BasicClient,
// };

// TODO Revisit this when we have some use for it...

// const CLIENT_ID: &str = "GITHUB_CLIENT_ID";

// pub async fn _login_via_github() -> anyhow::Result<AccessToken> {
//     let http_client = reqwest::ClientBuilder::new()
//         .redirect(reqwest::redirect::Policy::none())
//         .build()?;
//     let client = BasicClient::new(ClientId::new(CLIENT_ID.to_owned()))
//         .set_auth_uri(AuthUrl::from_url(Url::parse("https://github.com/login/oauth/authorize")?))
//         .set_token_uri(TokenUrl::from_url(Url::parse("https://github.com/login/oauth/access_token")?))
//         .set_device_authorization_url(DeviceAuthorizationUrl::from_url(Url::parse(
//             "https://github.com/login/device/code",
//         )?));

//     let details: DeviceAuthorizationResponse<EmptyExtraDeviceAuthorizationFields> = client
//         .exchange_device_code()
//         .add_scope(Scope::new("repo".into()))
//         .request_async(&http_client)
//         .await?;

//     println!("{details:?}");

//     let verify_url = details.verification_uri().to_string();
//     let full_url = details.verification_uri_complete();
//     let user_code = details.user_code().secret();

//     if let Some(url) = full_url {
//         webbrowser::open(url.secret())?;
//     } else {
//         println!("Open {verify_url} and enter code {user_code}");
//         // webbrowser::open(&verify_url)?;
//     }

//     let token = client
//         .exchange_device_access_token(&details)
//         .request_async(&http_client, tokio::time::sleep, None)
//         .await?;

//     println!("{token:?}");

//     Ok(token.access_token().clone())
// }

// pub fn _persist_github_token(token: &AccessToken) -> anyhow::Result<()> {
//     // TOOD this ought to be stored in the keyring
//     let proj_dirs = directories::ProjectDirs::from("com", "autoschematic", "cli").unwrap();
//     let path = proj_dirs.config_dir().join("github_token.json");
//     std::fs::create_dir_all(path.parent().unwrap())?;
//     std::fs::write(path, serde_json::to_string(token)?)?;
//     Ok(())
// }

// pub fn _load_github_token() -> anyhow::Result<Option<AccessToken>> {
//     let proj_dirs = directories::ProjectDirs::from("com", "autoschematic", "cli").unwrap();
//     let path = proj_dirs.config_dir().join("github_token.json");
//     if path.is_file() {
//         Ok(Some(serde_json::from_str(&std::fs::read_to_string(path)?)?))
//     } else {
//         Ok(None)
//     }
// }
