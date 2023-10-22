use warp::{Filter, Rejection, Reply};
use reqwest::StatusCode;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::Instant;
use reqwest::Response;

const START_TIMEOUT: Duration = Duration::from_secs(5);

pub fn origin_url(addr: ([u8; 4], u16)) -> String {
    let host = addr.0.map(|i| i.to_string()).join(".");
    format!("http://{}:{}", host, addr.1)
}

async fn sensor_get_handler(_new_val: String) -> Result<impl Reply, Rejection> {
    //let priority_mode = RWSENSORS[0].clone();
    Ok("This is a test".to_string())
}

async fn healthcheck_handler() -> Result<impl Reply, Rejection> {
    //let priority_mode = RWSENSORS[0].clone();
    Ok("Everything is ok.".to_string())
}

pub struct Server {
    pub shutdown_signal: Option<oneshot::Sender<()>>,
    pub(crate) _join_handle: tokio::task::JoinHandle<()>,
    pub(crate) address: ([u8; 4], u16),
}


impl Server {
    
    pub async fn start(addr: ([u8; 4], u16)) -> Server {
        let unstable_api = warp::path!("api" / "unstable" / String)
            .and(warp::get())
            .and_then(sensor_get_handler);
        let healthcheck_api_route = warp::path!("api" / "healthcheck")
            .and(warp::get())
            .and_then(healthcheck_handler);
        let routes = unstable_api.or(healthcheck_api_route);

        let (tx, rx) = oneshot::channel();

        let (_, server) = warp::serve(routes).bind_with_graceful_shutdown(addr, async move { rx.await.ok(); });
        let join_handle = tokio::task::spawn(server);
        let server = Server {
            shutdown_signal: Some(tx),
            _join_handle: join_handle,
            address: addr,
        };
        server.wait_for_healthcheck().await;
        server
    }

    pub async fn wait_for_healthcheck(&self) {
        let deadline = Instant::now() + START_TIMEOUT;

        while Instant::now() <= deadline {
            if let Some(res) = self.do_healthcheck().await {
                if res.status() == StatusCode::OK {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("Server did not become available.");
    }

    pub async fn do_healthcheck(&self) -> Option<Response> {
        reqwest::get(origin_url(self.address) + "/api/healthcheck").await.ok()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(shutdown_signal) = self.shutdown_signal.take() {
            if shutdown_signal.send(()).is_err() {
                eprintln!("failed send shutdown signal to server");
            }
        }
    }
}