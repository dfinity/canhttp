use ic_cdk::update;

#[update]
pub async fn hello_world() -> String {
    "Hello, World".to_string()
}

fn main() {}
