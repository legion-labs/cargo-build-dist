use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct TokenCredentials {
    pub username: String,
    pub password: String,
    pub endpoint: String,
}

impl TokenCredentials {
    fn new(token: String, endpoint: String) -> Result<Self, String> {
        let bytes = base64::decode(token).unwrap();
        let decoded_token = std::str::from_utf8(&bytes).unwrap();
        let basic_credentials = decoded_token.split(':');
        let credentials: Vec<&str> = basic_credentials.collect();
        if credentials.is_empty() {
            return Err("Cannot find credentials".to_string());
        }
        Ok(Self {
            username: credentials[0].to_string(),
            password: credentials[1].to_string(),
            endpoint,
        })
    }
}

pub async fn get_credentials_from_aws_ecr_authorization_token() -> Result<TokenCredentials, String> {
    let client = aws_sdk_ecr::Client::from_env();
    let resp = client.get_authorization_token().send().await;
    match resp {
        Ok(s) => {
            if let Some(data) = s.authorization_data {
                let authorization = data.first().unwrap();
                let ecr_endpoint = authorization
                    .proxy_endpoint
                    .as_ref()
                    .unwrap()
                    .replace("https://", "");
                let token = authorization.authorization_token.as_ref().unwrap();
                Ok(TokenCredentials::new(token.clone(), ecr_endpoint).unwrap())
            } else {
                Err("Fail to deseriazlize Authorization data".to_string())
            }
        }
        Err(e) => Err(format!("Failed to get ecr authorization token {}", e)),
    }
}

pub async fn repository_exists(name: String) -> bool {
    let client = aws_sdk_ecr::Client::from_env();
    let resp = client.describe_repositories().send().await;
    let describe_repositories = resp.unwrap();
    for repository in describe_repositories.repositories.unwrap() {
        if repository.repository_name.unwrap() == name {
            return true;
        }
    }
    false
}

pub async fn create_repository(name: String) -> Result<(), String> {
    let client = aws_sdk_ecr::Client::from_env();
    let resp = client
        .create_repository()
        .repository_name(&name)
        .send()
        .await;
    match resp {
        Ok(result) => {
            let repository = result.repository.unwrap();
            println!(
                "Repository {} is create",
                repository.repository_name.unwrap()
            );
            Ok(())
        }
        Err(e) => Err(format!("Failed to create repository {} : {}", &name, e)),
    }
}




