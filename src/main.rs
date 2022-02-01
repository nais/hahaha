use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{Api, ListParams},
    runtime::{utils::try_flatten_applied, watcher},
    Client,
};

mod actions;
mod haha;
use haha::Handleable;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    std::env::set_var("RUST_LOG", "info,kube=warn");
    tracing_subscriber::fmt::init();
    
    let actions = actions::generate()?;
    let client = Client::try_default().await?;

    let pods: Api<Pod> = Api::all(client.clone());
    let lp = ListParams::default()
        .timeout(30)
        // I'm leaning towards using labels as a filter for target pods.. so I'm using it here
        .labels("nais.io/ginuudan=enabled");

    let mut ew = try_flatten_applied(watcher(pods, lp)).boxed();

    while let Some(pod) = ew.try_next().await? {
        pod.handle(&client, &actions).await?;
    }
    Ok(())
}

