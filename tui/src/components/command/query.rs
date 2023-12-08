use crate::ConnectionAuth;

pub fn get(auth: ConnectionAuth, uri: &'static str) -> String {
    let client = reqwest::blocking::ClientBuilder::new()
        .add_root_certificate(reqwest::Certificate::from_pem(&auth.pem).unwrap())
        .build()
        .unwrap();
    let request = client
        .get(auth.url.join(uri).expect("get should be correct").as_str())
        .header("Macaroon", auth.macaroon)
        .send();

    match request {
        Ok(response) => {
            let status = response.status();
            let data = response.text().unwrap();
            if status.is_success() {
                data.to_string()
            } else {
                format!("{}{}", status, data)
            }
        }
        Err(request_error) => request_error.to_string(),
    }
}

pub fn post(auth: ConnectionAuth, uri: &'static str, payload: String) -> String {
    let client = reqwest::blocking::ClientBuilder::new()
        .add_root_certificate(reqwest::Certificate::from_pem(&auth.pem).unwrap())
        .build()
        .unwrap();
    let request = client
        .post(auth.url.join(uri).expect("post should be correct").as_str())
        .header("Macaroon", auth.macaroon)
        .header("Content-Type", "application/json")
        .body(payload)
        .send();

    match request {
        Ok(response) => {
            let status = response.status();
            let data = response.text().unwrap();
            if status.is_success() {
                data.to_string()
            } else {
                format!("{}{}", status, data)
            }
        }
        Err(request_error) => request_error.to_string(),
    }
}
