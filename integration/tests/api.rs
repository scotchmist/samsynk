use integration::setup::TestContext;
use reqwest;
use test_context::test_context;

#[test_context(TestContext)]
#[tokio_shared_rt::test]
async fn check_api_healthcheck(tctx: &mut TestContext) {
    let ret = tctx.http_get("/api/healthcheck").await.unwrap();
    assert_eq!(ret.status(), reqwest::StatusCode::OK);
    assert_eq!(ret.text().await.unwrap(), "Everything is OK!");
}

#[test_context(TestContext)]
#[tokio_shared_rt::test]
async fn check_read_battery_power(tctx: &mut TestContext) {
    tctx.set_sensor_state("battery_power".to_string(), vec![9001])
        .await
        .unwrap();

    let ret = tctx.http_get("/api/unstable/battery_power").await.unwrap();
    assert_eq!(ret.status(), reqwest::StatusCode::OK);
    assert_eq!(ret.text().await.unwrap(), "9001");
}

#[test_context(TestContext)]
#[tokio_shared_rt::test]
async fn check_read_battery_current(tctx: &mut TestContext) {
    tctx.set_sensor_state("battery_current".to_string(), vec![500])
        .await
        .unwrap();

    let ret = tctx
        .http_get("/api/unstable/battery_current")
        .await
        .unwrap();
    assert_eq!(ret.status(), reqwest::StatusCode::OK);
    assert_eq!(ret.text().await.unwrap(), "5");
}

#[test_context(TestContext)]
#[tokio_shared_rt::test]
async fn check_read_grid_power(tctx: &mut TestContext) {
    tctx.set_sensor_state("grid_power".to_string(), vec![123])
        .await
        .unwrap();

    let ret = tctx.http_get("/api/unstable/grid_power").await.unwrap();
    assert_eq!(ret.status(), reqwest::StatusCode::OK);
    assert_eq!(ret.text().await.unwrap(), "123");
}

#[test_context(TestContext)]
#[tokio_shared_rt::test]
async fn check_register_write(tctx: &mut TestContext) {
    tctx.set_sensor_state("priority_load".to_string(), vec![0])
        .await
        .unwrap();

    let ret = tctx
        .http_post("/api/unstable/priority_load", "1")
        .await
        .unwrap();

    assert_eq!(ret.status(), reqwest::StatusCode::OK);

    let ret = tctx.http_get("/api/unstable/priority_load").await.unwrap();
    assert_eq!(ret.text().await.unwrap(), "1");
}
